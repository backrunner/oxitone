//! Sample clip playback plans (02-domain-spec.md §Sample/§tempoSync,
//! 03-audio-runtime-spec.md §Sample player). Beat-domain lengths, content
//! lengths, loop regions and the stretch-ratio range check are resolved here
//! so the runtime player only executes precomputed data.

use std::sync::Arc;

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{SampleClipSpec, SampleRef, TempoSync};
use oxitone_samples::PreparedSample;
use oxitone_transport::tempo::CompiledTempoMap;

/// The only Phase 1 stretch algorithm (02-domain-spec.md §tempoSync).
pub const WSOLA_V1: &str = "wsola-v1";

/// Content loop region in prepared-sample frames plus the clip-timeline
/// frame at which looping stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipLoop {
    /// Loop region `[start_frame, end_frame)` in prepared (content) frames.
    pub start_frame: u64,
    pub end_frame: u64,
    /// Absolute clip-timeline frame where looping ends (playback continues
    /// one-shot past it until the clip end).
    pub until_frame: u64,
}

/// Fully resolved sample clip plan (sorted by clip ID in the plan).
pub struct SampleClipPlan {
    pub id: String,
    /// Owning channel indices (`RenderPlan::channels`); empty for clips on
    /// disabled tracks.
    pub channels: Vec<usize>,
    pub sample: Arc<PreparedSample>,
    pub start_beat: Beat,
    pub duration_beats: Beat,
    /// Clip window on the sample-frame timeline `[start_frame, end_frame)`.
    pub start_frame: u64,
    pub end_frame: u64,
    /// Declared or tempo-derived musical length of the prepared content.
    pub content_beats: f64,
    pub gain: f32,
    pub pan: f32,
    pub rate: f64,
    pub tempo_sync: TempoSync,
    pub loop_region: Option<ClipLoop>,
    pub enabled: bool,
}

/// Clip beat length: explicit `durationBeats`, otherwise the content length
/// (`musicalLengthBeats`, or content seconds converted at the clip's BPM).
pub(crate) fn resolve_duration(
    clip: &SampleClipSpec,
    sample: &SampleRef,
    tempo: &CompiledTempoMap,
) -> f64 {
    if let Some(duration) = clip.duration_beats {
        return duration.to_f64();
    }
    content_beats_fallback(
        sample.musical_length_beats,
        sample
            .edits
            .as_ref()
            .and_then(|e| e.end_frame)
            .unwrap_or(sample.frames)
            .saturating_sub(
                sample
                    .edits
                    .as_ref()
                    .and_then(|e| e.start_frame)
                    .unwrap_or(0),
            ),
        sample.sample_rate,
        clip.start_beat,
        tempo,
    )
}

/// Content beats when `musicalLengthBeats` is missing: integrate the effective
/// clock over the content's full seconds span (02-domain-spec.md §Sample).
fn content_beats_fallback(
    musical: Option<Beat>,
    frames: u64,
    sample_rate: u32,
    start_beat: Beat,
    tempo: &CompiledTempoMap,
) -> f64 {
    if let Some(musical) = musical {
        return musical.to_f64();
    }
    let seconds = frames as f64 / f64::from(sample_rate.max(1));
    tempo
        .seconds_to_beat(tempo.beat_to_seconds(start_beat) + seconds)
        .to_f64()
        - start_beat.to_f64()
}

/// WSOLA output/input duration ratio at `bpm` for this clip
/// (03-audio-runtime-spec.md §tempoSync stretch): the clip's beat span is
/// fixed on the baked timeline, so the instantaneous ratio tracks
/// `1 / bpm(t)` to keep content beats mapped linearly onto clip beats.
pub(crate) fn stretch_ratio(
    clip_beats: f64,
    content_frames: u64,
    sample_rate: u32,
    bpm: f64,
) -> f64 {
    clip_beats * 60.0 * f64::from(sample_rate) / (content_frames as f64 * bpm)
}

