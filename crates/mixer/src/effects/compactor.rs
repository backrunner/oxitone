//! Upward density compression and stereo-linked transient shaping.
use super::controls::{Control as C, Controls};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 6] = [
    C::db("thresholdDb", "Threshold", -60., 0., -24.),
    C::db("upwardDb", "Upward Range", 0., 24., 8.),
    C::log("attackMs", "Attack", 0.1, 100., 5.),
    C::log("releaseMs", "Release", 10., 1000., 120.),
    C::linear("transient", "Transient", -1., 1., 0.),
    C::db("outputDb", "Output", -24., 12., 0.),
];
pub struct CompactorPlugin;
impl Plugin for CompactorPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.compactor",
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
    controls: Controls<6>,
    envelope: f64,
    slow: f64,
    gain_db: f64,
}
impl Instance {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            envelope: 0.,
            slow: 0.,
            gain_db: 0.,
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
            let peak = ctx.inputs[0][n].abs().max(ctx.inputs[1][n].abs()) as f64;
            let attack = (-1. / (v[2] * 0.001 * self.rate)).exp();
            let release = (-1. / (v[3] * 0.001 * self.rate)).exp();
            self.envelope = peak
                + (self.envelope - peak)
                    * if peak > self.envelope {
                        attack
                    } else {
                        release
                    };
            self.slow = peak + (self.slow - peak) * (-1. / (0.04 * self.rate)).exp();
            let level = 20. * self.envelope.max(1e-12).log10();
            let upward = (v[0] - level).max(0.).min(v[1]) * ((level + 84.) / 18.).clamp(0., 1.);
            let transient =
                (20. * ((peak + 1e-6) / (self.slow + 1e-6)).log10()).clamp(0., 12.) * v[4];
            let target = upward + transient;
            self.gain_db = target
                + (self.gain_db - target)
                    * if target < self.gain_db {
                        attack
                    } else {
                        release
                    };
            let gain = super::db_to_linear(self.gain_db + v[5]);
            for ch in 0..2 {
                ctx.outputs[ch][n] = ctx.inputs[ch][n] * gain;
            }
            if self.envelope < 1e-20 {
                self.envelope = 0.;
            }
            if self.slow < 1e-20 {
                self.slow = 0.;
            }
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        self.envelope = 0.;
        self.slow = 0.;
        self.gain_db = 0.;
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
