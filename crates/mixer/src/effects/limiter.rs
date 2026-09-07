//! Stereo-linked 4x lookahead limiter with peak-window hold and bounded recovery.
use super::{
    controls::{Control as C, Controls},
    oversample::Oversampler4x,
    peak_window::PeakWindow,
};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 3] = [
    C::db("ceilingDb", "Ceiling", -12., 0., -1.),
    C::log("releaseMs", "Release", 5., 1000., 120.),
    C::db("inputDb", "Input", -12., 24., 0.),
];
pub struct LimiterPlugin;
impl Plugin for LimiterPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.limiter",
                TABLE.map(C::spec).to_vec(),
                PluginCapabilities::default(),
            )
        })
    }
    fn create(&self, h: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(h.sample_rate, h.max_block_size))
    }
}
struct Instance {
    rate: f64,
    controls: Controls<3>,
    os: [Oversampler4x; 2],
    delay: [Vec<f32>; 2],
    position: usize,
    peaks: PeakWindow,
    gain: f32,
}
impl Instance {
    fn new(rate: f64, block: u32) -> Self {
        let delay = ((rate * 0.005).round() as usize).max(1) * 4;
        Self {
            rate,
            controls: Controls::new(&TABLE, rate * 4.),
            os: std::array::from_fn(|_| Oversampler4x::new(block as usize)),
            delay: std::array::from_fn(|_| vec![0.; delay]),
            position: 0,
            peaks: PeakWindow::new(delay),
            gain: 1.,
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, block: u32) {
        *self = Self::new(rate, block);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        self.controls.events(ctx.parameter_events);
        let (os_l, os_r) = self.os.split_at_mut(1);
        let left = os_l[0].upsample(&ctx.inputs[0][..ctx.frames]);
        let right = os_r[0].upsample(&ctx.inputs[1][..ctx.frames]);
        for n in 0..ctx.frames * 4 {
            let v = self.controls.next();
            let input_gain = super::db_to_linear(v[2]);
            let x = [left[n] * input_gain, right[n] * input_gain];
            let peak = self.peaks.next(x[0].abs().max(x[1].abs()));
            // 0.92 dB reconstruction reserve, measured again by the export true-peak meter.
            let ceiling = super::db_to_linear(v[0]) * 0.9;
            let target = (ceiling / peak.max(1e-12)).min(1.);
            let coefficient = (-1. / (v[1] * 0.001 * self.rate * 4.)).exp() as f32;
            self.gain = if target < self.gain {
                target
            } else {
                target + (self.gain - target) * coefficient
            };
            left[n] = self.delay[0][self.position] * self.gain;
            right[n] = self.delay[1][self.position] * self.gain;
            for (ch, value) in x.into_iter().enumerate() {
                self.delay[ch][self.position] = value;
            }
            self.position = (self.position + 1) % self.delay[0].len();
        }
        ctx.outputs[0][..ctx.frames].copy_from_slice(os_l[0].downsample(ctx.frames));
        ctx.outputs[1][..ctx.frames].copy_from_slice(os_r[0].downsample(ctx.frames));
    }
    fn reset(&mut self) {
        self.controls.reset();
        for os in &mut self.os {
            os.reset();
        }
        for d in &mut self.delay {
            d.fill(0.);
        }
        self.peaks.reset();
        self.position = 0;
        self.gain = 1.;
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        (self.delay[0].len() / 4 + 36) as u64
    }
}
