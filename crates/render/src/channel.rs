//! Channel runtime node: instrument instance, insert chain with built-in
//! `mix`/`bypass` (02-domain-spec.md §Mixer: 每个 insert 节点暴露内建
//! mix/bypass), fader/pan, PDC compensation
//! delay, and per-block event staging. All buffers are preallocated; the
//! per-block path performs no heap allocation (parameter events use
//! preallocated descriptor-sized storage for insert events).

use std::sync::Arc;

use oxitone_core::wire::ParameterSpec;
use oxitone_dsp::gain_pan::{equal_power_gains, OnePoleSmoother};
use oxitone_graph::abi::{NoteEvent, ParameterEvent, PluginInstance, ProcessContext};
use oxitone_mixer::DelayLine;

/// Max staged parameter events per instance per block (control rate: a few
/// automation updates plus initial values and beat-unit conversions).
pub const MAX_PARAM_EVENTS: usize = 64;

/// Build an ABI parameter-event slice from staged `(offset, index, value)`
/// tuples. Stack array, no allocation (mirrors `oxitone-mixer`'s
/// pending-event path).
pub fn build_events<'a>(
    staged: &[(u32, usize, f64)],
    ids: &'a [String],
    buf: &'a mut [ParameterEvent<'a>; MAX_PARAM_EVENTS],
) -> &'a [ParameterEvent<'a>] {
    let count = staged.len().min(MAX_PARAM_EVENTS);
    for (slot, &(offset, index, value)) in buf.iter_mut().zip(staged.iter()) {
        *slot = ParameterEvent {
            frame_offset: offset,
            parameter_id: ids.get(index).map(String::as_str).unwrap_or(""),
            value,
        };
    }
    &buf[..count]
}

/// Fresh empty stack event buffer.
pub fn event_buf<'a>() -> [ParameterEvent<'a>; MAX_PARAM_EVENTS] {
    [ParameterEvent {
        frame_offset: 0,
        parameter_id: "",
        value: 0.0,
    }; MAX_PARAM_EVENTS]
}

pub use crate::insert::{BeatParam, InsertNode};

/// One arrangement channel with its instrument and insert chain.
pub struct ChannelNode {
    pub id: String,
    pub bus_id: String,
    pub instrument: Box<dyn PluginInstance>,
    pub instrument_param_ids: Arc<Vec<String>>,
    /// Descriptor parameter specs aligned with `instrument_param_ids`
    /// (host `setParameter` validation and range checks).
    pub instrument_specs: Arc<Vec<ParameterSpec>>,
    /// Third-party instrument initial values (built-ins receive their
    /// config at creation via `create_builtin_instance`).
    pub instrument_initial: Vec<(usize, f64)>,
    pub inserts: Vec<InsertNode>,
    pub level: OnePoleSmoother,
    pub pan: OnePoleSmoother,
    pub mute: bool,
    pub solo: bool,
    /// Effective swing for dispatch (static or automated control value).
    pub swing: f64,
    /// PDC compensation: delays this channel so every channel path has the
    /// same total latency (uniform alignment to the longest channel chain).
    pub comp: DelayLine,
    pub dry_l: Vec<f32>,
    pub dry_r: Vec<f32>,
    pub wet_l: Vec<f32>,
    pub wet_r: Vec<f32>,
    pub out_l: Vec<f32>,
    pub out_r: Vec<f32>,
    pub delayed_l: Vec<f32>,
    pub delayed_r: Vec<f32>,
    /// Staged note events for the current block (sorted by frame offset).
    pub notes: Vec<NoteEvent>,
    /// Staged instrument parameter events: (frame_offset, param index, value).
    pub instrument_staged: Vec<(u32, usize, f64)>,
    pub first_block: bool,
    /// Note-on shift per pitch for swing dispatch (frames; note-offs reuse
    /// the shift of their note-on so durations survive swing).
    pub note_shift: [i64; 128],
    pub note_active: [bool; 128],
}

