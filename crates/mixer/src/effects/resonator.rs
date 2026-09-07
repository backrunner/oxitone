//! Four stable damped modes. Excitation normalization bounds energy as decay grows.
use super::controls::{Control as C, Controls};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 6] = [
    C::hz("frequencyHz", "Fundamental", 20., 4000., 220.),
    C::seconds("decaySeconds", "Decay", 0.02, 8., 0.8),
    C::linear("spread", "Stereo", 0., 1., 0.3),
    C::linear("brightness", "Brightness", 0., 1., 0.5),
    C::linear("inharmonicity", "Inharmonicity", 0., 1., 0.),
    C::db("outputDb", "Output", -36., 12., 0.),
];
pub struct ResonatorPlugin;
impl Plugin for ResonatorPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.resonator",
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
    controls: Controls<6>,
    modes: [[[f64; 2]; 4]; 2],
    tail: u64,
}
impl Instance {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            modes: [[[0.; 2]; 4]; 2],
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
            let mut active = false;
            for ch in 0..2 {
                let x = ctx.inputs[ch][n] as f64;
                active |= x.abs() > 1e-9;
                let mut wet = 0.;
                for m in 0..4 {
                    let harmonic = (m + 1) as f64;
                    let frequency = v[0]
                        * harmonic
                        * (1. + v[4] * m as f64 * 0.173)
                        * 2f64.powf((ch as f64 * 2. - 1.) * v[2] * m as f64 / 1200.);
                    let r = 10f64.powf(-3. * (1. + m as f64 * (1. - v[3])) / (v[1] * self.rate));
                    let phase = std::f64::consts::TAU * frequency.min(self.rate * 0.48) / self.rate;
                    let (sin, cos) = phase.sin_cos();
                    let state = &mut self.modes[ch][m];
                    let real = r * (cos * state[0] - sin * state[1]) + x * (1. - r).sqrt();
                    let imag = r * (sin * state[0] + cos * state[1]);
                    *state = [
                        if real.abs() < 1e-20 { 0. } else { real },
                        if imag.abs() < 1e-20 { 0. } else { imag },
                    ];
                    if frequency < self.rate * 0.48 {
                        wet += real / harmonic;
                    }
                }
                ctx.outputs[ch][n] = (wet * 0.25 * 10f64.powf(v[5] / 20.)) as f32;
            }
            self.tail = if active {
                (v[1] * self.rate * 2.) as u64
            } else {
                self.tail.saturating_sub(1)
            };
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        self.modes = [[[0.; 2]; 4]; 2];
        self.tail = 0;
    }
    fn tail_frames(&self) -> u64 {
        self.tail
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
