//! Band-limited tape coloration: driven 2x saturation and deterministic speed modulation.
use super::{
    controls::{Control as C, Controls},
    fractional::FractionalDelay,
    oversample::Oversampler2x,
};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 5] = [
    C::db("driveDb", "Drive", 0., 24., 6.),
    C::linear("wow", "Wow", 0., 1., 0.15),
    C::linear("flutter", "Flutter", 0., 1., 0.1),
    C::hz("toneHz", "Bandwidth", 1000., 20000., 12000.),
    C::db("outputDb", "Output", -24., 12., 0.),
];
pub struct TapePlugin;
impl Plugin for TapePlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.tape",
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
    controls: [Controls<5>; 2],
    os: [Oversampler2x; 2],
    lines: [FractionalDelay; 2],
    phase: [[f64; 2]; 2],
    low: [f64; 2],
    latency: usize,
}
impl Instance {
    fn new(rate: f64, block: u32) -> Self {
        Self {
            rate,
            controls: std::array::from_fn(|_| Controls::new(&TABLE, rate * 2.)),
            os: std::array::from_fn(|_| Oversampler2x::new(block as usize)),
            lines: std::array::from_fn(|_| FractionalDelay::new((rate * 0.02) as usize + 8)),
            phase: [[0.; 2]; 2],
            low: [0.; 2],
            latency: (rate * 0.005).round() as usize,
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, block: u32) {
        *self = Self::new(rate, block);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for ch in 0..2 {
            let c = &mut self.controls[ch];
            c.events(ctx.parameter_events);
            let rate = self.rate * 2.;
            let phase = &mut self.phase[ch];
            let line = &mut self.lines[ch];
            let low = &mut self.low[ch];
            let latency = self.latency as f64 * 2.;
            let output =
                self.os[ch].process_roundtrip(&ctx.inputs[ch][..ctx.frames], &mut |band| {
                    for x in band {
                        let v = c.next();
                        let wow = (std::f64::consts::TAU * phase[0]).sin()
                            + 0.2 * (std::f64::consts::TAU * phase[0] * 3.).sin();
                        let flutter = (std::f64::consts::TAU * phase[1]).sin();
                        let delayed = line
                            .read(latency + rate * (0.001 * v[1] * wow + 0.00012 * v[2] * flutter));
                        let driven = (*x as f64 * 10f64.powf(v[0] / 20.)).tanh();
                        let coefficient =
                            1. - (-std::f64::consts::TAU * v[3].min(rate * 0.225) / rate).exp();
                        *low += coefficient * (driven - *low);
                        if low.abs() < 1e-20 {
                            *low = 0.;
                        }
                        line.push(*low as f32);
                        *x = delayed * super::db_to_linear(v[4]);
                        phase[0] = (phase[0] + 0.37 / rate).fract();
                        phase[1] = (phase[1] + 6.71 / rate).fract();
                    }
                });
            ctx.outputs[ch][..ctx.frames].copy_from_slice(output);
        }
    }
    fn reset(&mut self) {
        for c in &mut self.controls {
            c.reset();
        }
        for o in &mut self.os {
            o.reset();
        }
        for l in &mut self.lines {
            l.reset();
        }
        self.phase = [[0.; 2]; 2];
        self.low = [0.; 2];
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        self.latency as u64 + 24
    }
}