impl ChannelNode {
    /// Stage initial instrument values and insert control-rate events.
    pub fn stage_initial(&mut self) {
        if self.first_block {
            for &(index, value) in &self.instrument_initial {
                if self.instrument_staged.len() < MAX_PARAM_EVENTS {
                    self.instrument_staged.push((0, index, value));
                }
            }
            self.first_block = false;
        }
        for insert in &mut self.inserts {
            insert.stage_initial();
        }
    }

    /// Phase 1: run the instrument into `dry_l`/`dry_r` (overwrite).
    /// Sample-clip contributions are added onto these buffers by the
    /// renderer before [`ChannelNode::process_chain`]. RT-safe.
    pub fn process_instrument(&mut self, frames: usize, sample_rate: f64) {
        {
            let ids = self.instrument_param_ids.clone();
            let mut buf = event_buf();
            let events = build_events(&self.instrument_staged, &ids, &mut buf);
            let notes = std::mem::take(&mut self.notes);
            let mut outputs: [&mut [f32]; 2] =
                [&mut self.dry_l[..frames], &mut self.dry_r[..frames]];
            let mut ctx = ProcessContext {
                frames,
                sample_rate,
                inputs: &[],
                outputs: &mut outputs,
                note_events: &notes,
                parameter_events: events,
                sidechain: None,
            };
            self.instrument.process(&mut ctx);
            self.notes = notes;
            self.notes.clear();
        }
        self.instrument_staged.clear();
    }

    /// Phase 2: inserts (mix/bypass) → fader/pan/mute into
    /// `out_l`/`out_r`, then the compensation delay into
    /// `delayed_l`/`delayed_r`. RT-safe.
    pub fn process_chain(&mut self, frames: usize, sample_rate: f64, audible: bool) {
        for insert in &mut self.inserts {
            {
                {
                    insert.staged.with_events(|events| {
                        let inputs: [&[f32]; 2] = [&self.dry_l[..frames], &self.dry_r[..frames]];
                        let mut outputs: [&mut [f32]; 2] =
                            [&mut self.wet_l[..frames], &mut self.wet_r[..frames]];
                        let mut ctx = ProcessContext {
                            frames,
                            sample_rate,
                            inputs: &inputs,
                            outputs: &mut outputs,
                            note_events: &[],
                            parameter_events: events,
                            sidechain: None,
                        };
                        insert.instance.process(&mut ctx);
                    });
                }
                insert.delayed_l[..frames].fill(0.0);
                insert.delayed_r[..frames].fill(0.0);
                insert.dry_delay.process_add(
                    &self.dry_l[..frames],
                    &self.dry_r[..frames],
                    1.0,
                    &mut insert.delayed_l[..frames],
                    &mut insert.delayed_r[..frames],
                );
                // Preserve the plugin latency in both dry and bypass paths.
                for n in 0..frames {
                    let smoothed = insert.mix.next_sample();
                    let mix = if insert.bypass { 0.0 } else { smoothed };
                    self.dry_l[n] = insert.delayed_l[n] * (1.0 - mix) + self.wet_l[n] * mix;
                    self.dry_r[n] = insert.delayed_r[n] * (1.0 - mix) + self.wet_r[n] * mix;
                }
            }
        }

        // Fader: smoothed level, smoothed equal-power pan, mute/solo gate.
        for n in 0..frames {
            let level = self.level.next_sample();
            let (gain_l, gain_r) = equal_power_gains(self.pan.next_sample());
            let gain = if audible { level } else { 0.0 };
            self.out_l[n] = self.dry_l[n] * gain * gain_l;
            self.out_r[n] = self.dry_r[n] * gain * gain_r;
        }

        for slot in &mut self.delayed_l[..frames] {
            *slot = 0.0;
        }
        for slot in &mut self.delayed_r[..frames] {
            *slot = 0.0;
        }
        self.comp.process_add(
            &self.out_l[..frames],
            &self.out_r[..frames],
            1.0,
            &mut self.delayed_l[..frames],
            &mut self.delayed_r[..frames],
        );
    }
}
