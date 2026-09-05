//! Slicer plugin instance: one preallocated varispeed voice per slice
//! (03-audio-runtime-spec.md §Slicer). Note pitch selects the slice relative
//! to `triggerNote`; oneshot ignores note-off, gate releases on note-off.
//! `process` is allocation-free.

use std::sync::Arc;

use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::abi::{NoteEventKind, PluginInstance, ProcessContext};
use oxitone_samples::PreparedSample;

use super::params as p;
use super::state::{PlayMode, ResolvedSlice, MAX_SLICES};
use crate::sample_voice::{SampleVoicePool, VoiceStart, MAX_RATE, MIN_RATE};

const SMOOTH_MS: f64 = 5.0;
/// Fixed slice envelope: 1 ms click guard into full sustain, 5 ms release
/// for gate mode (02-domain-spec.md §内置 Slicer).
const SLICE_ADSR: (f64, f64, f32, f64) = (0.001, 0.0, 1.0, 0.005);

pub struct SlicerInstance {
    sample_rate: f64,
    max_block: usize,
    values: Vec<f64>,
    sample: Option<Arc<PreparedSample>>,
    slices: Vec<ResolvedSlice>,
    trigger_note: i32,
    play_mode: PlayMode,
    pool: SampleVoicePool,
    /// Slice index → pool slot (-1 = not playing).
    slot_for_slice: [i16; MAX_SLICES],
    level: OnePoleSmoother,
    pan: OnePoleSmoother,
    zeros: Vec<f32>,
}

impl SlicerInstance {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sample_rate: f64,
        max_block: usize,
        values: Vec<f64>,
        sample: Option<Arc<PreparedSample>>,
        slices: Vec<ResolvedSlice>,
        trigger_note: u8,
        play_mode: PlayMode,
    ) -> Self {
        let voices = slices.len().max(1);
        let mut instance = Self {
            sample_rate,
            max_block,
            values,
            sample,
            slices,
            trigger_note: i32::from(trigger_note),
            play_mode,
            pool: SampleVoicePool::new(voices, sample_rate, max_block),
            slot_for_slice: [-1; MAX_SLICES],
            level: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            pan: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            zeros: vec![0.0; max_block],
        };
        instance.snap_smoothers();
        instance
    }

    fn snap_smoothers(&mut self) {
        self.level.snap(self.values[p::LEVEL] as f32);
        self.pan.snap(self.values[p::PAN] as f32);
    }

    fn set_parameter(&mut self, index: usize, value: f64) {
        self.values[index] = value;
        match index {
            p::LEVEL => self.level.set_target(value as f32),
            p::PAN => self.pan.set_target(value as f32),
            p::TEMPO_FACTOR => {
                for i in 0..self.pool.len() {
                    if self.pool.voice(i).active {
                        let base_rate = self
                            .slot_slice(i)
                            .map(|s| self.slices[s].rate)
                            .unwrap_or(1.0);
                        self.pool.voice_mut(i).set_rate(base_rate * value);
                    }
                }
            }
            _ => {}
        }
    }

    /// Slice index currently mapped to a pool slot, if any.
    fn slot_slice(&self, slot: usize) -> Option<usize> {
        self.slot_for_slice.iter().position(|&s| s == slot as i16)
    }

    fn note_on(&mut self, pitch: u8) {
        let index = i32::from(pitch) - self.trigger_note;
        if index < 0 || index as usize >= self.slices.len() {
            return; // 超出 slice 数的 note 不发声
        }
        let index = index as usize;
        let slice = self.slices[index];
        let rate = (slice.rate * self.values[p::TEMPO_FACTOR]).clamp(MIN_RATE, MAX_RATE);
        let slot = match self.slot_for_slice[index] {
            s if s >= 0 && self.pool.voice(s as usize).active => s as usize,
            _ => {
                let slot = self.pool.allocate(slice.level);
                self.slot_for_slice[index] = slot as i16;
                slot
            }
        };
        self.pool.voice_mut(slot).start(VoiceStart {
            note: pitch,
            rate,
            base: slice.start_frame,
            stream_len: slice.end_frame - slice.start_frame,
            loop_region: None,
            reverse: slice.reverse,
            gain: slice.level,
            pan: slice.pan,
            adsr: SLICE_ADSR,
        });
    }

    fn note_off(&mut self, pitch: u8) {
        if self.play_mode == PlayMode::Oneshot {
            return; // oneshot 忽略 note-off，播到 slice 末
        }
        let index = i32::from(pitch) - self.trigger_note;
        if index < 0 || index as usize >= self.slices.len() {
            return;
        }
        let slot = self.slot_for_slice[index as usize];
        if slot >= 0 && self.pool.voice(slot as usize).active {
            self.pool.voice_mut(slot as usize).amp.note_off();
        }
    }

    fn render_segment(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let frames = out_l.len();
        let Some(sample) = self.sample.as_deref() else {
            return;
        };
        let chans: [&[f32]; 2] = [
            &sample.channels[0],
            sample.channels.get(1).unwrap_or(&sample.channels[0]),
        ];
        for i in 0..self.pool.len() {
            if !self.pool.voice(i).active {
                continue;
            }
            let voice = self.pool.voice_mut(i);
            voice.render(&chans, &self.zeros, out_l, out_r);
            voice.level = voice.amp.level() * voice.gain();
            if voice.is_drained() || !voice.amp.is_active() {
                self.pool.release(i);
                if let Some(slice) = self.slot_slice(i) {
                    self.slot_for_slice[slice] = -1;
                }
            }
        }
        // Global level/pan after the voice sum; `pan` is a stereo balance
        // (unity at center), slice pans already panned mono→stereo.
        let mut k = 0usize;
        while k < frames {
            let end = (k + 32).min(frames);
            let pan = self.pan.value();
            let (gl, gr) = (1.0 - pan.max(0.0), 1.0 + pan.min(0.0));
            for j in k..end {
                let gain = self.level.next_sample();
                self.pan.next_sample();
                out_l[j] *= gain * gl;
                out_r[j] *= gain * gr;
            }
            k = end;
        }
    }
}

