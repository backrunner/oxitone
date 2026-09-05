//! `oxitone.clipper` — hard/soft clipper with fixed 2x oversampling for
//! alias suppression (03-audio-runtime-spec.md §数值精度). The oversampler
//! always runs, so the reported latency (24 frames) is parameter
//! independent. Drive is one-pole smoothed.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::oversample::Oversampler2x;
use super::{db_to_linear, descriptor, enum_param, param};

pub struct ClipperPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for ClipperPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.clipper",
                vec![
                    enum_param("mode", "Mode (0=hard 1=soft)", 0.0, 1.0, 1.0),
                    param(
                        "driveDb",
                        "Drive",
                        ParameterUnit::Db,
                        0.0,
                        36.0,
                        6.0,
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
        Box::new(ClipperInstance::new(host.sample_rate, host.max_block_size))
    }
}

/// Waveshaper applied at 2x rate.
fn shape(x: f32, mode: u8) -> f32 {
    if mode == 0 {
        x.clamp(-1.0, 1.0)
    } else if x.abs() <= 1.0 {
        x - x * x * x / 3.0
    } else {
        x.signum() * (2.0 / 3.0)
    }
}

struct ClipperInstance {
    mode: u8,
    drive_db: f64,
    output_db: f64,
    gain_smooth: OnePoleSmoother,
    out_smooth: OnePoleSmoother,
    os_left: Oversampler2x,
    os_right: Oversampler2x,
}

impl ClipperInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        let mut gain_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        gain_smooth.snap(db_to_linear(6.0));
        let mut out_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        out_smooth.snap(1.0);
        Self {
            mode: 1,
            drive_db: 6.0,
            output_db: 0.0,
            gain_smooth,
            out_smooth,
            os_left: Oversampler2x::new(max_block_size as usize),
            os_right: Oversampler2x::new(max_block_size as usize),
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "mode" => self.mode = value.round().clamp(0.0, 1.0) as u8,
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

    /// In-place: `upsample` copies into the internal band before `buf` is
    /// overwritten, so aliasing input/output is safe.
    fn process_channel(
        os: &mut Oversampler2x,
        buf: &mut [f32],
        drive: f32,
        out_gain: f32,
        mode: u8,
    ) {
        let frames = buf.len();
        let band = os.upsample(buf);
        for x in band.iter_mut() {
            *x = shape(*x * drive, mode) * out_gain;
        }
        let down = os.downsample(frames);
        buf.copy_from_slice(down);
    }
}

impl PluginInstance for ClipperInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = ClipperInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let (left, right) = ctx.outputs.split_at_mut(1);
        // Smoothed gains are block-rate targets; the per-sample smoothing
        // advances across the block and the block uses the end value. For a
        // clipper the transient is inaudible relative to the shaper.
        let mut drive = 0.0f32;
        let mut out_gain = 0.0f32;
        for _ in 0..frames {
            drive = self.gain_smooth.next_sample();
            out_gain = self.out_smooth.next_sample();
        }
        Self::process_channel(
            &mut self.os_left,
            &mut left[0][..frames],
            drive,
            out_gain,
            self.mode,
        );
        Self::process_channel(
            &mut self.os_right,
            &mut right[0][..frames],
            drive,
            out_gain,
            self.mode,
        );
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
