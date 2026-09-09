//! RenderGraph compile plan (`.agents/docs/03-audio-runtime-spec.md`
//! §RenderGraph 生命周期). [`compile_plan`] turns a validated snapshot into
//! an immutable [`RenderPlan`]: the effective (possibly tempo-lane-baked)
//! tempo map, the expanded note-event schedule, compiled automation lanes
//! bound to resolved targets, and per-sample-clip playback plans with
//! prepared PCM attached.
//!
//! ## Crate division
//!
//! This crate cannot depend on `oxitone-instruments` / `oxitone-mixer` (they
//! depend on this crate for the ABI traits), so the plan carries *data only*:
//! no plugin instances, no mixer engine, no runtime buffers. The caller
//! (`oxitone-render`) assembles those from the plan. Everything here runs on
//! the control thread.

mod bindings;
mod initial_value;
mod placement;
pub use placement::{lane_has_edge, LanePlacement};
mod clips;
mod clock;
pub use clock::{effective_tempo_table, resolve_beat_duration};
mod value;

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EffectRef, EntityId, InstrumentRef, ProjectSnapshot, SampleRef};
use oxitone_samples::PreparedSample;
use oxitone_transport::scheduler::{ClipSource, Scheduler};
use oxitone_transport::tempo::{CompiledTempoMap, TempoMap};
use oxitone_transport::timesig::{CompiledTimeSignatureMap, TimeSignatureMap};

use crate::registry::PluginRegistry;
use crate::validate::validate;

pub(crate) use bindings::resolve as resolve_automation_target;
pub use bindings::{AutomationBinding, BindingTarget, CompiledLane};
pub use clips::{ClipLoop, SampleClipPlan};
pub use value::{binding_value_at, combine_value, lane_beat, map_normalized};

/// Prepared-sample lookup used during plan compilation. Called on the
/// control thread; implementations typically layer `load_asset` +
/// `prepare_cached` + `SampleCache` (see `oxitone_render::SampleStore`).
pub trait PlanSampleProvider {
    /// Load, decode, edit-bake and rate-convert the referenced sample, or
    /// return the cached prepared segment. Failures surface as
    /// `AssetUnavailable`/`InvalidProject`.
    fn prepared_sample(&self, sample: &SampleRef) -> Result<Arc<PreparedSample>, OxitoneError>;
}

/// Compile-time overrides; unset fields fall back to the snapshot values.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub sample_rate: Option<u32>,
    pub block_size: Option<u32>,
    pub seed: Option<u64>,
    /// Extra seconds the tempo-lane bake covers past the project timeline
    /// (03-audio-runtime-spec.md §Tempo automation 烘焙). Default 0.
    pub tail_seconds: Option<f64>,
    /// Honor Track solo during playback; exports default to all unmuted Tracks.
    pub respect_solo: bool,
}

/// One mixer-bound channel of the arrangement (sorted by channel ID).
#[derive(Debug, Clone)]
pub struct ChannelPlan {
    pub id: EntityId,
    pub mixer_channel_id: EntityId,
    pub level: f64,
    pub pan: f64,
    /// Static swing; a `ChannelSwing` binding (if any) replaces it.
    pub swing: f64,
    pub mute: bool,
    pub solo: bool,
    pub instrument: InstrumentRef,
    pub effect_chain: Vec<EffectRef>,
    /// Index into `RenderPlan::bindings` for the automated swing lane.
    pub swing_binding: Option<usize>,
}

/// Immutable output of [`compile_plan`]: every compiled structure the
/// runtime needs, in deterministic order.
pub struct RenderPlan {
    pub sample_rate: u32,
    pub block_size: u32,
    pub seed: u64,
    /// Effective clock: the tempo-lane bake when a tempo lane exists,
    /// otherwise the compiled `tempoMap`.
    pub tempo: CompiledTempoMap,
    pub time_signatures: CompiledTimeSignatureMap,
    /// Note events, expanded with swing 0; swing is applied at dispatch time
    /// so automated swing stays sample-accurate (see `oxitone-render`).
    pub scheduler: Scheduler,
    /// Maximum beat reached by clips, automation and markers.
    pub content_end_beat: Beat,
    /// Channels sorted by ID.
    pub channels: Vec<ChannelPlan>,
    /// Sample clips sorted by ID.
    pub sample_clips: Vec<SampleClipPlan>,
    /// Automation bindings sorted by (entity ID, parameter ID); the tempo
    /// lane is consumed by the bake and never appears here.
    pub bindings: Vec<AutomationBinding>,
    /// Any static or automated swing: enables the swing-aware dispatch path.
    pub has_swing: bool,
}