/// Resolve `clip` into a playback plan. Stretch clips validate the ratio
/// range over the clip's BPM span (`SampleStretchRange`).
pub(crate) fn plan_sample_clip(
    clip: &SampleClipSpec,
    sample: Arc<PreparedSample>,
    channels: Vec<usize>,
    tempo: &CompiledTempoMap,
) -> Result<SampleClipPlan, OxitoneError> {
    let path = |field: &str| format!("$.sampleClips[{}].{field}", clip.id);
    let tempo_sync = clip.tempo_sync.unwrap_or(TempoSync::Off);
    if tempo_sync == TempoSync::Stretch
        && clip.stretch_algorithm.as_deref().unwrap_or(WSOLA_V1) != WSOLA_V1
    {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!(
                "unknown stretchAlgorithm {:?}; only {WSOLA_V1:?} is supported",
                clip.stretch_algorithm
            ),
            path("stretchAlgorithm"),
        ));
    }

    let content_frames = sample.frames();
    let content_beats = content_beats_fallback(
        sample.musical_length_beats,
        content_frames,
        sample.sample_rate,
        clip.start_beat,
        tempo,
    );
    let duration = clip
        .duration_beats
        .unwrap_or(Beat::from_f64(content_beats)?);
    let start_frame = tempo.beat_to_frame(clip.start_beat);
    let end_beat = clip.start_beat.checked_add(duration)?;
    let end_frame = tempo.beat_to_frame(end_beat);

    if tempo_sync == TempoSync::Stretch {
        if content_frames == 0 {
            return Err(OxitoneError::with_path(
                codes::SAMPLE_STRETCH_RANGE,
                "stretch clip has empty content",
                path("sampleId"),
            ));
        }
        let (lo, hi) = tempo.bpm_range(clip.start_beat.to_f64(), end_beat.to_f64());
        let min_ratio = stretch_ratio(
            duration.to_f64(),
            content_frames,
            sample.sample_rate,
            hi.max(1e-9),
        );
        let max_ratio = stretch_ratio(
            duration.to_f64(),
            content_frames,
            sample.sample_rate,
            lo.max(1e-9),
        );
        // Bounds mirror `oxitone_dsp::wsola::{MIN_RATIO, MAX_RATIO}`; the
        // graph crate must not depend on oxitone-dsp (it sits below
        // instruments/mixer), so the 0.25..=4 contract is restated here.
        if min_ratio < 0.25 || max_ratio > 4.0 {
            return Err(OxitoneError::with_path(
                codes::SAMPLE_STRETCH_RANGE,
                format!(
                    "stretch ratio {min_ratio:.3}..={max_ratio:.3} over the clip's tempo span is outside 0.25..=4"
                ),
                path("tempoSync"),
            ));
        }
    }

    let loop_region = match &clip.loop_spec {
        Some(loop_spec) => {
            let loop_start = loop_spec.start_beat.unwrap_or(Beat::ZERO).to_f64();
            let loop_len = loop_spec.length_beats.to_f64();
            // Loop beats are *content* beats (musicalLengthBeats or the
            // tempo-derived fallback): for stretch/repitch content beats
            // equal the clip duration, so this matches the clip's content
            // map; for `off` it matches the sample's own musical scale.
            // Bounds clamp to the content.
            let content_scale = content_beats.max(1e-9);
            let to_content = |beat_offset: f64| -> u64 {
                ((beat_offset / content_scale) * content_frames as f64)
                    .floor()
                    .clamp(0.0, content_frames as f64) as u64
            };
            let start = to_content(loop_start);
            let end = to_content(loop_start + loop_len).max(start);
            let until_beat = match (loop_spec.count, loop_spec.last_beat) {
                (Some(count), None) => {
                    // Content-beat offsets scaled into the clip's beat span.
                    let scale = duration.to_f64().max(1e-9) / content_scale;
                    clip.start_beat.to_f64() + (loop_start + loop_len * f64::from(count)) * scale
                }
                (None, Some(last)) => last.to_f64(),
                _ => f64::INFINITY,
            };
            let until_frame = if until_beat.is_finite() {
                tempo
                    .beat_to_frame(Beat::from_f64(until_beat.max(0.0))?)
                    .min(end_frame)
            } else {
                end_frame
            };
            (end > start).then_some(ClipLoop {
                start_frame: start,
                end_frame: end,
                until_frame,
            })
        }
        None => None,
    };

    Ok(SampleClipPlan {
        id: clip.id.clone(),
        channels,
        sample,
        start_beat: clip.start_beat,
        duration_beats: duration,
        start_frame,
        end_frame,
        content_beats,
        gain: clip.gain.unwrap_or(1.0) as f32,
        pan: clip.pan.unwrap_or(0.0) as f32,
        rate: clip.rate.unwrap_or(1.0),
        tempo_sync,
        loop_region,
        enabled: clip.enabled != Some(false),
    })
}
