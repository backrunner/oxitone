//! Smoothed filtering, stereo width and input-driven wet ducking for spatial effects.
use super::controls::{Control as C, Controls};
use oxitone_graph::ProcessContext;
const TABLE: [C; 4] = [
    C::hz("highpassHz", "Wet Highpass", 20., 2000., 20.),
    C::hz("lowpassHz", "Wet Lowpass", 1000., 20000., 20000.),
    C::linear("ducking", "Wet Ducking", 0., 1., 0.),
    C::linear("width", "Width", 0., 2., 1.),
];
pub(super) fn specs() -> impl Iterator<Item = oxitone_core::wire::ParameterSpec> {
    TABLE.into_iter().map(C::spec)
}
pub(super) struct WetShape {
    rate: f64,
    controls: Controls<4>,
    low: [[f64; 2]; 2],
    envelope: f64,
}
impl WetShape {
    pub fn new(rate: f64) -> Self {
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            low: [[0.; 2]; 2],
            envelope: 0.,
        }
    }
    pub fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        self.controls.events(ctx.parameter_events);
        let attack = (-1. / (0.005 * self.rate)).exp();
        let release = (-1. / (0.15 * self.rate)).exp();
        for n in 0..ctx.frames {
            let v = self.controls.next();
            let peak = ctx.inputs[0][n].abs().max(ctx.inputs[1][n].abs()) as f64;
            self.envelope = peak
                + (self.envelope - peak)
                    * if peak > self.envelope {
                        attack
                    } else {
                        release
                    };
            if self.envelope < 1e-20 {
                self.envelope = 0.;
            }
            let hp = 1. - (-std::f64::consts::TAU * v[0] / self.rate).exp();
            let lp = 1. - (-std::f64::consts::TAU * v[1].min(self.rate * 0.45) / self.rate).exp();
            let mut wet = [0.; 2];
            for (ch, y) in wet.iter_mut().enumerate() {
                let x = ctx.outputs[ch][n] as f64;
                self.low[ch][0] += hp * (x - self.low[ch][0]);
                let high = if v[0] <= 20.0001 {
                    x
                } else {
                    x - self.low[ch][0]
                };
                self.low[ch][1] += lp * (high - self.low[ch][1]);
                *y = if v[1] >= 19999.99 {
                    high
                } else {
                    self.low[ch][1]
                };
                for s in &mut self.low[ch] {
                    if s.abs() < 1e-20 {
                        *s = 0.;
                    }
                }
            }
            let gain = 1. / (1. + self.envelope * v[2] * 12.);
            let mid = (wet[0] + wet[1]) * 0.5;
            let side = (wet[0] - wet[1]) * 0.5 * v[3];
            ctx.outputs[0][n] = ((mid + side) * gain) as f32;
            ctx.outputs[1][n] = ((mid - side) * gain) as f32;
        }
    }
    pub fn reset(&mut self) {
        self.controls.reset();
        self.low = [[0.; 2]; 2];
        self.envelope = 0.;
    }
}
