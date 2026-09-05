//! Numeric domain checks: ranges, finite values, mutually exclusive clip
//! length forms, and timeline map ordering (02-domain-spec.md). Clip/sample
//! and channel/mixer level domains live in `clips.rs` and `levels.rs`.

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{LoopSpec, ProjectSnapshot};

use super::{clips, levels};

const BPM_MIN: f64 = 20.0;
const BPM_MAX: f64 = 999.0;

pub(super) fn range(
    path: &str,
    field: &str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), OxitoneError> {
    if !value.is_finite() || value < min || value > max {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("{field} must be finite within {min}..={max}, got {value}"),
            path,
        ));
    }
    Ok(())
}

fn bpm(path: &str, value: f64) -> Result<(), OxitoneError> {
    if !value.is_finite() || !(BPM_MIN..=BPM_MAX).contains(&value) {
        return Err(OxitoneError::with_path(
            codes::TEMPO_RANGE,
            format!("bpm must be finite within {BPM_MIN}..={BPM_MAX}, got {value}"),
            path,
        ));
    }
    Ok(())
}

/// `lengthBeats > 0`; `count` and `lastBeat` are mutually exclusive.
pub(super) fn check_loop_spec(path: &str, loop_spec: &LoopSpec) -> Result<(), OxitoneError> {
    if loop_spec.length_beats.numerator() == 0 {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "loop lengthBeats must be > 0",
            path,
        ));
    }
    if loop_spec.count.is_some() && loop_spec.last_beat.is_some() {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "loop count and lastBeat are mutually exclusive",
            path,
        ));
    }
    if loop_spec.count == Some(0) {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "loop count must be >= 1",
            path,
        ));
    }
    Ok(())
}

/// `durationBeats`, `loopCount`, and `lastBeat` are alternatives; at most one
/// may be set, and `lastBeat` is an exclusive end after `startBeat`.
pub(super) fn check_clip_length(
    path: &str,
    duration_beats: Option<oxitone_core::Beat>,
    loop_count: Option<u32>,
    last_beat: Option<oxitone_core::Beat>,
    start_beat: oxitone_core::Beat,
) -> Result<(), OxitoneError> {
    let set =
        duration_beats.is_some() as u8 + loop_count.is_some() as u8 + last_beat.is_some() as u8;
    if set > 1 {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "durationBeats, loopCount and lastBeat are mutually exclusive",
            path,
        ));
    }
    if let Some(duration) = duration_beats {
        if duration.numerator() == 0 {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "durationBeats must be > 0",
                path,
            ));
        }
    }
    if loop_count == Some(0) {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "loopCount must be >= 1",
            path,
        ));
    }
    if let Some(last) = last_beat {
        if last.to_f64() <= start_beat.to_f64() {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "lastBeat must be greater than startBeat",
                path,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_numeric(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    let tempo = &snapshot.tempo_map;
    if tempo.is_empty() || tempo[0].start_beat.numerator() != 0 {
        return Err(OxitoneError::with_path(
            codes::TEMPO_MAP_ORDER,
            "tempoMap must be non-empty with the first segment at beat 0",
            "$.tempoMap",
        ));
    }
    for (i, segment) in tempo.iter().enumerate() {
        bpm(&format!("$.tempoMap[{i}].bpm"), segment.bpm)?;
        if i > 0 && segment.start_beat.to_f64() <= tempo[i - 1].start_beat.to_f64() {
            return Err(OxitoneError::with_path(
                codes::TEMPO_MAP_ORDER,
                "tempoMap segments must be strictly increasing by startBeat",
                format!("$.tempoMap[{i}].startBeat"),
            ));
        }
    }
    for (i, ts) in snapshot.time_signature_map.iter().enumerate() {
        let path = format!("$.timeSignatureMap[{i}]");
        if ts.numerator == 0 || ts.denominator == 0 || !ts.denominator.is_power_of_two() {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "time signature needs numerator >= 1 and a power-of-two denominator",
                path,
            ));
        }
        if (i == 0 && ts.start_bar != 1)
            || (i > 0 && ts.start_bar <= snapshot.time_signature_map[i - 1].start_bar)
        {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "timeSignatureMap starts at bar 1 and is strictly increasing",
                format!("{path}.startBar"),
            ));
        }
    }
    for (i, track) in snapshot.tracks.iter().enumerate() {
        if let Some(tempo) = track.tempo {
            bpm(&format!("$.tracks[{i}].tempo"), tempo)?;
        }
        if let Some(channel) = track.midi_channel {
            if !(1..=16).contains(&channel) {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("midiChannel must be 1..=16, got {channel}"),
                    format!("$.tracks[{i}].midiChannel"),
                ));
            }
        }
    }
    for (i, lane) in snapshot.automation.iter().enumerate() {
        let path = format!("$.automation[{i}]");
        if let Some(loop_spec) = &lane.loop_spec {
            check_loop_spec(&format!("{path}.loop"), loop_spec)?;
        }
        if lane.loop_spec.is_some() && lane.last_beat.is_some() {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "loop and lastBeat are mutually exclusive on an automation lane",
                path,
            ));
        }
    }
    clips::validate_clips(snapshot)?;
    levels::validate_levels(snapshot)?;
    Ok(())
}
