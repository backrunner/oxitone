//! `oxitone.utility` — gain / width (M-S) / polarity / mono channel
//! utility. Gain and width are one-pole smoothed; `mono` collapses to the
//! mid signal; `polarity` inverts both channels. Zero latency.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{db_to_linear, descriptor, enum_param, param};

pub struct UtilityPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for UtilityPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.utility",
                vec![
                    param(
                        "gainDb",
                        "Gain",
                        ParameterUnit::Db,
                        -24.0,
                        24.0,
                        0.0,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "width",
                        "Width",
                        ParameterUnit::Normalized,
                        0.0,
                        2.0,
                        1.0,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Linear,
                    ),
                    enum_param("mono", "Mono (0=off 1=on)", 0.0, 1.0, 0.0),
                    enum_param("polarity", "Polarity (0=normal 1=inverted)", 0.0, 1.0, 0.0),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(UtilityInstance::new(host.sample_rate))
    }
}

struct UtilityInstance {
    gain_db: f64,
    width: f64,
    mono: bool,
    inverted: bool,
    gain_smooth: OnePoleSmoother,
    width_smooth: OnePoleSmoother,
}

impl UtilityInstance {
    fn new(sample_rate: f64) -> Self {
        let mut gain_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        gain_smooth.snap(1.0);
        let mut width_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        width_smooth.snap(1.0);
        Self {
            gain_db: 0.0,
            width: 1.0,
            mono: false,
            inverted: false,
            gain_smooth,
            width_smooth,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "gainDb" => {
                self.gain_db = value.clamp(-24.0, 24.0);
                self.gain_smooth.set_target(db_to_linear(self.gain_db));
            }
            "width" => {
                self.width = value.clamp(0.0, 2.0);
                self.width_smooth.set_target(self.width as f32);
            }
            "mono" => self.mono = value.round() >= 1.0,
            "polarity" => self.inverted = value.round() >= 1.0,
            _ => {}
        }
    }
}

impl PluginInstance for UtilityInstance {
    fn prepare(&mut self, sample_rate: f64, _max_block_size: u32) {
        *self = UtilityInstance::new(sample_rate);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let sign = if self.inverted { -1.0f32 } else { 1.0 };
        let (left, right) = ctx.outputs.split_at_mut(1);
        let left = &mut left[0][..frames];
        let right = &mut right[0][..frames];
        for n in 0..frames {
            let gain = self.gain_smooth.next_sample() * sign;
            let width = self.width_smooth.next_sample();
            let mid = 0.5 * (left[n] + right[n]);
            let side = 0.5 * (left[n] - right[n]) * width;
            if self.mono {
                left[n] = mid * gain;
                right[n] = mid * gain;
            } else {
                left[n] = (mid + side) * gain;
                right[n] = (mid - side) * gain;
            }
        }
    }

    fn reset(&mut self) {
        self.gain_smooth.snap(db_to_linear(self.gain_db));
        self.width_smooth.snap(self.width as f32);
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
