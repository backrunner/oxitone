//! Non-destructive edit baking: trim, level, normalize, and fades are applied
//! at prepare time into an immutable PCM segment (`02-domain-spec.md` §Sample,
//! `03-audio-runtime-spec.md` §Sample player). `tone` is a runtime tilt
//! parameter and is intentionally not baked here.

use oxitone_core::error::codes;
use oxitone_core::wire::{FadeCurve, FadeSpec, SampleEditSpec, SampleRef};
use oxitone_core::OxitoneError;

use crate::resample::resample_planes;
use crate::types::{DecodedSample, LoopPoints, PreparedSample};

fn edit_err(message: impl Into<String>, path: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

fn fade_gain(curve: FadeCurve, p: f64) -> f32 {
    let p = p.clamp(0.0, 1.0);
    (match curve {
        FadeCurve::Linear => p,
        FadeCurve::EqualPower => (p * std::f64::consts::FRAC_PI_2).sin(),
        FadeCurve::Exponential => p * p,
    }) as f32
}

fn apply_fade_in(planes: &mut [Vec<f32>], spec: &FadeSpec) {
    let frames = planes[0].len();
    let length = (spec.length_frames as usize).min(frames);
    let curve = spec.curve.unwrap_or(FadeCurve::Linear);
    for i in 0..length {
        let g = fade_gain(curve, i as f64 / length as f64);
        for plane in planes.iter_mut() {
            plane[i] *= g;
        }
    }
}

fn apply_fade_out(planes: &mut [Vec<f32>], spec: &FadeSpec) {
    let frames = planes[0].len();
    let length = (spec.length_frames as usize).min(frames);
    let curve = spec.curve.unwrap_or(FadeCurve::Linear);
    for i in (frames - length)..frames {
        let from_end = frames - 1 - i;
        let g = fade_gain(curve, from_end as f64 / length as f64);
        for plane in planes.iter_mut() {
            plane[i] *= g;
        }
    }
}

fn apply_normalize(planes: &mut [Vec<f32>], peak_db: f64) -> Result<(), OxitoneError> {
    if !peak_db.is_finite() || peak_db > 0.0 {
        return Err(edit_err(
            format!("normalize peakDb must be finite and <= 0 dBFS, got {peak_db}"),
            "$.edits.normalize.peakDb",
        ));
    }
    let peak = planes
        .iter()
        .flat_map(|p| p.iter())
        .fold(0.0f32, |a, &b| a.max(b.abs()));
    if peak > 0.0 {
        let gain = (10f64.powf(peak_db / 20.0) / peak as f64) as f32;
        for plane in planes.iter_mut() {
            for sample in plane.iter_mut() {
                *sample *= gain;
            }
        }
    }
    Ok(())
}

fn remap_loop(
    loop_points: Option<LoopPoints>,
    start: u64,
    segment_frames: u64,
    source_rate: u32,
    target_rate: u32,
) -> Option<LoopPoints> {
    let lp = loop_points?;
    if lp.end_frame <= start || lp.start_frame >= start + segment_frames {
        return None;
    }
    let trim_start = lp.start_frame.max(start) - start;
    let trim_end = (lp.end_frame.min(start + segment_frames)) - start;
    if trim_start >= trim_end {
        return None;
    }
    if source_rate == target_rate {
        return Some(LoopPoints {
            start_frame: trim_start,
            end_frame: trim_end,
        });
    }
    let scale = |frame: u64| {
        ((frame as u128 * target_rate as u128 + source_rate as u128 / 2) / source_rate as u128)
            as u64
    };
    let out = LoopPoints {
        start_frame: scale(trim_start),
        end_frame: scale(trim_end),
    };
    (out.start_frame < out.end_frame).then_some(out)
}

/// Bake `sample_ref.edits` into `decoded` and convert to `target_sample_rate`.
/// Edit order is: trim (`startFrame`/`endFrame`, half-open) → `level` →
/// `normalize` → `fadeIn`/`fadeOut` → SRC. Loop points are remapped into
/// prepared-frame coordinates; invalid or fully trimmed loops are dropped.
pub fn prepare(
    sample_ref: &SampleRef,
    decoded: &DecodedSample,
    target_sample_rate: u32,
) -> Result<PreparedSample, OxitoneError> {
    if target_sample_rate == 0 {
        return Err(edit_err(
            "target sample rate must be > 0",
            "$.engineOptions.sampleRate",
        ));
    }
    let source_frames = decoded.frames();
    let edits = sample_ref.edits.clone().unwrap_or(SampleEditSpec {
        start_frame: None,
        end_frame: None,
        level: None,
        tone: None,
        normalize: None,
        fade_in: None,
        fade_out: None,
        crossfade: None,
    });
    let start = edits.start_frame.unwrap_or(0);
    let end = edits.end_frame.unwrap_or(source_frames);
    if start >= end || end > source_frames {
        return Err(edit_err(
            format!("invalid trim range [{start}, {end}) for {source_frames} source frames"),
            "$.edits.startFrame",
        ));
    }
    if let Some(level) = edits.level {
        if !level.is_finite() || !(0.0..=2.0).contains(&level) {
            return Err(edit_err(
                format!("level must be in 0..=2, got {level}"),
                "$.edits.level",
            ));
        }
    }

    let mut planes: Vec<Vec<f32>> = decoded
        .channels
        .iter()
        .map(|plane| plane[start as usize..end as usize].to_vec())
        .collect();
    if let Some(level) = edits.level {
        for plane in planes.iter_mut() {
            for sample in plane.iter_mut() {
                *sample *= level as f32;
            }
        }
    }
    if let Some(normalize) = &edits.normalize {
        apply_normalize(&mut planes, normalize.peak_db)?;
    }
    if let Some(fade) = &edits.fade_in {
        apply_fade_in(&mut planes, fade);
    }
    if let Some(fade) = &edits.fade_out {
        apply_fade_out(&mut planes, fade);
    }

    let segment_frames = end - start;
    let loop_points = remap_loop(
        decoded.loop_points,
        start,
        segment_frames,
        decoded.sample_rate,
        target_sample_rate,
    );
    let planes = resample_planes(&planes, decoded.sample_rate, target_sample_rate);
    Ok(PreparedSample {
        channels: planes,
        sample_rate: target_sample_rate,
        loop_points,
        musical_length_beats: sample_ref.musical_length_beats,
        metadata: decoded.metadata.clone(),
    })
}

#[cfg(test)]
mod tests;
