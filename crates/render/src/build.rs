//! RenderGraph assembly from a snapshot + `RenderPlan` (control thread;
//! allocates). Plugin instances come from `oxitone-instruments`
//! (`create_builtin_instance`, the single built-in integration point) or
//! straight from the registry (third-party ABI path); the mixer engine is
//! built from preprocessed specs where beat-unit insert parameters
//! (`timeBeats`-style) are converted to their DSP-facing seconds
//! counterparts (02-domain-spec.md §Mixer: 引擎经 tempo map 换算).

use std::collections::{BTreeMap, BTreeSet};

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{MixerChannelSpec, ProjectSnapshot};
use oxitone_graph::abi::HostContext;
use oxitone_graph::compile::{compile_plan, CompileOptions};
use oxitone_graph::PluginRegistry;
use oxitone_mixer::{DelayLine, MixerEngine};

use crate::assets::SampleStore;
use crate::build_plugins::{create_insert, create_instrument, preprocess_mixer_channel};
use crate::channel::ChannelNode;
use crate::clip::ClipNode;
use crate::graph::RenderGraph;
use crate::metronome::Metronome;

fn invalid(message: impl Into<String>, path: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

/// Master protection limiter plugin (03-audio-runtime-spec.md §数值精度:
/// 默认启用的最终 limiter/clip 保护节点).
pub const MASTER_LIMITER_PLUGIN_ID: &str = "oxitone.limit";
pub const MASTER_LIMITER_VERSION: &str = "1.0.0";

/// Assembly-time options for `RenderGraph::compile`.
#[derive(Debug, Clone)]
pub struct RenderGraphOptions {
    pub compile: CompileOptions,
    /// Master limiter/clip protection (default on; only diagnostics may
    /// disable it, 03-audio-runtime-spec.md §数值精度).
    pub master_limiter: bool,
    /// Honor channel/bus solo flags (export default off).
    pub respect_solo: bool,
    /// Metronome click level; `None` disables the click.
    pub metronome_level: Option<f32>,
}

impl Default for RenderGraphOptions {
    fn default() -> Self {
        Self {
            compile: CompileOptions::default(),
            master_limiter: true,
            respect_solo: false,
            metronome_level: None,
        }
    }
}

impl RenderGraph {
    /// Validate and compile the snapshot into a playable graph. Control
    /// thread; allocates every runtime buffer.
    pub fn compile(
        snapshot: &ProjectSnapshot,
        registry: &PluginRegistry,
        samples: &SampleStore,
        options: &RenderGraphOptions,
    ) -> Result<Self, OxitoneError> {
        let sample_rate = options.compile.sample_rate.unwrap_or(snapshot.sample_rate);
        samples.prepare_all(snapshot, sample_rate)?;
        let plan = compile_plan(snapshot, registry, samples, &options.compile)?;
        let max_block = plan.block_size;
        let host = HostContext {
            sample_rate: f64::from(sample_rate),
            max_block_size: max_block,
        };
        let bpm0 = plan.tempo.bpm_at_frame(0);

        // Channels (plan order = sorted channel IDs).
        let mut channels = Vec::with_capacity(plan.channels.len());
        let mut latencies = Vec::with_capacity(plan.channels.len());
        for channel_plan in &plan.channels {
            let (instrument, instrument_param_ids, instrument_specs, instrument_initial) =
                create_instrument(
                    channel_plan,
                    &host,
                    registry,
                    samples,
                    &plan.tempo,
                    host.sample_rate,
                    max_block,
                )?;
            let slicer_tempo = crate::slicer_tempo::SlicerTempo::resolve(
                &channel_plan.instrument,
                samples,
                &plan.tempo,
                &instrument_param_ids,
            )?;
            let mut inserts = Vec::with_capacity(channel_plan.effect_chain.len());
            for (index, effect) in channel_plan.effect_chain.iter().enumerate() {
                inserts.push(create_insert(
                    &channel_plan.id,
                    index,
                    effect,
                    &host,
                    registry,
                    host.sample_rate,
                    max_block,
                )?);
            }
            let mut latency = instrument.latency_frames();
            for insert in &inserts {
                latency += insert.instance.latency_frames();
            }
            latencies.push(latency);
            let mut level = oxitone_dsp::gain_pan::OnePoleSmoother::new(host.sample_rate, 20.0);
            level.snap(channel_plan.level as f32);
            let mut pan = oxitone_dsp::gain_pan::OnePoleSmoother::new(host.sample_rate, 20.0);
            pan.snap(channel_plan.pan as f32);
            channels.push(ChannelNode {
                id: channel_plan.id.clone(),
                bus_id: channel_plan.mixer_channel_id.clone(),
                instrument,
                instrument_param_ids,
                instrument_specs: instrument_specs.clone(),
                instrument_initial,
                slicer_tempo,
                inserts,
                level,
                pan,
                mute: channel_plan.mute,
                solo: channel_plan.solo,
                swing: channel_plan.swing,
                comp: DelayLine::new(0, max_block as usize),
                dry_l: vec![0.0; max_block as usize],
                dry_r: vec![0.0; max_block as usize],
                wet_l: vec![0.0; max_block as usize],
                wet_r: vec![0.0; max_block as usize],
                out_l: vec![0.0; max_block as usize],
                out_r: vec![0.0; max_block as usize],
                delayed_l: vec![0.0; max_block as usize],
                delayed_r: vec![0.0; max_block as usize],
                notes: Vec::with_capacity(plan.scheduler.events_in_range(0, u64::MAX).len()),
                instrument_staged: oxitone_mixer::parameter_queue::ParameterQueue::new(
                    &instrument_specs,
                ),
                first_block: true,
                note_shift: [0; 128],
                note_active: [false; 128],
            });
        }
        // Uniform channel-path PDC: every channel is delayed so all paths
        // share the longest channel-chain latency (02-domain-spec.md §PDC).
        let channel_latency_max = latencies.iter().copied().max().unwrap_or(0);
        for (channel, &latency) in channels.iter_mut().zip(latencies.iter()) {
            channel.comp = DelayLine::new(channel_latency_max as usize, max_block as usize);
            channel
                .comp
                .set_delay((channel_latency_max - latency) as usize);
        }

        // Track membership per channel (for track stems).
        let mut channel_tracks: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut tracks: Vec<&_> = snapshot.tracks.iter().collect();
        tracks.sort_by(|a, b| a.id.cmp(&b.id));
        for track in tracks {
            let mut channel_ids: Vec<&str> = track.channel_ids.iter().map(String::as_str).collect();
            channel_ids.sort_unstable();
            for id in channel_ids {
                channel_tracks
                    .entry(id.to_string())
                    .or_default()
                    .push(track.id.clone());
            }
        }

        // Mixer with beat-unit preprocessing.
        let mut mixer_beat_params = Vec::new();
        let mixer_specs: Vec<MixerChannelSpec> = snapshot
            .mixer_channels
            .iter()
            .map(|spec| preprocess_mixer_channel(spec, registry, bpm0, &mut mixer_beat_params))
            .collect::<Result<_, _>>()?;
        let mut mixer = MixerEngine::build(host.sample_rate, max_block, &mixer_specs, registry)?;
        mixer.set_respect_solo(options.respect_solo);

        // Declared send routes (for host `setParameter` validation of
        // `send.<destinationId>.ratio` targets).
        let mut mixer_sends = BTreeSet::new();
        for spec in &mixer_specs {
            for send in &spec.sends {
                mixer_sends.insert((spec.id.clone(), send.destination_id.clone()));
            }
        }

        // Sample clips.
        let clips: Vec<ClipNode> = plan
            .sample_clips
            .iter()
            .map(|clip| ClipNode::new(clip, sample_rate, max_block as usize))
            .collect();

        // Master protection limiter.
        let mut limiter_latency = 0;
        let limiter = if options.master_limiter {
            let plugin = registry
                .lookup(MASTER_LIMITER_PLUGIN_ID, MASTER_LIMITER_VERSION)
                .ok_or_else(|| {
                    invalid("master limiter plugin is not registered", "$.engineOptions")
                })?;
            let mut instance = plugin.create(&host);
            instance.prepare(host.sample_rate, max_block);
            limiter_latency = instance.latency_frames();
            Some(instance)
        } else {
            None
        };

        let metronome = options
            .metronome_level
            .map(|level| Metronome::new(&plan, level));

        Ok(RenderGraph::new(
            plan,
            channels,
            channel_tracks,
            clips,
            mixer,
            mixer_beat_params,
            mixer_sends,
            limiter,
            limiter_latency,
            channel_latency_max,
            metronome,
            options.respect_solo,
        ))
    }
}
