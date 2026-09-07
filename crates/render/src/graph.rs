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

/// Compiled, playable/renderable graph. `process_block` is RT-safe: every
/// buffer was preallocated at compile; no allocation, no locks, no I/O.
pub struct RenderGraph {
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
}

impl RenderGraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        plan: RenderPlan,
        channels: Vec<ChannelNode>,
        channel_tracks: BTreeMap<String, Vec<String>>,
        clips: Vec<ClipNode>,
        mixer: MixerEngine,
        mixer_beat_params: Vec<MixerBeatParam>,
        mixer_sends: BTreeSet<(String, String)>,
        limiter: Option<Box<dyn PluginInstance>>,
        limiter_latency: u64,
        channel_latency_max: u64,
        metronome: Option<Metronome>,
        respect_solo: bool,
    ) -> Self {
        let block_size = plan.block_size as usize;
        let sample_rate = f64::from(plan.sample_rate);
        let channel_index = channels
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i))
            .collect();
        let graph_latency = mixer.graph_latency_frames() + channel_latency_max + limiter_latency;
        let dispatcher = Dispatcher::new(
            sample_rate as u32,
            plan.scheduler.events_in_range(0, u64::MAX).len(),
        );
        let mut graph = Self {
            plan,
            sample_rate,
            block_size,
            channels,
            channel_index,
            channel_tracks,
            clips,
            mixer,
            mixer_beat_params,
            mixer_sends,
            effect_targets: Default::default(),
            limiter,
            metronome,
            transport: Transport::new(),
            dispatcher,
            rt_bindings: Vec::new(),
            param_queue: VecDeque::with_capacity(4096),
            respect_solo,
            eval_ctx: EvalContext::default(),
            faulted: false,
            graph_latency,
            master_l: vec![0.0; block_size],
            master_r: vec![0.0; block_size],
            limited_l: vec![0.0; block_size],
            limited_r: vec![0.0; block_size],
            metro_block_l: vec![0.0; block_size],
            metro_block_r: vec![0.0; block_size],
            metro_l: vec![0.0; block_size],
            metro_r: vec![0.0; block_size],
            clip_l: vec![0.0; block_size],
            clip_r: vec![0.0; block_size],
            clip_gain: vec![0.0; block_size],
            clip_pan: vec![0.0; block_size],
        };
        for beat in &mut graph.mixer_beat_params {
            beat.bus_index = graph
                .mixer
                .bus_index(&beat.bus)
                .expect("validated mixer bus");
        }
        graph.effect_targets = crate::effect_targets::EffectTargetIndex::from_graph(&graph);
        graph.rt_bindings = resolve_bindings(&graph);
        graph
    }

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

    /// Render one block at the transport cursor into `out_l`/`out_r`
    /// (overwrite; `out` length must equal the compiled block size). Silent
    /// blocks are produced while stopped/paused without advancing the
    /// cursor. RT-safe: no allocation, no locks, no I/O.
    pub fn process_block(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let frames = out_l.len().min(out_r.len()).min(self.block_size);
        out_l.fill(0.0);
        out_r.fill(0.0);
        if !self.transport.running() {
            return;
        }
        let mut offset = 0;
        while offset < frames {
            let cur = self.transport.cursor;
            let mut count = frames - offset;
            if self.transport.state == TransportState::Playing {
                if let Some((start, end)) = self.transport.loop_region {
                    if end > start {
                        if cur >= end {
                            self.seek(start);
                            continue;
                        }
                        count = count.min((end - cur) as usize);
                    }
                }
            }
            if let Some(event) = self.param_queue.iter().find(|e| e.frame > cur) {
                count = count.min(event.frame.saturating_sub(cur).min(count as u64) as usize);
            }
            for n in 1..count {
                if self
                    .rt_bindings
                    .iter()
                    .any(|b| crate::bindings::binding_due(self, b, cur + n as u64))
                {
                    count = n;
                    break;
                }
            }
            self.process_segment(
                &mut out_l[offset..offset + count],
                &mut out_r[offset..offset + count],
                offset == 0,
            );
            self.mixer.capture_stem_segment(offset, count);
            self.metro_block_l[offset..offset + count].copy_from_slice(&self.metro_l[..count]);
            self.metro_block_r[offset..offset + count].copy_from_slice(&self.metro_r[..count]);
            if self.transport.cursor < cur + count as u64 {
                let iteration = self.eval_ctx.loop_iteration.wrapping_add(1);
                self.seek(self.transport.cursor);
                self.eval_ctx.loop_iteration = iteration;
            }
            offset += count;
        }
    }

    fn process_segment(&mut self, out_l: &mut [f32], out_r: &mut [f32], first: bool) {
        let frames = out_l.len().min(self.block_size);
        if !self.transport.running() {
            for slot in out_l.iter_mut() {
                *slot = 0.0;
            }
            for slot in out_r.iter_mut() {
                *slot = 0.0;
            }
            return;
        }
        let cur = self.transport.cursor;
        let end = cur + frames as u64;
        let beat = self.plan.tempo.frame_to_beat(cur).to_f64();
        let bpm = self.plan.tempo.bpm_at_frame(cur);

        for channel in &mut self.channels {
            channel.stage_initial();
            if let Some(sync) = &channel.slicer_tempo {
                channel
                    .instrument_staged
                    .set(sync.parameter_index, sync.factor(bpm));
            }
        }

        // Host parameter events due at or before this block's start
        // (control rate, ahead of automation application).
        while let Some(event) = self.param_queue.front() {
            if event.frame > cur {
                break;
            }
            let event = self.param_queue.pop_front().expect("front checked");
            crate::bindings::apply_rt_target(self, &event.target, event.value);
        }

        // Control-rate automation and parameter staging.
        apply_bindings(self, beat, first);
        for channel in &mut self.channels {
            for insert in &mut channel.inserts {
                insert.stage_tempo(bpm);
            }
        }
        for beat_param in &mut self.mixer_beat_params {
            if let Some((index, seconds)) = beat_param.state.poll(bpm) {
                self.mixer.set_insert_parameter_at(
                    beat_param.bus_index,
                    beat_param.insert,
                    index,
                    seconds,
                );
            }
        }

        // Note dispatch (swing-aware) and instrument render.
        let eval_ctx = self.eval_ctx;
        let plan = &self.plan;
        self.dispatcher.dispatch(
            plan,
            &mut self.channels,
            &self.channel_index,
            cur,
            end,
            |channel, event_beat| match plan.channels[channel].swing_binding {
                Some(binding) => binding_value_at(&plan.bindings[binding], event_beat, &eval_ctx),
                None => plan.channels[channel].swing,
            },
        );
        for channel in &mut self.channels {
            channel.process_instrument(frames, self.sample_rate);
        }

        // Sample clips: render dry, tone tilt, then mix into each owning
        // channel's pre-insert buffer with smoothed level/gain/pan.
        let sample_rate = self.sample_rate;
        let tempo = &self.plan.tempo;
        let RenderGraph {
            clips,
            channels,
            clip_l,
            clip_r,
            clip_gain,
            clip_pan,
            ..
        } = self;
        for clip in clips.iter_mut() {
            if !clip.render(cur, frames, tempo, clip_l, clip_r) {
                continue;
            }
            clip.apply_tone(&mut clip_l[..frames], &mut clip_r[..frames]);
            for i in 0..frames {
                clip_gain[i] = clip.level.next_sample() * clip.gain.next_sample();
                clip_pan[i] = clip.pan.next_sample();
            }
            for &ch in &clip.channels {
                let node = &mut channels[ch];
                for i in 0..frames {
                    let (gain_l, gain_r) = equal_power_gains(clip_pan[i]);
                    node.dry_l[i] += clip_l[i] * clip_gain[i] * gain_l;
                    node.dry_r[i] += clip_r[i] * clip_gain[i] * gain_r;
                }
            }
        }

        // Channel inserts + fader/pan (+PDC compensation).
        let any_channel_solo = self.respect_solo && self.channels.iter().any(|c| c.solo);
        for channel in &mut self.channels {
            let audible = !(channel.mute || (any_channel_solo && !channel.solo));
            channel.process_chain(frames, sample_rate, audible);
        }

        // Mixer buses (inputs in channel order: part of the deterministic
        // summation order).
        let inputs = self.channels.iter().map(|channel| ChannelInput {
            bus_id: &channel.bus_id,
            left: &channel.delayed_l[..frames],
            right: &channel.delayed_r[..frames],
        });
        self.mixer.process_block_with(
            inputs,
            &mut self.master_l[..frames],
            &mut self.master_r[..frames],
        );

        // Metronome (pre-limiter), then the master protection limiter.
        if let Some(metronome) = &mut self.metronome {
            for slot in &mut self.metro_l[..frames] {
                *slot = 0.0;
            }
            for slot in &mut self.metro_r[..frames] {
                *slot = 0.0;
            }
            let (metro_l, metro_r, master_l, master_r) = (
                &mut self.metro_l,
                &mut self.metro_r,
                &mut self.master_l,
                &mut self.master_r,
            );
            metronome.render_add(
                &self.plan,
                cur,
                &mut metro_l[..frames],
                &mut metro_r[..frames],
            );
            for i in 0..frames {
                master_l[i] += metro_l[i];
                master_r[i] += metro_r[i];
            }
        }
        match &mut self.limiter {
            Some(limiter) => {
                let inputs: [&[f32]; 2] = [&self.master_l[..frames], &self.master_r[..frames]];
                let mut outputs: [&mut [f32]; 2] =
                    [&mut self.limited_l[..frames], &mut self.limited_r[..frames]];
                let mut ctx = oxitone_graph::ProcessContext {
                    frames,
                    sample_rate,
                    inputs: &inputs,
                    outputs: &mut outputs,
                    note_events: &[],
                    parameter_events: &[],
                    sidechain: None,
                };
                limiter.process(&mut ctx);
            }
            None => {
                self.limited_l[..frames].copy_from_slice(&self.master_l[..frames]);
                self.limited_r[..frames].copy_from_slice(&self.master_r[..frames]);
            }
        }

        // NaN/Inf guard: mute the block and latch the fault flag.
        let clean = self.limited_l[..frames]
            .iter()
            .chain(self.limited_r[..frames].iter())
            .all(|x| x.is_finite());
        if clean {
            out_l[..frames].copy_from_slice(&self.limited_l[..frames]);
            out_r[..frames].copy_from_slice(&self.limited_r[..frames]);
        } else {
            self.faulted = true;
            for slot in out_l.iter_mut() {
                *slot = 0.0;
            }
            for slot in out_r.iter_mut() {
                *slot = 0.0;
            }
        }

        self.transport.advance(frames as u64);
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
