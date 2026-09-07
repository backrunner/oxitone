//! Stereo-linked three-band upward/downward dynamics; original curves and timing.
use super::{
    controls::{Control as C, Controls},
    crossover::Crossover,
};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 10] = [
    C::linear("depth", "Depth", 0., 1., 0.5),
    C::log("time", "Time Scale", 0.1, 10., 1.),
    C::db("upwardDb", "Upward Range", 0., 36., 18.),
    C::log("downwardRatio", "Downward Ratio", 1., 40., 8.),
    C::hz("lowHz", "Low Crossover", 60., 500., 180.),
    C::hz("highHz", "High Crossover", 1000., 8000., 2800.),
    C::db("lowGainDb", "Low Gain", -18., 18., 0.),
    C::db("midGainDb", "Mid Gain", -18., 18., 0.),
    C::db("highGainDb", "High Gain", -18., 18., 0.),
    C::db("outputDb", "Output", -24., 12., 0.),
];
pub struct MultibandDynamicsPlugin;
impl Plugin for MultibandDynamicsPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.multiband-dynamics",
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
    controls: Controls<10>,
    split: [Crossover; 2],
    tick: u8,
    envelope: [f64; 3],
    gain_db: [f64; 3],
}
impl Instance {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            split: std::array::from_fn(|_| Crossover::new(rate, 180., 2800.)),
            tick: 0,
            envelope: [0.; 3],
            gain_db: [0.; 3],
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
            if self.tick == 0 {
                for split in &mut self.split {
                    split.update(self.rate, v[4], v[5]);
                }
            }
            self.tick = (self.tick + 1) % 32;
            let bands = [
                self.split[0].next(ctx.inputs[0][n]),
                self.split[1].next(ctx.inputs[1][n]),
            ];
            let mut gain = [0f32; 3];
            for b in 0..3 {
                let peak = bands[0][b].abs().max(bands[1][b].abs()) as f64;
                let attack = (-1. / ([0.012, 0.004, 0.0015][b] * v[1] * self.rate)).exp();
                let release = (-1. / ([0.18, 0.1, 0.06][b] * v[1] * self.rate)).exp();
                self.envelope[b] = peak
                    + (self.envelope[b] - peak)
                        * if peak > self.envelope[b] {
                            attack
                        } else {
                            release
                        };
                if self.envelope[b] < 1e-20 {
                    self.envelope[b] = 0.;
                }
                let level = 20. * self.envelope[b].max(1e-12).log10();
                let down = -(level - [-24., -22., -26.][b]).max(0.) * (1. - 1. / v[3]);
                let up = ([-42., -40., -44.][b] - level).max(0.).min(v[2])
                    * ((level + 84.) / 18.).clamp(0., 1.);
                let target = down + up;
                self.gain_db[b] = target
                    + (self.gain_db[b] - target)
                        * if target < self.gain_db[b] {
                            attack
                        } else {
                            release
                        };
                gain[b] = super::db_to_linear((self.gain_db[b] + v[6 + b]) * v[0] + v[9]);
            }
            for ch in 0..2 {
                ctx.outputs[ch][n] = (0..3).map(|b| bands[ch][b] * gain[b]).sum();
            }
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        for s in &mut self.split {
            s.reset();
        }
        self.tick = 0;
        self.envelope = [0.; 3];
        self.gain_db = [0.; 3];
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
