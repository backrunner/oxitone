//! Shared effective tempo table for audio and MIDI; no asset decoding.
use super::{clips, value::pattern_content_end};
use oxitone_core::wire::{ProjectSnapshot, SampleRef, TempoSegment};
use oxitone_core::{
    error::{codes, OxitoneError},
    Beat,
};
use oxitone_transport::tempo::{CompiledTempoMap, TempoMap};
use oxitone_transport::TrackClock;
use oxitone_transport::{bake_tempo_lane_spec, find_tempo_lane};
use std::collections::BTreeMap;

fn estimate_end(snapshot: &ProjectSnapshot, tempo: &CompiledTempoMap) -> Result<f64, OxitoneError> {
    let sample_refs: BTreeMap<&str, &SampleRef> = snapshot
        .samples
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let track_enabled = |id: &str| {
        snapshot
            .tracks
            .iter()
            .any(|t| t.id == id && t.enabled != Some(false))
    };
    let mut end = pattern_content_end(snapshot, tempo)?;
    for clip in &snapshot.sample_clips {
        if clip.enabled == Some(false) || !track_enabled(&clip.track_id) {
            continue;
        }
        let sample = sample_refs.get(clip.sample_id.as_str()).ok_or_else(|| {
            OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!("unknown sample id {:?}", clip.sample_id),
                format!("$.sampleClips[{}].sampleId", clip.id),
            )
        })?;
        let bpm = snapshot
            .tracks
            .iter()
            .find(|t| t.id == clip.track_id)
            .and_then(|t| t.tempo);
        let clock = TrackClock::new(tempo, bpm)?;
        let duration = clips::resolve_duration(clip, sample, clock);
        end = end.max(
            clock
                .project_beat(clip.start_beat.checked_add(Beat::from_f64(duration)?)?)
                .to_f64(),
        );
    }
    for clip in snapshot.automation_clips.as_deref().unwrap_or(&[]) {
        if clip.enabled != Some(false) && track_enabled(&clip.track_id) {
            end = end.max(clip.start_beat.to_f64() + clip.duration_beats.unwrap().to_f64());
        }
    }
    for lane in &snapshot.automation {
        if lane.playback == Some(oxitone_core::wire::AutomationPlayback::Playlist) {
            continue;
        }
        if let Some(last) = lane.last_beat {
            end = end.max(last.to_f64());
        }
    }
    for marker in &snapshot.markers {
        end = end.max(marker.start_beat.to_f64());
    }
    Ok(end)
}

pub fn effective_tempo_table(
    snapshot: &ProjectSnapshot,
    sample_rate: u32,
    seed: u64,
    tail_seconds: f64,
) -> Result<Vec<TempoSegment>, OxitoneError> {
    TempoMap::compile(&snapshot.tempo_map, sample_rate)?;
    if let Some(lane) = find_tempo_lane(&snapshot.automation, &snapshot.id)? {
        // The fastest legal project clock bounds local Track seconds and
        // implicit sample durations before baking the actual tempo lane.
        let upper_clock = TempoMap::compile(
            &[TempoSegment {
                start_beat: Beat::ZERO,
                bpm: 999.0,
                curve: None,
            }],
            sample_rate,
        )?;
        let horizon = estimate_end(snapshot, &upper_clock)?;
        bake_tempo_lane_spec(lane, seed, horizon, tail_seconds)
    } else {
        Ok(snapshot.tempo_map.clone())
    }
}

/// Authoring query through the same clock as audio/MIDI, with no asset I/O.
pub fn resolve_beat_duration(
    snapshot: &ProjectSnapshot,
    start: Beat,
    seconds: f64,
) -> Result<Beat, OxitoneError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "durationSeconds must be finite and non-negative",
            "durationSeconds",
        ));
    }
    let mut tempo = TempoMap::compile(&snapshot.tempo_map, snapshot.sample_rate)?;
    if let Some(lane) = find_tempo_lane(&snapshot.automation, &snapshot.id)? {
        // 999 is the maximum BPM, so this covers the entire requested seconds span.
        let horizon = start.to_f64() + seconds * 999.0 / 60.0;
        let table = bake_tempo_lane_spec(lane, snapshot.seed, horizon, 0.0)?;
        tempo = TempoMap::compile(&table, snapshot.sample_rate)?;
    }
    let end = tempo.seconds_to_beat(tempo.beat_to_seconds(start) + seconds);
    if seconds > 0.0 && end.to_f64() <= start.to_f64() {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "timing query exceeds the representable beat range",
            "durationSeconds",
        ));
    }
    end.checked_sub(start)
}

pub(super) fn content_end(
    snapshot: &ProjectSnapshot,
    tempo: &CompiledTempoMap,
) -> Result<Beat, OxitoneError> {
    Beat::from_f64(estimate_end(snapshot, tempo)?)
}
