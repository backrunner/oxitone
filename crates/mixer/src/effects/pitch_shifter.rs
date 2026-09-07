//! Two complementary Hann delay grains. Fixed 50 ms window, duration preserving.
use super::{
    controls::{Control as C, Controls},
    fractional::FractionalDelay,
};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TABLE: [C; 3] = [
    C::linear("semitones", "Semitones", -24., 24., 7.),
    C::linear("cents", "Fine", -100., 100., 0.),
    C::db("outputDb", "Output", -24., 12., 0.),
];
pub struct PitchShifterPlugin;
impl Plugin for PitchShifterPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.pitch-shifter",
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
    controls: Controls<3>,
    lines: [FractionalDelay; 2],
    phase: f64,
    window: usize,
    tail: u64,
    low: [[f64; 2]; 2],
}
impl Instance {
    fn new(rate: f64) -> Self {
        let window = ((rate * 0.05).round() as usize / 2 * 2).max(32);
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            lines: std::array::from_fn(|_| FractionalDelay::new(window + 16)),
            phase: 0.5,
            window,
            tail: 0,
            low: [[0.; 2]; 2],
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
            let ratio = 2f64.powf((v[0] + v[1] * 0.01) / 12.);
            let second = (self.phase + 0.5).fract();
            let weight = 0.5 - 0.5 * (std::f64::consts::TAU * self.phase).cos();
            let gain = super::db_to_linear(v[2]);
            let mut active = false;
            let coefficient = 1.
                - (-std::f64::consts::TAU * (self.rate * 0.45 / ratio.max(1.)) / self.rate).exp();
            for ch in 0..2 {
                let a = self.lines[ch].read(8. + self.phase * self.window as f64);
                let b = self.lines[ch].read(8. + second * self.window as f64);
                ctx.outputs[ch][n] = (a * weight as f32 + b * (1. - weight) as f32) * gain;
                let mut x = ctx.inputs[ch][n] as f64;
                active |= x.abs() > 1e-9;
                for state in &mut self.low[ch] {
                    *state += coefficient * (x - *state);
                    x = *state;
                }
                // Unity/downward shifts retain full bandwidth; upward shifts are band-limited.
                self.lines[ch].push(if ratio > 1.00001 {
                    x as f32
                } else {
                    ctx.inputs[ch][n]
                });
            }
            self.phase = (self.phase + (1. - ratio) / self.window as f64).rem_euclid(1.);
            self.tail = if active {
                (self.window + 8) as u64
            } else {
                self.tail.saturating_sub(1)
            };
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        self.phase = 0.5;
        self.tail = 0;
        self.low = [[0.; 2]; 2];
        for l in &mut self.lines {
            l.reset();
        }
    }
    fn tail_frames(&self) -> u64 {
        self.tail
    }
    fn latency_frames(&self) -> u64 {
        (self.window / 2 + 8) as u64
    }
}
