//! Shared effective tempo table for audio and MIDI; no asset decoding.
use super::{clips, value::pattern_content_end};
use oxitone_core::wire::{ProjectSnapshot, SampleRef, TempoSegment};
use oxitone_core::{
    error::{codes, OxitoneError},
    Beat,
};
use oxitone_transport::tempo::{CompiledTempoMap, TempoMap};
use oxitone_transport::{bake_tempo_lane, find_tempo_lane};
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
    let mut end = pattern_content_end(snapshot)?;
    for clip in &snapshot.sample_clips {
        if !track_enabled(&clip.track_id) {
            continue;
        }
        let sample = sample_refs.get(clip.sample_id.as_str()).ok_or_else(|| {
            OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!("unknown sample id {:?}", clip.sample_id),
                format!("$.sampleClips[{}].sampleId", clip.id),
            )
        })?;
        let duration = clips::resolve_duration(clip, sample, tempo);
        end = end.max(clip.start_beat.to_f64() + duration);
    }
    for lane in &snapshot.automation {
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
    let tempo = TempoMap::compile(&snapshot.tempo_map, sample_rate)?;
    if let Some(lane) = find_tempo_lane(&snapshot.automation, &snapshot.id)? {
        bake_tempo_lane(
            &lane.source,
            seed,
            estimate_end(snapshot, &tempo)?,
            tail_seconds,
        )
    } else {
        Ok(snapshot.tempo_map.clone())
    }
}

pub(super) fn content_end(
    snapshot: &ProjectSnapshot,
    tempo: &CompiledTempoMap,
) -> Result<Beat, OxitoneError> {
    Beat::from_f64(estimate_end(snapshot, tempo)?)
}
