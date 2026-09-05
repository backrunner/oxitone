//! `oxitone.limit` — lookahead limiter. A fixed 5 ms lookahead delays the
//! audio while the gain computer tracks the peak envelope; the optional
//! saturation stage runs at 2x oversampling. The oversampler always runs, so
//! latency (lookahead + 24 frames) is constant and reported via
//! `latency_frames`.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::oversample::Oversampler2x;
use super::{db_to_linear, descriptor, enum_param, param};

const LOOKAHEAD_SECONDS: f64 = 0.005;

pub struct LimitPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for LimitPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.limit",
                vec![
                    param(
                        "ceilingDb",
                        "Ceiling",
                        ParameterUnit::Db,
                        -12.0,
                        0.0,
                        -0.3,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "releaseMs",
                        "Release",
                        ParameterUnit::Normalized,
                        5.0,
                        500.0,
                        100.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    enum_param("saturate", "Saturate (0=off 1=on)", 0.0, 1.0, 0.0),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(LimitInstance::new(host.sample_rate, host.max_block_size))
    }
}

/// Stereo lookahead delay line.
struct Lookahead {
    left: Vec<f32>,
    right: Vec<f32>,
    pos: usize,
}

impl Lookahead {
    fn new(frames: usize) -> Self {
        Self {
            left: vec![0.0; frames.max(1)],
            right: vec![0.0; frames.max(1)],
            pos: 0,
        }
    }

    /// Push `x`, return the delayed sample.
    #[inline]
    fn next(buf: &mut [f32], pos: usize, x: f32) -> f32 {
        let y = buf[pos];
        buf[pos] = x;
        y
    }
}

struct LimitInstance {
    sample_rate: f64,
    ceiling_db: f64,
    release_ms: f64,
    saturate: bool,
    ceiling_smooth: OnePoleSmoother,
    lookahead: Lookahead,
    envelope: f32,
    gain: f32,
    os_left: Oversampler2x,
    os_right: Oversampler2x,
}

impl LimitInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        let lookahead_frames = (LOOKAHEAD_SECONDS * sample_rate).round() as usize;
        let mut ceiling_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        ceiling_smooth.snap(db_to_linear(-0.3));
        Self {
            sample_rate,
            ceiling_db: -0.3,
            release_ms: 100.0,
            saturate: false,
            ceiling_smooth,
            lookahead: Lookahead::new(lookahead_frames),
            envelope: 0.0,
            gain: 1.0,
            os_left: Oversampler2x::new(max_block_size as usize),
            os_right: Oversampler2x::new(max_block_size as usize),
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "ceilingDb" => {
                self.ceiling_db = value.clamp(-12.0, 0.0);
                self.ceiling_smooth
                    .set_target(db_to_linear(self.ceiling_db));
            }
            "releaseMs" => self.release_ms = value.clamp(5.0, 500.0),
            "saturate" => self.saturate = value.round() >= 1.0,
            _ => {}
        }
    }

    /// Gain computer over one block of detector input; applies the computed
    /// gain to the delayed audio. RT-safe.
    fn limit_block(&mut self, detector_l: &[f32], detector_r: &[f32], out: &mut [&mut [f32]]) {
        let release_coeff = (-1.0 / (self.release_ms / 1000.0 * self.sample_rate)).exp() as f32;
        let attack_coeff = (-1.0 / (LOOKAHEAD_SECONDS * self.sample_rate)).exp() as f32;
        let frames = detector_l.len();
        let len = self.lookahead.left.len();
        for n in 0..frames {
            let pos = self.lookahead.pos;
            let xl = Lookahead::next(&mut self.lookahead.left, pos, detector_l[n]);
            let xr = Lookahead::next(&mut self.lookahead.right, pos, detector_r[n]);
            self.lookahead.pos = (pos + 1) % len;
            let peak = detector_l[n].abs().max(detector_r[n].abs());
            self.envelope = peak.max(self.envelope * release_coeff);
            let ceiling = self.ceiling_smooth.next_sample();
            let target = if self.envelope > ceiling && self.envelope > 0.0 {
                ceiling / self.envelope
            } else {
                1.0
            };
            let coeff = if target < self.gain {
                attack_coeff
            } else {
                release_coeff
            };
            self.gain += (target - self.gain) * coeff;
            out[0][n] = xl * self.gain;
            out[1][n] = xr * self.gain;
        }
    }
}

impl PluginInstance for LimitInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = LimitInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        let frames = ctx.frames;
        let saturate = self.saturate;
        // Limit into the output buffers, then run the (always active) 2x
        // stage; saturation applies tanh, otherwise the stage is linear.
        let mut shaper = |band: &mut [f32]| {
            if saturate {
                for x in band.iter_mut() {
                    *x = x.tanh();
                }
            }
        };
        {
            let (left, right) = ctx.outputs.split_at_mut(1);
            let mut outs = [&mut left[0][..frames], &mut right[0][..frames]];
            self.limit_block(
                &ctx.inputs[0][..frames],
                &ctx.inputs[1][..frames],
                &mut outs,
            );
        }
        let (left, right) = ctx.outputs.split_at_mut(1);
        let left = &mut left[0][..frames];
        let down = self.os_left.process_roundtrip(left, &mut shaper);
        left.copy_from_slice(down);
        let right = &mut right[0][..frames];
        let down = self.os_right.process_roundtrip(right, &mut shaper);
        right.copy_from_slice(down);
    }

    fn reset(&mut self) {
        for x in self
            .lookahead
            .left
            .iter_mut()
            .chain(self.lookahead.right.iter_mut())
        {
            *x = 0.0;
        }
        self.lookahead.pos = 0;
        self.envelope = 0.0;
        self.gain = 1.0;
        self.os_left.reset();
        self.os_right.reset();
        self.ceiling_smooth.snap(db_to_linear(self.ceiling_db));
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        (LOOKAHEAD_SECONDS * self.sample_rate).round() as u64 + self.os_left.latency_frames()
    }
}
