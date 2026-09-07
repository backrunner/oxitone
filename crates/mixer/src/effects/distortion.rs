//! Four-times-oversampled distortion, independent input/output smoothing and DC rejection.
use super::{db_to_linear, descriptor, enum_param, oversample::Oversampler4x, param};
use oxitone_core::wire::{
    ParameterMapping as Map, ParameterSmoothing as Smooth, ParameterUnit as Unit,
};
use oxitone_dsp::{ftz::flush_denormal, gain_pan::OnePoleSmoother};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;

pub struct DistortionPlugin;
impl Plugin for DistortionPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        static DESC: OnceLock<PluginDescriptor> = OnceLock::new();
        DESC.get_or_init(|| {
            descriptor(
                "oxitone.distortion",
                vec![
                    enum_param("mode", "Mode (soft/hard/fold/asymmetric)", 0., 3., 0.),
                    param(
                        "driveDb",
                        "Drive",
                        Unit::Db,
                        0.,
                        36.,
                        6.,
                        Smooth::OnePole,
                        Map::Linear,
                    ),
                    param(
                        "outputDb",
                        "Output",
                        Unit::Db,
                        -36.,
                        12.,
                        -6.,
                        Smooth::OnePole,
                        Map::Linear,
                    ),
                    param(
                        "toneHz",
                        "Tone",
                        Unit::Hz,
                        200.,
                        20000.,
                        16000.,
                        Smooth::None,
                        Map::Log,
                    ),
                    param(
                        "bias",
                        "Asymmetry",
                        Unit::Normalized,
                        -1.,
                        1.,
                        0.2,
                        Smooth::None,
                        Map::Bipolar,
                    ),
                ],
                PluginCapabilities::default(),
            )
        })
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(host.sample_rate, host.max_block_size))
    }
}
struct Instance {
    rate: f64,
    mode: usize,
    bias: f32,
    tone: f64,
    drive_db: f64,
    out_db: f64,
    drive: OnePoleSmoother,
    output: OnePoleSmoother,
    os: [Oversampler4x; 2],
    gains: Vec<f32>,
    out_gains: Vec<f32>,
    dc: [f32; 2],
    low: [f32; 2],
}
impl Instance {
    fn new(rate: f64, block: u32) -> Self {
        let mut drive = OnePoleSmoother::new(rate, 5.);
        drive.snap(db_to_linear(6.));
        let mut output = OnePoleSmoother::new(rate, 5.);
        output.snap(db_to_linear(-6.));
        Self {
            rate,
            mode: 0,
            bias: 0.2,
            tone: 16000.,
            drive_db: 6.,
            out_db: -6.,
            drive,
            output,
            os: std::array::from_fn(|_| Oversampler4x::new(block as usize)),
            gains: vec![0.; block as usize],
            out_gains: vec![0.; block as usize],
            dc: [0.; 2],
            low: [0.; 2],
        }
    }
    fn parameter(&mut self, id: &str, value: f64) {
        match id {
            "mode" => self.mode = value.round().clamp(0., 3.) as usize,
            "bias" => self.bias = value.clamp(-1., 1.) as f32,
            "toneHz" => self.tone = value.clamp(200., 20000.),
            "driveDb" => {
                self.drive_db = value.clamp(0., 36.);
                self.drive.set_target(db_to_linear(self.drive_db));
            }
            "outputDb" => {
                self.out_db = value.clamp(-36., 12.);
                self.output.set_target(db_to_linear(self.out_db));
            }
            _ => {}
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, block: u32) {
        *self = Self::new(rate, block);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.parameter(event.parameter_id, event.value);
        }
        for n in 0..ctx.frames {
            self.gains[n] = self.drive.next_sample();
            self.out_gains[n] = self.output.next_sample();
        }
        let mode = self.mode;
        let bias = self.bias;
        let gains = &self.gains;
        let tone = (1. - (-std::f64::consts::TAU * self.tone / self.rate).exp()) as f32;
        let dc = (1. - (-std::f64::consts::TAU * 8. / self.rate).exp()) as f32;
        for ch in 0..2 {
            let wet = self.os[ch].process(&ctx.inputs[ch][..ctx.frames], true, &mut |band| {
                for (i, value) in band.iter_mut().enumerate() {
                    let x = *value * gains[i / 4];
                    *value = match mode {
                        1 => x.clamp(-1., 1.),
                        2 => (x * std::f32::consts::FRAC_PI_2).sin(),
                        3 => (x + bias).tanh() - bias.tanh(),
                        _ => (x).tanh(),
                    };
                }
            });
            for (n, &value) in wet.iter().enumerate() {
                self.dc[ch] = flush_denormal(self.dc[ch] + (value - self.dc[ch]) * dc);
                self.low[ch] =
                    flush_denormal(self.low[ch] + (value - self.dc[ch] - self.low[ch]) * tone);
                ctx.outputs[ch][n] = self.low[ch] * self.out_gains[n];
            }
        }
    }
    fn reset(&mut self) {
        for os in &mut self.os {
            os.reset();
        }
        self.dc = [0.; 2];
        self.low = [0.; 2];
        self.drive.snap(db_to_linear(self.drive_db));
        self.output.snap(db_to_linear(self.out_db));
    }
    fn latency_frames(&self) -> u64 {
        36
    }
    fn tail_frames(&self) -> u64 {
        0
    }
}
