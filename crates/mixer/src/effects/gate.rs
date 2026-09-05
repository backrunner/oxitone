//! `oxitone.gate` — threshold gate with attack / hold / release. The
//! envelope follower is stereo-linked; gain ramps are linear in amplitude
//! over the attack/release times. Zero latency.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{db_to_linear, descriptor, param};

pub struct GatePlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for GatePlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.gate",
                vec![
                    param(
                        "thresholdDb",
                        "Threshold",
                        ParameterUnit::Db,
                        -80.0,
                        0.0,
                        -40.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "attackMs",
                        "Attack",
                        ParameterUnit::Normalized,
                        0.01,
                        100.0,
                        1.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "holdMs",
                        "Hold",
                        ParameterUnit::Normalized,
                        0.0,
                        500.0,
                        50.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "releaseMs",
                        "Release",
                        ParameterUnit::Normalized,
                        1.0,
                        2000.0,
                        100.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(GateInstance::new(host.sample_rate))
    }
}

struct GateInstance {
    sample_rate: f64,
    threshold_db: f64,
    attack_ms: f64,
    hold_ms: f64,
    release_ms: f64,
    envelope: f32,
    gain: f32,
    hold_remaining: u32,
}

impl GateInstance {
    fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            threshold_db: -40.0,
            attack_ms: 1.0,
            hold_ms: 50.0,
            release_ms: 100.0,
            envelope: 0.0,
            gain: 0.0,
            hold_remaining: 0,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "thresholdDb" => self.threshold_db = value.clamp(-80.0, 0.0),
            "attackMs" => self.attack_ms = value.clamp(0.01, 100.0),
            "holdMs" => self.hold_ms = value.clamp(0.0, 500.0),
            "releaseMs" => self.release_ms = value.clamp(1.0, 2000.0),
            _ => {}
        }
    }
}

impl PluginInstance for GateInstance {
    fn prepare(&mut self, sample_rate: f64, _max_block_size: u32) {
        *self = GateInstance::new(sample_rate);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let threshold = db_to_linear(self.threshold_db);
        let attack_step = 1.0 / (self.attack_ms / 1000.0 * self.sample_rate).max(1.0) as f32;
        let release_step = 1.0 / (self.release_ms / 1000.0 * self.sample_rate).max(1.0) as f32;
        let hold_frames = (self.hold_ms / 1000.0 * self.sample_rate) as u32;
        let env_coeff = (-1.0 / (0.005 * self.sample_rate)).exp() as f32;
        let (left, right) = ctx.outputs.split_at_mut(1);
        let left = &mut left[0][..frames];
        let right = &mut right[0][..frames];
        for n in 0..frames {
            let peak = left[n].abs().max(right[n].abs());
            self.envelope = peak.max(self.envelope * env_coeff);
            if self.envelope >= threshold {
                self.hold_remaining = hold_frames;
                self.gain = (self.gain + attack_step).min(1.0);
            } else if self.hold_remaining > 0 {
                self.hold_remaining -= 1;
            } else {
                self.gain = (self.gain - release_step).max(0.0);
            }
            left[n] *= self.gain;
            right[n] *= self.gain;
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.gain = 0.0;
        self.hold_remaining = 0;
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
