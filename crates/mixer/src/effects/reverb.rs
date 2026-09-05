//! `oxitone.reverb` — Schroeder reverb: predelay, four damped parallel
//! combs and two series allpasses per channel (algorithm choice: Schroeder;
//! the interface does not expose it). All delay lines are preallocated in
//! `prepare`, feedback state is denormal-flushed, and the plugin reports a
//! tail. Output is 100% wet; the host `mix` parameter blends dry.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::ftz::flush_denormal;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{descriptor, param};

/// Comb lengths in samples at 44.1 kHz (right channel gets +23 spread);
/// scaled by the prepare sample rate.
const COMBS: [usize; 4] = [1557, 1617, 1491, 1422];
const ALLPASSES: [usize; 2] = [225, 556];
const STEREO_SPREAD: usize = 23;
const MAX_PREDELAY_SECONDS: f64 = 0.1;

pub struct ReverbPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for ReverbPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.reverb",
                vec![
                    param(
                        "decaySeconds",
                        "Decay Time",
                        ParameterUnit::Seconds,
                        0.1,
                        12.0,
                        1.8,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "damping",
                        "Damping",
                        ParameterUnit::Normalized,
                        0.0,
                        1.0,
                        0.4,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "predelayMs",
                        "Predelay",
                        ParameterUnit::Normalized,
                        0.0,
                        100.0,
                        20.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                ],
                PluginCapabilities {
                    sidechain_input: false,
                    reports_tail: true,
                },
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(ReverbInstance::new(host.sample_rate, host.max_block_size))
    }
}

/// Preallocated integer delay line with a read/write cursor.
struct Line {
    buf: Vec<f32>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            pos: 0,
        }
    }

    #[inline]
    fn read(&self) -> f32 {
        self.buf[self.pos]
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.pos] = flush_denormal(x);
        self.pos = (self.pos + 1) % self.buf.len();
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

struct Comb {
    line: Line,
    feedback: f32,
    damp_state: f32,
}

impl Comb {
    #[inline]
    fn next(&mut self, input: f32, damping: f32) -> f32 {
        let out = self.line.read();
        self.damp_state = flush_denormal(out * (1.0 - damping) + self.damp_state * damping);
        self.line.write(input + self.damp_state * self.feedback);
        out
    }
}

struct Allpass {
    line: Line,
}

impl Allpass {
    #[inline]
    fn next(&mut self, input: f32) -> f32 {
        let delayed = self.line.read();
        let out = delayed - input;
        self.line.write(input + delayed * 0.5);
        out
    }
}

struct ReverbChannel {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

struct ReverbInstance {
    sample_rate: f64,
    decay_seconds: f64,
    damping: f64,
    predelay_ms: f64,
    predelay: [Line; 2],
    channels: [ReverbChannel; 2],
    tail_remaining: u64,
}

impl ReverbInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        let scale = sample_rate / 44_100.0;
        let predelay_len =
            (MAX_PREDELAY_SECONDS * sample_rate) as usize + max_block_size as usize + 1;
        let make_channel = |spread: usize| ReverbChannel {
            combs: COMBS
                .iter()
                .map(|&len| Comb {
                    line: Line::new(((len + spread) as f64 * scale).round() as usize),
                    feedback: 0.8,
                    damp_state: 0.0,
                })
                .collect(),
            allpasses: ALLPASSES
                .iter()
                .map(|&len| Allpass {
                    line: Line::new(((len + spread) as f64 * scale).round() as usize),
                })
                .collect(),
        };
        Self {
            sample_rate,
            decay_seconds: 1.8,
            damping: 0.4,
            predelay_ms: 20.0,
            predelay: [Line::new(predelay_len), Line::new(predelay_len)],
            channels: [make_channel(0), make_channel(STEREO_SPREAD)],
            tail_remaining: 0,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "decaySeconds" => self.decay_seconds = value.clamp(0.1, 12.0),
            "damping" => self.damping = value.clamp(0.0, 1.0),
            "predelayMs" => self.predelay_ms = value.clamp(0.0, 100.0),
            _ => {}
        }
    }

    /// RT60 feedback per comb: g = 10^(-3·delay/decay). Control rate.
    fn update_feedback(&mut self) {
        for channel in &mut self.channels {
            for comb in &mut channel.combs {
                let delay_seconds = comb.line.buf.len() as f64 / self.sample_rate;
                comb.feedback = 10f64.powf(-3.0 * delay_seconds / self.decay_seconds) as f32;
            }
        }
    }

    fn max_tail_frames(&self) -> u64 {
        // -100 dB ≈ 5/3 × RT60, plus predelay.
        (self.decay_seconds * self.sample_rate * (100.0 / 60.0)
            + self.predelay_ms / 1000.0 * self.sample_rate) as u64
    }
}

impl PluginInstance for ReverbInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = ReverbInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        self.update_feedback();
        let frames = ctx.frames;
        let damping = self.damping as f32;
        let predelay_frames = ((self.predelay_ms / 1000.0 * self.sample_rate) as usize)
            .min(self.predelay[0].buf.len() - 1);
        let mut block_peak = 0.0f32;
        for (ch, channel) in self.channels.iter_mut().enumerate() {
            let input = &ctx.inputs[ch][..frames];
            let output = &mut ctx.outputs[ch][..frames];
            let predelay = &mut self.predelay[ch];
            let plen = predelay.buf.len();
            for n in 0..frames {
                block_peak = block_peak.max(input[n].abs());
                // Predelay: read `predelay_frames` behind the write cursor.
                let delayed = if predelay_frames == 0 {
                    input[n]
                } else {
                    let read_pos = (predelay.pos + plen - predelay_frames) % plen;
                    predelay.buf[read_pos]
                };
                predelay.write(input[n]);
                let mut wet = 0.0f32;
                for comb in channel.combs.iter_mut() {
                    wet += comb.next(delayed, damping);
                }
                wet *= 0.25;
                for allpass in channel.allpasses.iter_mut() {
                    wet = allpass.next(wet);
                }
                output[n] = wet;
            }
        }
        if block_peak > 1e-6 {
            self.tail_remaining = self.max_tail_frames();
        } else {
            self.tail_remaining = self.tail_remaining.saturating_sub(frames as u64);
        }
    }

    fn reset(&mut self) {
        for line in &mut self.predelay {
            line.clear();
        }
        for channel in &mut self.channels {
            for comb in &mut channel.combs {
                comb.line.clear();
                comb.damp_state = 0.0;
            }
            for allpass in &mut channel.allpasses {
                allpass.line.clear();
            }
        }
        self.tail_remaining = 0;
    }

    fn tail_frames(&self) -> u64 {
        self.tail_remaining
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
