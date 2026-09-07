//! Fixed one-shot voices; all storage exists before the first audio block.
use std::f64::consts::TAU;

#[derive(Clone, Copy, Default)]
pub(crate) struct Voice {
    pub remaining: u64,
    age: u64,
    phase: f64,
    envelope: f64,
    falloff: f64,
    sweep: f64,
    sweep_falloff: f64,
    previous_noise: f64,
}

impl Voice {
    pub fn trigger(&mut self, pad: usize, velocity: f32, rate: f64, decay: f64) {
        let duration = [0.7, 0.5, 0.12, 0.65][pad] * decay;
        let time_constant = [0.16, 0.10, 0.025, 0.15][pad] * decay;
        *self = Self {
            remaining: (duration * rate) as u64,
            envelope: f64::from(velocity),
            falloff: (-1.0 / (time_constant * rate)).exp(),
            sweep: 130.0,
            sweep_falloff: (-45.0 / rate).exp(),
            ..Self::default()
        };
    }

    pub fn tick(&mut self, pad: usize, rate: f64, noise: f64, parameters: &[f64; 10]) -> f64 {
        if self.remaining == 0 {
            return 0.0;
        }
        let attack = (self.age as f64 / (rate * 0.001)).min(1.0);
        let fade = (self.remaining as f64 / (rate * 0.005)).min(1.0);
        let high_noise = (noise - self.previous_noise) * 0.5;
        self.previous_noise = noise;
        let (hz, tone) = match pad {
            0 => (
                parameters[0] + self.sweep * parameters[1] / 130.,
                self.phase.sin() * 0.95
                    + high_noise * parameters[2] * (-(self.age as f64) / (rate * 0.003)).exp(),
            ),
            1 => (
                parameters[3],
                self.phase.sin() * (0.25 + parameters[4] * 0.18)
                    + high_noise * (0.9 + parameters[4] * 0.55)
                    + (self.phase * 1.47).sin() * parameters[4] * 0.18,
            ),
            _ => (
                7320.0 - parameters[9] * 1800.,
                high_noise * (0.32 + parameters[9] * 0.12)
                    + self.phase.sin() * 0.07
                    + (self.phase * 1.413).sin() * parameters[9] * 0.045,
            ),
        };
        self.phase = (self.phase + TAU * hz / rate) % TAU;
        self.sweep *= self.sweep_falloff;
        let result = tone * self.envelope * attack * fade;
        self.envelope *= self.falloff;
        self.remaining -= 1;
        self.age += 1;
        result
    }
}
