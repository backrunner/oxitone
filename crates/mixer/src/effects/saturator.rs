//! `oxitone.saturator` — tanh/cubic waveshaper with selectable 2x/4x
//! oversampling. The 4x cascade always runs (linear second stage in 2x
//! mode), so the reported latency (36 frames) never changes outside
//! `prepare`.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::oversample::Oversampler4x;
use super::{db_to_linear, descriptor, enum_param, param};

pub struct SaturatorPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for SaturatorPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.saturator",
                vec![
                    enum_param("curve", "Curve (0=tanh 1=cubic)", 0.0, 1.0, 0.0),
                    enum_param("oversample", "Oversample (0=2x 1=4x)", 0.0, 1.0, 1.0),
                    param(
                        "driveDb",
                        "Drive",
                        ParameterUnit::Db,
                        0.0,
                        36.0,
                        12.0,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "outputDb",
                        "Output",
                        ParameterUnit::Db,
                        -24.0,
                        12.0,
                        0.0,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Linear,
                    ),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(SaturatorInstance::new(
            host.sample_rate,
            host.max_block_size,
        ))
    }
}

fn shape(x: f32, curve: u8) -> f32 {
    if curve == 0 {
        x.tanh()
    } else if x.abs() <= 1.5 {
        x - x * x * x * (4.0 / 27.0)
    } else {
        x.signum()
    }
}

struct SaturatorInstance {
    curve: u8,
    at_4x: bool,
    drive_db: f64,
    output_db: f64,
    gain_smooth: OnePoleSmoother,
    out_smooth: OnePoleSmoother,
    os_left: Oversampler4x,
    os_right: Oversampler4x,
}

impl SaturatorInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        let mut gain_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        gain_smooth.snap(db_to_linear(12.0));
        let mut out_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        out_smooth.snap(1.0);
        Self {
            curve: 0,
            at_4x: true,
            drive_db: 12.0,
            output_db: 0.0,
            gain_smooth,
            out_smooth,
            os_left: Oversampler4x::new(max_block_size as usize),
            os_right: Oversampler4x::new(max_block_size as usize),
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "curve" => self.curve = value.round().clamp(0.0, 1.0) as u8,
            "oversample" => self.at_4x = value.round() >= 1.0,
            "driveDb" => {
                self.drive_db = value.clamp(0.0, 36.0);
                self.gain_smooth.set_target(db_to_linear(self.drive_db));
            }
            "outputDb" => {
                self.output_db = value.clamp(-24.0, 12.0);
                self.out_smooth.set_target(db_to_linear(self.output_db));
            }
            _ => {}
        }
    }
}

impl PluginInstance for SaturatorInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = SaturatorInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let mut drive = 0.0f32;
        let mut out_gain = 0.0f32;
        for _ in 0..frames {
            drive = self.gain_smooth.next_sample();
            out_gain = self.out_smooth.next_sample();
        }
        let (curve, at_4x) = (self.curve, self.at_4x);
        let (left, right) = ctx.outputs.split_at_mut(1);
        let mut shaper = |band: &mut [f32]| {
            for x in band.iter_mut() {
                *x = shape(*x * drive, curve) * out_gain;
            }
        };
        let left = &mut left[0][..frames];
        let down = self.os_left.process(left, at_4x, &mut shaper);
        left.copy_from_slice(down);
        let right = &mut right[0][..frames];
        let down = self.os_right.process(right, at_4x, &mut shaper);
        right.copy_from_slice(down);
    }

    fn reset(&mut self) {
        self.os_left.reset();
        self.os_right.reset();
        self.gain_smooth.snap(db_to_linear(self.drive_db));
        self.out_smooth.snap(db_to_linear(self.output_db));
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        self.os_left.latency_frames()
    }
}
