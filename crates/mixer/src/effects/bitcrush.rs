use super::controls::{Control as C, Controls};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 4] = [
    C::linear("bits", "Bits", 2., 24., 10.),
    C::hz("rateHz", "Sample Rate", 200., 48000., 12000.),
    C::linear("jitter", "Clock Jitter", 0., 1., 0.),
    C::db("outputDb", "Output", -24., 12., 0.),
];
pub struct BitcrushPlugin;
impl Plugin for BitcrushPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.bitcrush",
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
    controls: Controls<4>,
    hold: [f32; 2],
    remaining: f64,
    random: u32,
}
impl Instance {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            hold: [0.; 2],
            remaining: 0.,
            random: 0x192867ab,
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
            if self.remaining <= 0. {
                self.random ^= self.random << 13;
                self.random ^= self.random >> 17;
                self.random ^= self.random << 5;
                let jitter = self.random as f64 / u32::MAX as f64 - 0.5;
                self.remaining += (self.rate / v[1].min(self.rate) * (1. + jitter * v[2])).max(1.);
                let steps = 2f64.powf(v[0] - 1.);
                for ch in 0..2 {
                    self.hold[ch] = ((ctx.inputs[ch][n] as f64 * steps).round() / steps) as f32;
                }
            }
            self.remaining -= 1.;
            let gain = super::db_to_linear(v[3]);
            for ch in 0..2 {
                ctx.outputs[ch][n] = self.hold[ch] * gain;
            }
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        self.hold = [0.; 2];
        self.remaining = 0.;
        self.random = 0x192867ab;
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
