//! M/S width plus high-band decorrelation. The mid signal is preserved exactly.
use super::controls::{Control as C, Controls};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 3] = [
    C::linear("width", "Width", 0., 2., 1.),
    C::linear("amount", "Decorrelation", 0., 1., 0.4),
    C::hz("bassMonoHz", "Bass Mono", 20., 1000., 180.),
];
pub struct SpreaderPlugin;
impl Plugin for SpreaderPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.spreader",
                TABLE.map(C::spec).to_vec(),
                PluginCapabilities::default(),
            )
        })
    }
    fn create(&self, h: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(h.sample_rate))
    }
}
struct Instance {
    rate: f64,
    controls: Controls<3>,
    low: [[f64; 2]; 2],
    allpass: [f64; 4],
}
impl Instance {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            low: [[0.; 2]; 2],
            allpass: [0.; 4],
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, _: u32) {
        *self = Self::new(rate);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        self.controls.events(ctx.parameter_events);
        for n in 0..ctx.frames {
            let v = self.controls.next();
            let mid = (ctx.inputs[0][n] as f64 + ctx.inputs[1][n] as f64) * 0.5;
            let side = (ctx.inputs[0][n] as f64 - ctx.inputs[1][n] as f64) * 0.5;
            let coefficient = 1. - (-std::f64::consts::TAU * v[2] / self.rate).exp();
            let mut high = [mid, side];
            for (channel, value) in high.iter_mut().enumerate() {
                for state in &mut self.low[channel] {
                    *state += coefficient * (*value - *state);
                    *value -= *state;
                    if state.abs() < 1e-20 {
                        *state = 0.;
                    }
                }
            }
            let mut decor = high[0];
            for (state, a) in self.allpass.iter_mut().zip([0.47, -0.63, 0.81, -0.31]) {
                let y = *state - a * decor;
                *state = decor + a * y;
                if state.abs() < 1e-20 {
                    *state = 0.;
                }
                decor = y;
            }
            let side = (high[1] + (decor - high[0]) * v[1] * 0.5) * v[0];
            ctx.outputs[0][n] = (mid + side) as f32;
            ctx.outputs[1][n] = (mid - side) as f32;
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        self.low = [[0.; 2]; 2];
        self.allpass = [0.; 4];
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
