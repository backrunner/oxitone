//! Driven TPT state-variable filter with bounded nonlinear integrator memory, at 2x.
use super::{
    controls::{Control as C, Controls},
    oversample::Oversampler2x,
};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;

const TABLE: [C; 5] = [
    C::choice("mode", "Mode (LP/HP/BP/Notch)", 3., 0.),
    C::hz("cutoffHz", "Cutoff", 20., 20000., 1800.),
    C::linear("resonance", "Resonance", 0., 1., 0.25),
    C::db("driveDb", "Drive", 0., 30., 6.),
    C::db("outputDb", "Output", -30., 12., 0.),
];
pub struct NonlinearFilterPlugin;
impl Plugin for NonlinearFilterPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.nonlinear-filter",
                TABLE.map(C::spec).to_vec(),
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
    controls: [Controls<5>; 2],
    os: [Oversampler2x; 2],
    state: [[f64; 2]; 2],
}
impl Instance {
    fn new(rate: f64, block: u32) -> Self {
        Self {
            rate,
            controls: std::array::from_fn(|_| Controls::new(&TABLE, rate * 2.)),
            os: std::array::from_fn(|_| Oversampler2x::new(block as usize)),
            state: [[0.; 2]; 2],
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, block: u32) {
        *self = Self::new(rate, block);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for ch in 0..2 {
            let controls = &mut self.controls[ch];
            controls.events(ctx.parameter_events);
            let state = &mut self.state[ch];
            let rate = self.rate * 2.;
            let output =
                self.os[ch].process_roundtrip(&ctx.inputs[ch][..ctx.frames], &mut |band| {
                    for x in band {
                        let v = controls.next();
                        let g = (std::f64::consts::PI * v[1].min(rate * 0.225) / rate).tan();
                        let k = 2. - 1.94 * v[2];
                        let input = (*x as f64 * 10f64.powf(v[3] / 20.)).tanh();
                        let b = (state[0] + g * (input - state[1])) / (1. + g * (g + k));
                        let l = state[1] + g * b;
                        state[0] = (2. * b - state[0]).tanh();
                        state[1] = (2. * l - state[1]).tanh();
                        let h = input - k * b - l;
                        let y = match v[0] as u8 {
                            1 => h,
                            2 => b,
                            3 => h + l,
                            _ => l,
                        };
                        *x = (y * 10f64.powf(v[4] / 20.)) as f32;
                    }
                });
            ctx.outputs[ch][..ctx.frames].copy_from_slice(output);
        }
    }
    fn reset(&mut self) {
        self.state = [[0.; 2]; 2];
        for c in &mut self.controls {
            c.reset();
        }
        for o in &mut self.os {
            o.reset();
        }
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        24
    }
}
