//! Sampler plugin instance: single-sample varispeed instrument. Note pitch
//! relative to `rootKey` sets the playback rate (`2^((pitch-rootKey)/12)`),
//! velocity maps to level through `velocitySensitivity` as
//! `(1 - s) + s * velocity`. Loop mode `forward` uses the prepared sample's
//! loop points. `process` is allocation-free.

use std::sync::Arc;

use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::abi::{NoteEventKind, PluginInstance, ProcessContext};
use oxitone_samples::PreparedSample;

use super::params as p;
use crate::sample_voice::{SampleVoicePool, VoiceStart, MAX_RATE, MIN_RATE};

const MAX_VOICES: usize = 32;
const SMOOTH_MS: f64 = 5.0;

pub struct SamplerInstance {
    sample_rate: f64,
    max_block: usize,
    values: Vec<f64>,
    sample: Option<Arc<PreparedSample>>,
    pool: SampleVoicePool,
    level: OnePoleSmoother,
    pan: OnePoleSmoother,
    zeros: Vec<f32>,
}

impl SamplerInstance {
    pub fn new(
        sample_rate: f64,
        max_block: usize,
        values: Vec<f64>,
        sample: Option<Arc<PreparedSample>>,
    ) -> Self {
        let mut instance = Self {
            sample_rate,
            max_block,
            values,
            sample,
            pool: SampleVoicePool::new(MAX_VOICES, sample_rate, max_block),
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
            p::AMP_ATTACK | p::AMP_DECAY | p::AMP_SUSTAIN | p::AMP_RELEASE => {
                let adsr = self.adsr_params();
                for i in 0..self.pool.len() {
                    if self.pool.voice(i).active {
                        let (a, d, s, r) = adsr;
                        self.pool.voice_mut(i).amp.set_params(a, d, s, r);
                    }
                }
            }
            _ => {}
        }
    }

    fn adsr_params(&self) -> (f64, f64, f32, f64) {
        (
            self.values[p::AMP_ATTACK],
            self.values[p::AMP_DECAY],
            self.values[p::AMP_SUSTAIN] as f32,
            self.values[p::AMP_RELEASE],
        )
    }

    fn start_frame(&self, sample: &PreparedSample) -> u64 {
        let frame = (self.values[p::START_SECONDS] * self.sample_rate).round() as u64;
        frame.min(sample.frames().saturating_sub(1))
    }

    fn note_on(&mut self, pitch: u8, velocity: f32) {
        let Some(sample) = self.sample.as_deref() else {
            return;
        };
        let semitones = f64::from(pitch) - self.values[p::ROOT_KEY];
        let rate = 2f64.powf(semitones / 12.0).clamp(MIN_RATE, MAX_RATE);
        let sensitivity = self.values[p::VELOCITY_SENSITIVITY];
        let gain = ((1.0 - sensitivity) + sensitivity * f64::from(velocity)) as f32;
        let base = self.start_frame(sample);
        let stream_len = sample.frames() - base;
        let loop_region = if self.values[p::LOOP_MODE] as usize == p::LOOP_FORWARD {
            sample.loop_points.and_then(|lp| {
                let (s, e) = (lp.start_frame, lp.end_frame.min(sample.frames()));
                (s >= base && s < e).then_some((s - base, e - base))
            })
        } else {
            debug_assert_eq!(self.values[p::LOOP_MODE] as usize, p::LOOP_OFF);
            None
        };
        let slot = self.pool.allocate(gain);
        let adsr = self.adsr_params();
        self.pool.voice_mut(slot).start(VoiceStart {
            note: pitch,
            rate,
            base,
            stream_len,
            loop_region,
            reverse: false,
            gain,
            pan: 0.0,
            adsr,
        });
    }

    fn note_off(&mut self, pitch: u8) {
        for i in 0..self.pool.len() {
            if self.pool.voice(i).active && self.pool.voice(i).note == pitch {
                self.pool.voice_mut(i).amp.note_off();
            }
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
            }
        }
        // Global level/pan after the voice sum; `pan` is a stereo balance
        // (unity at center), the voice already panned mono→stereo.
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

impl PluginInstance for SamplerInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        let max_block = max_block_size as usize;
        if sample_rate != self.sample_rate || max_block != self.max_block {
            self.sample_rate = sample_rate;
            self.max_block = max_block;
            self.pool = SampleVoicePool::new(MAX_VOICES, sample_rate, max_block);
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
                            NoteEventKind::NoteOn => self.note_on(note.pitch, note.velocity),
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
        self.snap_smoothers();
    }

    fn tail_frames(&self) -> u64 {
        if self.pool.active_count() == 0 {
            return 0;
        }
        let release = (self.values[p::AMP_RELEASE] * self.sample_rate) as u64;
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
