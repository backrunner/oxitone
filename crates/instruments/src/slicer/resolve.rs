//! Slice-table resolution: markers / grid / `onset-v1` → immutable
//! frame intervals (compile time only).

use oxitone_core::error::OxitoneError;
use oxitone_core::Beat;
use oxitone_samples::PreparedSample;

use super::onset::detect_onsets_v1;
use super::state::{err, Position, ResolvedSlice, SliceSource, SlicerConfig, MAX_SLICES};

/// Resolve the parsed state against the prepared sample into the immutable
/// frame-interval slice table. `beat_to_frame` (the compiler's baked tempo
/// table) is required when any marker uses `{ beat }`.
pub fn resolve_slices(
    config: &SlicerConfig,
    sample: &PreparedSample,
    beat_to_frame: Option<&dyn Fn(Beat) -> Option<u64>>,
) -> Result<Vec<ResolvedSlice>, OxitoneError> {
    let frames = sample.frames();
    if frames == 0 {
        return Err(err("$.state.sampleId", "sample is empty"));
    }
    let resolve_position = |pos: Position| -> Result<u64, OxitoneError> {
        match pos {
            Position::Frames(f) => Ok(f),
            Position::Beat(b) => beat_to_frame
                .and_then(|f| f(b))
                .ok_or_else(|| err("$.state.slices", "beat markers require a beat→frame map")),
        }
    };
    match &config.source {
        SliceSource::Grid(n) => {
            let n = u64::from(*n);
            Ok((0..n)
                .map(|k| ResolvedSlice {
                    start_frame: k * frames / n,
                    end_frame: (k + 1) * frames / n,
                    level: 1.0,
                    pan: 0.0,
                    rate: 1.0,
                    reverse: false,
                })
                .collect())
        }
        SliceSource::Onset { sensitivity } => {
            let mut starts = vec![0u64];
            starts.extend(detect_onsets_v1(
                &sample.channels,
                sample.sample_rate,
                *sensitivity,
            ));
            starts.sort_unstable();
            starts.dedup();
            starts.truncate(MAX_SLICES);
            Ok(starts
                .iter()
                .enumerate()
                .map(|(i, &start)| ResolvedSlice {
                    start_frame: start,
                    end_frame: starts.get(i + 1).copied().unwrap_or(frames),
                    level: 1.0,
                    pan: 0.0,
                    rate: 1.0,
                    reverse: false,
                })
                .collect())
        }
        SliceSource::Explicit(list) => {
            if list.len() > MAX_SLICES {
                return Err(err(
                    "$.state.slices",
                    format!("at most {MAX_SLICES} slices are supported"),
                ));
            }
            let mut resolved = Vec::with_capacity(list.len());
            for (i, slice) in list.iter().enumerate() {
                let path = format!("$.state.slices[{i}]");
                let start = resolve_position(slice.start)?;
                if start >= frames {
                    return Err(err(path, "slice start is beyond the sample end"));
                }
                resolved.push((start, slice.end, slice.over));
            }
            resolved.sort_by_key(|&(start, _, _)| start);
            for window in resolved.windows(2) {
                if window[0].0 == window[1].0 {
                    return Err(err("$.state.slices", "duplicate slice start"));
                }
            }
            let mut out = Vec::with_capacity(resolved.len());
            for (i, &(start, end, over)) in resolved.iter().enumerate() {
                let path = format!("$.state.slices[{i}]");
                let end = match end {
                    Some(pos) => {
                        let e = resolve_position(pos)?;
                        if e <= start {
                            return Err(err(path, "slice end must be after its start"));
                        }
                        e.min(frames)
                    }
                    None => resolved.get(i + 1).map(|&(s, _, _)| s).unwrap_or(frames),
                };
                out.push(ResolvedSlice {
                    start_frame: start,
                    end_frame: end,
                    level: over.level.unwrap_or(1.0) as f32,
                    pan: over.pan.unwrap_or(0.0) as f32,
                    rate: over.rate.unwrap_or(1.0),
                    reverse: over.reverse.unwrap_or(false),
                });
            }
            Ok(out)
        }
    }
}
