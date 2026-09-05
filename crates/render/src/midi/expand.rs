//! Beat-domain pattern clip expansion for MIDI export. The rules mirror
//! `oxitone_transport::scheduler` exactly (loop iteration, exclusive clip
//! end, transpose, velocity scale, per-note probability drawn from the same
//! seeded hash), except that swing is not applied and positions stay in
//! beats/ticks instead of frames.

use std::collections::BTreeMap;

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::pcg32::{hash64, Hash64Part, Pcg32};
use oxitone_core::wire::{PatternClipSpec, PatternSpec};

use super::{beat_to_ticks, velocity_to_midi};

/// One expanded note with tick positions and MIDI velocities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExpandedNote {
    pub tick_on: u64,
    pub tick_off: u64,
    pub pitch: u8,
    pub on_velocity: u8,
    pub off_velocity: u8,
    /// Deterministic creation ordinal; shared by the note's on/off pair.
    pub sequence: u64,
}

fn invalid(message: impl Into<String>, path: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

pub(crate) fn expand_track(
    clips: &[&PatternClipSpec],
    patterns: &BTreeMap<&str, &PatternSpec>,
    ppq: u16,
    seed: u64,
    sequence: &mut u64,
    clock: oxitone_transport::TrackClock<'_>,
) -> Result<Vec<ExpandedNote>, OxitoneError> {
    let mut notes = Vec::new();
    for clip in clips {
        let pattern = patterns.get(clip.pattern_id.as_str()).ok_or_else(|| {
            invalid(
                format!("unknown pattern id: {}", clip.pattern_id),
                format!("$.patternClips[{}].patternId", clip.id),
            )
        })?;
        expand_clip(clip, pattern, ppq, seed, sequence, &mut notes, clock)?;
    }
    Ok(notes)
}

fn expand_clip(
    clip: &PatternClipSpec,
    pattern: &PatternSpec,
    ppq: u16,
    seed: u64,
    sequence: &mut u64,
    out: &mut Vec<ExpandedNote>,
    clock: oxitone_transport::TrackClock<'_>,
) -> Result<(), OxitoneError> {
    if clip.enabled == Some(false) {
        return Ok(());
    }
    let path = |field: &str| format!("$.patternClips[{}].{field}", clip.id);

    let scale = clip.velocity_scale.unwrap_or(1.0);
    if !scale.is_finite() || !(0.0..=2.0).contains(&scale) {
        return Err(invalid(
            "velocityScale must be in 0..=2",
            path("velocityScale"),
        ));
    }
    if let Some(p) = clip.probability {
        if !p.is_finite() || !(0.0..=1.0).contains(&p) {
            return Err(invalid("probability must be in 0..=1", path("probability")));
        }
    }
    let end_kinds = u8::from(clip.duration_beats.is_some())
        + u8::from(clip.loop_count.is_some())
        + u8::from(clip.last_beat.is_some());
    if end_kinds > 1 {
        return Err(invalid(
            "durationBeats, loopCount and lastBeat are mutually exclusive",
            path("lastBeat"),
        ));
    }

    let pattern_len = pattern.length_beats;
    if pattern_len == Beat::ZERO {
        return Err(invalid(
            "pattern lengthBeats must be > 0",
            path("patternId"),
        ));
    }
    let start = clip.start_beat;
    let end = match (clip.duration_beats, clip.loop_count, clip.last_beat) {
        (Some(d), None, None) => {
            if d == Beat::ZERO {
                return Err(invalid("durationBeats must be > 0", path("durationBeats")));
            }
            start.checked_add(d)?
        }
        (None, Some(n), None) => {
            start.checked_add(pattern_len.checked_mul(Beat::new(i64::from(n), 1)?)?)?
        }
        (None, None, Some(last)) => {
            if beat_cmp(last, start).is_gt() {
                last
            } else {
                return Err(invalid(
                    "lastBeat must be after startBeat",
                    path("lastBeat"),
                ));
            }
        }
        _ => start.checked_add(pattern_len)?,
    };

    let transpose = clip.transpose.unwrap_or(0);
    let notes = &pattern.notes;
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|&a, &b| {
        beat_cmp(notes[a].start, notes[b].start)
            .then(
                notes[a]
                    .voice
                    .unwrap_or(0)
                    .cmp(&notes[b].voice.unwrap_or(0)),
            )
            .then(a.cmp(&b))
    });

    for (i, note) in notes.iter().enumerate() {
        let note_path = format!("$.patterns[{}].notes[{i}]", pattern.id);
        if note.duration == Beat::ZERO {
            return Err(invalid(
                "note duration must be > 0",
                format!("{note_path}.duration"),
            ));
        }
        if note.pitch > 127 {
            return Err(invalid(
                "note pitch must be in 0..=127",
                format!("{note_path}.pitch"),
            ));
        }
        if !note.velocity.is_finite() || !(0.0..=1.0).contains(&note.velocity) {
            return Err(invalid(
                "note velocity must be in 0..=1",
                format!("{note_path}.velocity"),
            ));
        }
        if let Some(off) = note.off_velocity {
            if !off.is_finite() || !(0.0..=1.0).contains(&off) {
                return Err(invalid(
                    "note offVelocity must be in 0..=1",
                    format!("{note_path}.offVelocity"),
                ));
            }
        }
        if !beat_cmp(note.start, pattern_len).is_lt() {
            return Err(invalid(
                "note start must be inside the pattern length",
                format!("{note_path}.start"),
            ));
        }
        let shifted = i32::from(note.pitch) + transpose;
        if !(0..=127).contains(&shifted) {
            return Err(invalid(
                "clip transpose pushes note pitch outside 0..=127",
                path("transpose"),
            ));
        }
    }

    let mut iteration = 0_u64;
    loop {
        let iter_offset = pattern_len.checked_mul(Beat::new(iteration as i64, 1)?)?;
        let iter_start = start.checked_add(iter_offset)?;
        if !beat_cmp(iter_start, end).is_lt() {
            break;
        }
        for &ni in &order {
            let note = &notes[ni];
            let on_beat = iter_start.checked_add(note.start)?;
            if !beat_cmp(on_beat, end).is_lt() {
                continue;
            }
            let probability = clip.probability.unwrap_or(1.0) * note.chance.unwrap_or(1.0);
            if probability < 1.0 {
                let p = probability;
                let draw_seed = hash64(&[
                    Hash64Part::Int(seed),
                    Hash64Part::Str(clip.id.clone()),
                    Hash64Part::Int(iteration),
                    Hash64Part::Int(ni as u64),
                ]);
                if Pcg32::new(draw_seed).next_f64() >= p {
                    continue;
                }
            }
            let off_beat = on_beat.checked_add(note.duration)?;
            let off_beat = if beat_cmp(off_beat, end).is_gt() {
                end
            } else {
                off_beat
            };
            let pitch = (i32::from(note.pitch) + transpose) as u8;
            let on_velocity = (note.velocity * scale).clamp(0.0, 1.0);
            let off_velocity = (note.off_velocity.unwrap_or(note.velocity) * scale).clamp(0.0, 1.0);
            out.push(ExpandedNote {
                tick_on: beat_to_ticks(clock.project_beat(on_beat), u32::from(ppq)),
                tick_off: beat_to_ticks(clock.project_beat(off_beat), u32::from(ppq)),
                pitch,
                on_velocity: velocity_to_midi(on_velocity),
                off_velocity: velocity_to_midi(off_velocity),
                sequence: *sequence,
            });
            *sequence += 1;
        }
        iteration += 1;
    }
    Ok(())
}

// Same exact rational comparison as `oxitone_transport::beat_cmp`; kept
// local so the exporter does not depend on scheduler internals.
fn beat_cmp(a: Beat, b: Beat) -> std::cmp::Ordering {
    (a.numerator() as i128 * b.denominator() as i128)
        .cmp(&(b.numerator() as i128 * a.denominator() as i128))
}