/// Validate `snapshot` and compile the immutable render plan.
pub fn compile_plan(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    samples: &dyn PlanSampleProvider,
    options: &CompileOptions,
) -> Result<RenderPlan, OxitoneError> {
    validate(snapshot, registry)?;
    let sample_rate = options.sample_rate.unwrap_or(snapshot.sample_rate);
    let block_size = options.block_size.unwrap_or(snapshot.block_size);
    let seed = options.seed.unwrap_or(snapshot.seed);
    if sample_rate == 0 || block_size == 0 {
        return Err(OxitoneError::new(
            codes::INVALID_PROJECT,
            "sampleRate and blockSize must be > 0",
        ));
    }
    let tail_seconds = options.tail_seconds.unwrap_or(0.0);

    let mut channel_specs: Vec<&_> = snapshot.channels.iter().collect();
    channel_specs.sort_by(|a, b| a.id.cmp(&b.id));
    let channel_index: BTreeMap<&str, usize> = channel_specs
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();

    // Track membership (sorted track IDs) → sorted channel indices.
    let mut tracks: Vec<&_> = snapshot.tracks.iter().collect();
    tracks.sort_by(|a, b| a.id.cmp(&b.id));
    let track_channels = |track_id: &str| -> Vec<usize> {
        tracks
            .iter()
            .find(|t| t.id == track_id)
            .map(|t| {
                let mut ids: Vec<&str> = t.channel_ids.iter().map(String::as_str).collect();
                ids.sort_unstable();
                ids.iter()
                    .filter_map(|id| channel_index.get(id).copied())
                    .collect()
            })
            .unwrap_or_default()
    };
    let any_track_solo = options.respect_solo
        && tracks
            .iter()
            .any(|t| t.enabled != Some(false) && t.solo == Some(true));
    let track_enabled = |track_id: &str| -> bool {
        tracks
            .iter()
            .find(|t| t.id == track_id)
            .map(|t| t.audible(any_track_solo))
            .unwrap_or(false)
    };

    let sample_refs: BTreeMap<&str, &SampleRef> = snapshot
        .samples
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let mut sample_clips: Vec<_> = snapshot.sample_clips.iter().collect();
    sample_clips.sort_by(|a, b| a.id.cmp(&b.id));
    let tempo = TempoMap::compile(
        &effective_tempo_table(snapshot, sample_rate, seed, tail_seconds)?,
        sample_rate,
    )?;
    let time_signatures = TimeSignatureMap::compile(&snapshot.time_signature_map)?;

    // Note schedule: Track membership selects the Channel voices. Playlist edits change
    // clip placement while preserving the existing authoring contract.
    let patterns: BTreeMap<&str, &_> = snapshot
        .patterns
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    let mut sources = Vec::new();
    for channel in &channel_specs {
        for track in tracks.iter().filter(|t| t.audible(any_track_solo)) {
            let mut clip_ids: Vec<&str> =
                track.pattern_clip_ids.iter().map(String::as_str).collect();
            clip_ids.sort_unstable();
            for clip_id in clip_ids {
                let clip = snapshot
                    .pattern_clips
                    .iter()
                    .find(|c| c.id == clip_id)
                    .ok_or_else(|| {
                        OxitoneError::with_path(
                            codes::INVALID_PROJECT,
                            format!("unknown pattern clip id {clip_id:?}"),
                            format!("$.tracks[{}].patternClipIds", track.id),
                        )
                    })?;
                let pattern = patterns.get(clip.pattern_id.as_str()).ok_or_else(|| {
                    OxitoneError::with_path(
                        codes::INVALID_PROJECT,
                        format!("unknown pattern id {:?}", clip.pattern_id),
                        format!("$.patternClips[{}].patternId", clip.id),
                    )
                })?;
                let period = pattern.length_beats;
                let pattern = if let Some(parts) = &pattern.parts {
                    let Some(part) = parts.iter().find(|part| part.channel_id == channel.id) else {
                        continue;
                    };
                    patterns[part.pattern_id.as_str()]
                } else {
                    if !track.channel_ids.contains(&channel.id) {
                        continue;
                    }
                    *pattern
                };
                sources.push(ClipSource {
                    clip,
                    pattern,
                    period,
                    channel_id: &channel.id,
                    swing: 0.0,
                    track_tempo: track.tempo,
                });
            }
        }
    }
    let scheduler = Scheduler::compile(&sources, &tempo, seed)?;

    // Sample clip plans with the effective clock (sorted by clip ID).
    let mut clip_plans = Vec::with_capacity(sample_clips.len());
    for clip in &sample_clips {
        let sample = sample_refs.get(clip.sample_id.as_str()).ok_or_else(|| {
            OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!("unknown sample id {:?}", clip.sample_id),
                format!("$.sampleClips[{}].sampleId", clip.id),
            )
        })?;
        let prepared = samples.prepared_sample(sample)?;
        let channels = if track_enabled(&clip.track_id) {
            track_channels(&clip.track_id)
        } else {
            Vec::new()
        };
        let track_tempo = tracks
            .iter()
            .find(|t| t.id == clip.track_id)
            .and_then(|t| t.tempo);
        clip_plans.push(clips::plan_sample_clip(
            clip,
            prepared,
            channels,
            &tempo,
            track_tempo,
        )?);
    }
    let clip_index: BTreeMap<&str, usize> = sample_clips
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();

    let content_end_beat = clock::content_end(snapshot, &tempo)?;
    let bindings = bindings::compile_bindings(
        snapshot,
        registry,
        &channel_index,
        &clip_index,
        seed,
        any_track_solo,
    )?;

    let mut has_swing = channel_specs.iter().any(|c| c.swing.unwrap_or(0.0) != 0.0);
    let mut channels = Vec::with_capacity(channel_specs.len());
    for (i, spec) in channel_specs.iter().enumerate() {
        let swing_binding = bindings
            .iter()
            .position(|b| matches!(&b.target, BindingTarget::ChannelSwing(c) if *c == i));
        has_swing |= swing_binding.is_some();
        channels.push(ChannelPlan {
            id: spec.id.clone(),
            mixer_channel_id: spec.mixer_channel_id.clone(),
            level: spec.level,
            pan: spec.pan,
            swing: spec.swing.unwrap_or(0.0),
            mute: spec.mute.unwrap_or(false),
            solo: spec.solo.unwrap_or(false),
            instrument: spec.instrument.clone(),
            effect_chain: spec.effect_chain.clone(),
            swing_binding,
        });
    }

    Ok(RenderPlan {
        sample_rate,
        block_size,
        seed,
        tempo,
        time_signatures,
        scheduler,
        content_end_beat,
        channels,
        sample_clips: clip_plans,
        bindings,
        has_swing,
    })
}
