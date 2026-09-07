//! `oxitone.filter` — single multimode biquad (lp/hp/bp) with cutoff and
//! resonance. `f64` coefficients and state; cutoff is smoothed per sample and
//! coefficients redesigned on a persistent 32-frame control cycle.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::biquad::{design, BiquadF64, BiquadKind};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{descriptor, enum_param, param};

pub struct FilterPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for FilterPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.filter",
                vec![
                    enum_param("mode", "Mode (0=lp 1=hp 2=bp)", 0.0, 2.0, 0.0),
                    param(
                        "cutoffHz",
                        "Cutoff",
                        ParameterUnit::Hz,
                        20.0,
                        20_000.0,
                        1000.0,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Log,
                    ),
                    param(
                        "resonance",
                        "Resonance (Q)",
                        ParameterUnit::Normalized,
                        0.1,
                        24.0,
                        0.707,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(FilterInstance::new(host.sample_rate))
    }
}

struct FilterInstance {
    sample_rate: f64,
    mode: u8,
    cutoff_hz: f64,
    q: f64,
    cutoff_smooth: OnePoleSmoother,
    left: BiquadF64,
    right: BiquadF64,
    coefficient_tick: u8,
}

impl FilterInstance {
    fn new(sample_rate: f64) -> Self {
        let coeffs = design(BiquadKind::Lowpass, sample_rate, 1000.0, 0.707, 0.0);
        let mut cutoff_smooth = OnePoleSmoother::new(sample_rate, 15.0);
        cutoff_smooth.snap(1000.0);
        Self {
            sample_rate,
            mode: 0,
            cutoff_hz: 1000.0,
            q: 0.707,
            cutoff_smooth,
            left: BiquadF64::new(coeffs),
            right: BiquadF64::new(coeffs),
            coefficient_tick: 0,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "mode" => self.mode = value.round().clamp(0.0, 2.0) as u8,
            "cutoffHz" => {
                self.cutoff_hz = value.clamp(20.0, 20_000.0);
                self.cutoff_smooth.set_target(self.cutoff_hz as f32);
            }
            "resonance" => self.q = value.clamp(0.1, 24.0),
            _ => {}
        }
    }
}

impl PluginInstance for FilterInstance {
    fn prepare(&mut self, sample_rate: f64, _max_block_size: u32) {
        *self = FilterInstance::new(sample_rate);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let (left, right) = ctx.outputs.split_at_mut(1);
        let left = &mut left[0][..frames];
        let right = &mut right[0][..frames];
        let kind = match self.mode {
            1 => BiquadKind::Highpass,
            2 => BiquadKind::Bandpass,
            _ => BiquadKind::Lowpass,
        };
        for n in 0..frames {
            let cutoff = self.cutoff_smooth.next_sample() as f64;
            if self.coefficient_tick == 0 {
                let coeffs = design(kind, self.sample_rate, cutoff, self.q, 0.0);
                self.left.set_coeffs(coeffs);
                self.right.set_coeffs(coeffs);
            }
            self.coefficient_tick = (self.coefficient_tick + 1) % 32;
            left[n] = self.left.next(left[n]);
            right[n] = self.right.next(right[n]);
        }
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.cutoff_smooth.snap(self.cutoff_hz as f32);
        self.coefficient_tick = 0;
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
