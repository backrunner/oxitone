//! Three complementary bands with bounded stereo-linked upward/downward dynamics.
//! This is an original processor, not an emulation of Xfer OTT. Residual splits
//! reconstruct the dry input at unity gain without crossover phase cancellation.
use super::{db_to_linear, descriptor, param};
use oxitone_core::wire::{
    ParameterMapping as Map, ParameterSmoothing as Smooth, ParameterUnit as Unit,
};
use oxitone_dsp::{ftz::flush_denormal, gain_pan::OnePoleSmoother};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
pub struct MultibandPlugin;
impl Plugin for MultibandPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        static DESC: OnceLock<PluginDescriptor> = OnceLock::new();
        DESC.get_or_init(|| {
            descriptor(
                "oxitone.multiband",
                vec![
                    param(
                        "depth",
                        "Depth",
                        Unit::Normalized,
                        0.,
                        1.,
                        0.5,
                        Smooth::OnePole,
                        Map::Linear,
                    ),
                    param(
                        "upwardDb",
                        "Maximum Upward Gain",
                        Unit::Db,
                        0.,
                        24.,
                        12.,
                        Smooth::None,
                        Map::Linear,
                    ),
                    param(
                        "downwardRatio",
                        "Downward Ratio",
                        Unit::Normalized,
                        1.,
                        20.,
                        4.,
                        Smooth::None,
                        Map::Log,
                    ),
                    param(
                        "upperThresholdDb",
                        "Upper Threshold",
                        Unit::Db,
                        -36.,
                        0.,
                        -18.,
                        Smooth::None,
                        Map::Linear,
                    ),
                    param(
                        "lowerThresholdDb",
                        "Lower Threshold",
                        Unit::Db,
                        -60.,
                        -24.,
                        -36.,
                        Smooth::None,
                        Map::Linear,
                    ),
                    param(
                        "attackMs",
                        "Attack",
                        Unit::Normalized,
                        0.1,
                        100.,
                        3.,
                        Smooth::None,
                        Map::Log,
                    ),
                    param(
                        "releaseMs",
                        "Release",
                        Unit::Normalized,
                        10.,
                        1000.,
                        90.,
                        Smooth::None,
                        Map::Log,
                    ),
                    param(
                        "lowHz",
                        "Low Crossover",
                        Unit::Hz,
                        60.,
                        500.,
                        180.,
                        Smooth::None,
                        Map::Log,
                    ),
                    param(
                        "highHz",
                        "High Crossover",
                        Unit::Hz,
                        1000.,
                        8000.,
                        2500.,
                        Smooth::None,
                        Map::Log,
                    ),
                    param(
                        "lowGainDb",
                        "Low Band",
                        Unit::Db,
                        -18.,
                        18.,
                        0.,
                        Smooth::None,
                        Map::Linear,
                    ),
                    param(
                        "midGainDb",
                        "Mid Band",
                        Unit::Db,
                        -18.,
                        18.,
                        0.,
                        Smooth::None,
                        Map::Linear,
                    ),
                    param(
                        "highGainDb",
                        "High Band",
                        Unit::Db,
                        -18.,
                        18.,
                        0.,
                        Smooth::None,
                        Map::Linear,
                    ),
                    param(
                        "outputDb",
                        "Output",
                        Unit::Db,
                        -24.,
                        12.,
                        0.,
                        Smooth::OnePole,
                        Map::Linear,
                    ),
                ],
                PluginCapabilities::default(),
            )
        })
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(host.sample_rate))
    }
}
struct Instance {
    rate: f64,
    values: [f64; 13],
    split: [[f32; 2]; 2],
    envelope: [f32; 3],
    gain: [f32; 3],
    depth: OnePoleSmoother,
    output: OnePoleSmoother,
}
impl Instance {
    fn new(rate: f64) -> Self {
        let mut depth = OnePoleSmoother::new(rate, 5.);
        depth.snap(0.5);
        let mut output = OnePoleSmoother::new(rate, 5.);
        output.snap(1.);
        Self {
            rate,
            values: [
                0.5, 12., 4., -18., -36., 3., 90., 180., 2500., 0., 0., 0., 0.,
            ],
            split: [[0.; 2]; 2],
            envelope: [0.; 3],
            gain: [1.; 3],
            depth,
            output,
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, _: u32) {
        *self = Self::new(rate);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        const IDS: [&str; 13] = [
            "depth",
            "upwardDb",
            "downwardRatio",
            "upperThresholdDb",
            "lowerThresholdDb",
            "attackMs",
            "releaseMs",
            "lowHz",
            "highHz",
            "lowGainDb",
            "midGainDb",
            "highGainDb",
            "outputDb",
        ];
        const LIMITS: [(f64, f64); 13] = [
            (0., 1.),
            (0., 24.),
            (1., 20.),
            (-36., 0.),
            (-60., -24.),
            (0.1, 100.),
            (10., 1000.),
            (60., 500.),
            (1000., 8000.),
            (-18., 18.),
            (-18., 18.),
            (-18., 18.),
            (-24., 12.),
        ];
        for event in ctx.parameter_events {
            if let Some(i) = IDS.iter().position(|s| *s == event.parameter_id) {
                self.values[i] = event.value.clamp(LIMITS[i].0, LIMITS[i].1);
                if i == 0 {
                    self.depth.set_target(self.values[0] as f32);
                }
                if i == 12 {
                    self.output.set_target(db_to_linear(self.values[12]));
                }
            }
        }
        let v = self.values;
        let low = (1. - (-std::f64::consts::TAU * v[7] / self.rate).exp()) as f32;
        let high = (1. - (-std::f64::consts::TAU * v[8] / self.rate).exp()) as f32;
        let attack = (-1. / (v[5] * 0.001 * self.rate)).exp() as f32;
        let release = (-1. / (v[6] * 0.001 * self.rate)).exp() as f32;
        let trim = [db_to_linear(v[9]), db_to_linear(v[10]), db_to_linear(v[11])];
        for n in 0..ctx.frames {
            let mut bands = [[0.; 3]; 2];
            for (ch, b) in bands.iter_mut().enumerate() {
                let x = ctx.inputs[ch][n];
                self.split[ch][0] =
                    flush_denormal(self.split[ch][0] + low * (x - self.split[ch][0]));
                let upper = x - self.split[ch][0];
                self.split[ch][1] =
                    flush_denormal(self.split[ch][1] + high * (upper - self.split[ch][1]));
                *b = [
                    self.split[ch][0],
                    self.split[ch][1],
                    upper - self.split[ch][1],
                ];
            }
            let depth = self.depth.next_sample();
            let output = self.output.next_sample();
            for b in 0..3 {
                let peak = bands[0][b].abs().max(bands[1][b].abs());
                let c = if peak > self.envelope[b] {
                    attack
                } else {
                    release
                };
                self.envelope[b] = flush_denormal(peak + (self.envelope[b] - peak) * c);
                let level = 20. * (self.envelope[b].max(1e-12) as f64).log10();
                let down = -(level - v[3]).max(0.) * (1. - 1. / v[2]);
                let up = (v[4] - level).max(0.).min(v[1]) * ((level + 72.) / 12.).clamp(0., 1.);
                let target =
                    db_to_linear((down + up) * depth as f64) * (1. + (trim[b] - 1.) * depth);
                let c = if target < self.gain[b] {
                    attack
                } else {
                    release
                };
                self.gain[b] = target + (self.gain[b] - target) * c;
            }
            for ch in 0..2 {
                ctx.outputs[ch][n] =
                    (0..3).map(|b| bands[ch][b] * self.gain[b]).sum::<f32>() * output;
            }
        }
    }
    fn reset(&mut self) {
        self.split = [[0.; 2]; 2];
        self.envelope = [0.; 3];
        self.gain = [1.; 3];
        self.depth.snap(self.values[0] as f32);
        self.output.snap(db_to_linear(self.values[12]));
    }
    fn latency_frames(&self) -> u64 {
        0
    }
    fn tail_frames(&self) -> u64 {
        0
    }
}