impl PluginInstance for SlicerInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        let max_block = max_block_size as usize;
        if sample_rate != self.sample_rate || max_block != self.max_block {
            self.sample_rate = sample_rate;
            self.max_block = max_block;
            self.pool = SampleVoicePool::new(self.slices.len().max(1), sample_rate, max_block);
            self.slot_for_slice = [-1; MAX_SLICES];
            self.level = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.pan = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.zeros = vec![0.0; max_block];
        }
        self.snap_smoothers();
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        let frames = ctx.frames.min(self.max_block);
        let (out_l, out_r) = ctx.outputs.split_at_mut(1);
        let out_l = &mut out_l[0][..frames];
        let out_r = &mut out_r[0][..frames];
        out_l.fill(0.0);
        out_r.fill(0.0);
        if self.sample.is_none() {
            return;
        }
        let specs = super::parameter_specs();
        crate::block::walk_block(frames, ctx.note_events, ctx.parameter_events, |step| {
            use crate::block::Walk;
            match step {
                Walk::Parameters(events) => {
                    for event in events {
                        if let Some(index) = crate::params::index_of(specs, event.parameter_id) {
                            let value =
                                crate::params::sanitize_event_value(&specs[index], event.value);
                            self.set_parameter(index, value);
                        }
                    }
                }
                Walk::Notes(notes) => {
                    for note in notes {
                        match note.kind {
                            NoteEventKind::NoteOn => self.note_on(note.pitch),
                            NoteEventKind::NoteOff => self.note_off(note.pitch),
                        }
                    }
                }
                Walk::Render(offset, len) => self.render_segment(
                    &mut out_l[offset..offset + len],
                    &mut out_r[offset..offset + len],
                ),
            }
        });
    }

    fn reset(&mut self) {
        self.pool.reset();
        self.slot_for_slice = [-1; MAX_SLICES];
        self.snap_smoothers();
    }

    fn tail_frames(&self) -> u64 {
        if self.pool.active_count() == 0 {
            return 0;
        }
        let release = (SLICE_ADSR.3 * self.sample_rate) as u64;
        let mut longest = 0u64;
        for i in 0..self.pool.len() {
            let voice = self.pool.voice(i);
            if !voice.active {
                continue;
            }
            let stream = (voice.remaining_input_frames() as f64 / voice.rate()) as u64;
            longest = longest.max(stream);
        }
        longest + release + 2
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
