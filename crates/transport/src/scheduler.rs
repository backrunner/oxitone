//! Pattern clip expansion and the block event scheduler
//! (`.agents/docs/02-domain-spec.md` §Track、Pattern、PatternClip).
//!
//! Expansion happens once at compile time; the block query path is a pair
//! of binary searches over the pre-sorted event array and never allocates
//! or locks. Interpretation notes where the spec is silent:
//!
//! - Clip `probability` is drawn per scheduled note, seeded by
//!   `hash64(project seed, clip id, loop iteration, note ordinal)` so the
//!   same project + seed always produces the same events.
//! - Swing is evaluated against the absolute project beat grid (1/16 =
//!   0.25 beat); notes exactly on an odd grid index shift by
//!   `swing * 0.125` beat, off-grid notes are untouched.

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::pcg32::{hash64, Hash64Part, Pcg32};
use oxitone_core::wire::{EntityId, PatternClipSpec, PatternSpec};

use crate::beat_cmp;
use crate::event::{EventPayload, EventPriority, ScheduledEvent};
use crate::tempo::CompiledTempoMap;

/// One pattern clip to schedule, with its resolved channel and swing.
/// `swing` is the per-event value hook (automation lands in M3); today it
/// is a constant in 0..=1 and only affects note events.
pub struct ClipSource<'a> {
    pub clip: &'a PatternClipSpec,
    pub pattern: &'a PatternSpec,
    pub channel_id: &'a EntityId,
    pub swing: f64,
}

/// Precompiled event scheduler. The event array is sorted by
/// `(frame, priority, sequence)` at compile time; block queries return
/// sub-slices of it and perform no allocation.
#[derive(Debug, Clone)]
pub struct Scheduler {
    events: Vec<ScheduledEvent>,
}

impl Scheduler {
    /// Expand all clip sources into a sorted event array. Validation
    /// failures return `InvalidProject` with a path.
    pub fn compile(
        sources: &[ClipSource],
        tempo: &CompiledTempoMap,
        seed: u64,
    ) -> Result<Self, OxitoneError> {
        let mut events = Vec::new();
        let mut sequence = 0_u64;
        for source in sources {
            expand_source(source, tempo, seed, &mut sequence, &mut events)?;
        }
        events.sort_by_key(|e| (e.frame, e.priority, e.sequence));
        Ok(Self { events })
    }

    /// All scheduled events, sorted by `(frame, priority, sequence)`.
    pub fn events(&self) -> &[ScheduledEvent] {
        &self.events
    }

    /// Number of scheduled events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the scheduler holds no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Events inside `[frame_start, frame_end)`, sorted by
    /// `(frame, priority, sequence)`. Allocation-free: two binary searches
    /// over the pre-sorted array.
    pub fn events_in_range(&self, frame_start: u64, frame_end: u64) -> &[ScheduledEvent] {
        let lo = self.events.partition_point(|e| e.frame < frame_start);
        let hi = self.events.partition_point(|e| e.frame < frame_end);
        &self.events[lo..hi]
    }
}

fn invalid(message: impl Into<String>, path: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

fn expand_source(
    source: &ClipSource,
    tempo: &CompiledTempoMap,
    seed: u64,
    sequence: &mut u64,
    events: &mut Vec<ScheduledEvent>,
) -> Result<(), OxitoneError> {
    let clip = source.clip;
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
    if !source.swing.is_finite() || !(0.0..=1.0).contains(&source.swing) {
        return Err(invalid("swing must be in 0..=1", path("swing")));
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

    let pattern_len = source.pattern.length_beats;
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
            if !beat_cmp(last, start).is_gt() {
                return Err(invalid(
                    "lastBeat must be after startBeat",
                    path("lastBeat"),
                ));
            }
            last
        }
        _ => start.checked_add(pattern_len)?,
    };

    let transpose = clip.transpose.unwrap_or(0);
    let notes = &source.pattern.notes;
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
        let note_path = format!("$.patterns[{}].notes[{i}]", source.pattern.id);
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
            let shift = swing_shift(on_beat, source.swing);
            let on_frame = tempo.beat_f64_to_frame(on_beat.to_f64() + shift);
            let off_beat = on_beat.checked_add(note.duration)?;
            let off_beat = if beat_cmp(off_beat, end).is_gt() {
                end
            } else {
                off_beat
            };
            let off_frame = tempo.beat_f64_to_frame(off_beat.to_f64() + shift);
            let pitch = (i32::from(note.pitch) + transpose) as u8;
            let on_velocity = (note.velocity * scale).clamp(0.0, 1.0);
            let off_velocity = (note.off_velocity.unwrap_or(note.velocity) * scale).clamp(0.0, 1.0);
            events.push(ScheduledEvent {
                frame: on_frame,
                priority: EventPriority::NoteOn,
                sequence: *sequence,
                track_id: clip.track_id.clone(),
                channel_id: source.channel_id.clone(),
                payload: EventPayload::NoteOn {
                    pitch,
                    velocity: on_velocity,
                },
            });
            events.push(ScheduledEvent {
                frame: off_frame,
                priority: EventPriority::NoteOff,
                sequence: *sequence,
                track_id: clip.track_id.clone(),
                channel_id: source.channel_id.clone(),
                payload: EventPayload::NoteOff {
                    pitch,
                    velocity: off_velocity,
                },
            });
            *sequence += 1;
        }
        iteration += 1;
    }
    Ok(())
}

fn swing_shift(beat: Beat, swing: f64) -> f64 {
    if swing == 0.0 {
        return 0.0;
    }
    let grid_num = i128::from(beat.numerator()) * 4;
    let grid_den = i128::from(beat.denominator());
    if grid_num % grid_den == 0 && (grid_num / grid_den) % 2 == 1 {
        swing * 0.125
    } else {
        0.0
    }
}
