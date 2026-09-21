//! RenderGraph: every runtime object assembled from a
//! `oxitone_graph::RenderPlan` — instruments, channel insert chains, sample
//! clip players, the mixer engine, the master limiter and the metronome.
//! `process_block` follows the DSP order of 03-audio-runtime-spec.md:
//! transport advance → event dispatch → instrument render → clip mix →
//! channel inserts → channel fader/pan → mixer buses → metronome (pre-
//! limiter) → master limiter → meter/NaN guard.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use oxitone_dsp::gain_pan::equal_power_gains;
use oxitone_graph::abi::PluginInstance;
use oxitone_graph::compile::{binding_value_at, RenderPlan};
use oxitone_mixer::{ChannelInput, MixerEngine};
use oxitone_transport::EvalContext;

use crate::bindings::{apply_bindings, resolve_bindings, RtBinding};
use crate::channel::ChannelNode;
use crate::clip::ClipNode;
use crate::dispatch::Dispatcher;
use crate::metronome::Metronome;
use crate::params::QueuedParameterEvent;
use crate::transport::{Transport, TransportState};

/// Beat-unit insert parameter converted by the renderer for a mixer bus
/// insert (channel inserts keep their state inside `InsertNode`).
pub struct MixerBeatParam {
    pub bus: String,
    pub bus_index: usize,
    pub insert: usize,
    pub state: crate::channel::BeatParam,
}

mod channels;
mod initialize;
mod position;
/// Compiled, playable/renderable graph. `process_block` is RT-safe: every
/// buffer was preallocated at compile; no allocation, no locks, no I/O.
mod process;
mod segment;

pub struct RenderGraph {
    pub(crate) controls: crate::plugin_controls::ControlGraph,
    isolated: bool,
    continuous_frame: u64,
    pub(crate) preview: Option<std::sync::Arc<crate::preview::PreviewTelemetry>>,
    pub(crate) plan: RenderPlan,
    pub(crate) sample_rate: f64,
    pub(crate) block_size: usize,
    pub(crate) channels: Vec<ChannelNode>,
    pub(crate) channel_index: BTreeMap<String, usize>,
    /// Track IDs per channel ID (sorted), for track-stem export.
    pub(crate) channel_tracks: BTreeMap<String, Vec<String>>,
    pub(crate) clips: Vec<ClipNode>,
    pub(crate) mixer: MixerEngine,
    pub(crate) mixer_beat_params: Vec<MixerBeatParam>,
    /// Declared send routes `(source bus, destination bus)` from the
    /// compiled specs (host `setParameter` target validation).
    pub(crate) mixer_sends: BTreeSet<(String, String)>,
    pub(crate) effect_targets: crate::effect_targets::EffectTargetIndex,
    pub(crate) limiter: Option<Box<dyn PluginInstance>>,
    pub(crate) metronome: Option<Metronome>,
    pub(crate) transport: Transport,
    dispatcher: Dispatcher,
    pub(crate) rt_bindings: Vec<RtBinding>,
    /// Host `setParameter` events, sorted by target frame; drained at
    /// control rate in `process_block` (frame <= block start).
    pub(crate) param_queue: VecDeque<QueuedParameterEvent>,
    respect_solo: bool,
    pub(crate) eval_ctx: EvalContext,
    faulted: bool,
    graph_latency: u64,
    master_l: Vec<f32>,
    master_r: Vec<f32>,
    limited_l: Vec<f32>,
    limited_r: Vec<f32>,
    metro_block_l: Vec<f32>,
    metro_block_r: Vec<f32>,
    metro_l: Vec<f32>,
    metro_r: Vec<f32>,
    clip_l: Vec<f32>,
    clip_r: Vec<f32>,
    clip_gain: Vec<f32>,
    clip_pan: Vec<f32>,
    midi_clip_l: Vec<f32>,
    midi_clip_r: Vec<f32>,
}

impl RenderGraph {
    pub fn plan(&self) -> &RenderPlan {
        &self.plan
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut Transport {
        &mut self.transport
    }

    /// PDC + channel-alignment + master-limiter latency, in frames.
    pub fn graph_latency_frames(&self) -> u64 {
        self.graph_latency
    }

    /// Set once after a non-finite sample was detected (the block was
    /// muted; the renderer never panics).
    pub fn faulted(&self) -> bool {
        self.faulted
    }

    pub fn mixer(&self) -> &MixerEngine {
        &self.mixer
    }

    pub fn mixer_mut(&mut self) -> &mut MixerEngine {
        &mut self.mixer
    }

    /// Last block's metronome-only contribution (for stem export).
    pub fn metronome_output(&self) -> (&[f32], &[f32]) {
        (&self.metro_block_l, &self.metro_block_r)
    }

    /// Bus IDs whose channels belong to `track_id` (sorted, deduped).
    pub fn buses_of_track(&self, track_id: &str) -> Vec<String> {
        let mut buses: Vec<String> = self
            .plan
            .channels
            .iter()
            .filter(|channel| {
                self.channel_tracks
                    .get(&channel.id)
                    .map(|tracks| tracks.iter().any(|t| t == track_id))
                    .unwrap_or(false)
            })
            .map(|channel| channel.mixer_channel_id.clone())
            .collect();
        buses.sort();
        buses.dedup();
        buses
    }

    /// Seek: move the cursor and flush all voices/DSP state
    /// (02-domain-spec.md §Playback: seek 会 flush voices).
    pub fn seek(&mut self, frame: u64) {
        if let Some(preview) = &self.preview {
            preview.reset();
        }
        self.transport.cursor = frame;
        self.eval_ctx = EvalContext {
            origin_beat: self.plan.tempo.frame_to_beat(frame).to_f64(),
            loop_iteration: 0,
        };
        for channel in &mut self.channels {
            channel.instrument.reset();
            for insert in &mut channel.inserts {
                insert.instance.reset();
                insert.dry_delay.reset();
            }
            channel.comp.reset();
            for route in &mut channel.output_routes {
                route.reset();
            }
            channel.notes.clear();
            channel.instrument_staged.clear();
            channel.note_active = [false; 128];
            channel.note_shift = [0; 128];
        }
        for clip in &mut self.clips {
            clip.reset();
        }
        self.mixer.reset();
        if let Some(limiter) = &mut self.limiter {
            limiter.reset();
        }
        if let Some(metronome) = &mut self.metronome {
            metronome.seek(&self.plan, frame);
        }
    }

    /// Current transport state.
    pub fn state(&self) -> TransportState {
        self.transport.state
    }

    /// Project sample rate the graph was compiled at.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Compiled block size in frames.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Channel index with the most active voices (best-effort load
    /// attribution for underrun diagnostics; worker-thread only).
    pub fn busiest_channel(&self) -> Option<usize> {
        self.channels
            .iter()
            .enumerate()
            .map(|(index, channel)| (index, channel.note_active.iter().filter(|&&on| on).count()))
            .max_by_key(|&(_, active)| active)
            .map(|(index, _)| index)
    }

    /// Channel IDs in compiled order (diagnostic attribution labels).
    pub fn channel_ids(&self) -> Vec<String> {
        self.channels.iter().map(|c| c.id.clone()).collect()
    }
}
