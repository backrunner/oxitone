use super::{
    controls::{Control as C, Controls},
    fractional::FractionalDelay,
};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 5] = [
    C::hz("rateHz", "Rate", 0.01, 10., 0.25),
    C::log("delayMs", "Delay", 0.2, 10., 2.),
    C::linear("depthMs", "Depth", 0., 10., 2.),
    C::linear("feedback", "Feedback", -0.95, 0.95, 0.35),
    C::linear("stereo", "Stereo Phase", 0., 1., 0.5),
];
pub struct FlangerPlugin;
impl Plugin for FlangerPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.flanger",
                TABLE.map(C::spec).to_vec(),
                PluginCapabilities {
                    reports_tail: true,
                    sidechain_input: false,
                },
            )
        })
    }
    fn create(&self, h: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(h.sample_rate))
    }
}
struct Instance {
    rate: f64,
    controls: Controls<5>,
    lines: [FractionalDelay; 2],
    phase: f64,
    tail: u64,
}
impl Instance {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            lines: std::array::from_fn(|_| FractionalDelay::new((rate * 0.022) as usize + 4)),
            phase: 0.,
            tail: 0,
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
            self.phase = (self.phase + v[0] / self.rate).fract();
            let mut active = false;
            for ch in 0..2 {
                let lfo = (std::f64::consts::TAU * (self.phase + ch as f64 * v[4] * 0.5)).sin();
                let delay = (v[1] + v[2] * (lfo + 1.) * 0.5) * 0.001 * self.rate;
                let y = self.lines[ch].read(delay);
                let x = ctx.inputs[ch][n];
                self.lines[ch].push(x + y * v[3] as f32);
                // Standard feed-forward comb at full insert mix, host can blend further.
                ctx.outputs[ch][n] = 0.5 * (x + y);
                active |= x.abs() > 1e-9;
            }
            self.tail = if active {
                (self.rate * 5.) as u64
            } else {
                self.tail.saturating_sub(1)
            };
        }
    }
    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.controls.reset();
        self.phase = 0.;
        self.tail = 0;
    }
    fn tail_frames(&self) -> u64 {
        self.tail
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
