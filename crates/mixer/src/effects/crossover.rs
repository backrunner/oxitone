//! Fourth-order Linkwitz-Riley split, including phase compensation for three bands.
use oxitone_dsp::biquad::{design, BiquadF64, BiquadKind};
struct Split {
    low: [BiquadF64; 2],
    high: [BiquadF64; 2],
}
impl Split {
    fn new(rate: f64, hz: f64) -> Self {
        Self {
            low: std::array::from_fn(|_| {
                BiquadF64::new(design(
                    BiquadKind::Lowpass,
                    rate,
                    hz,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.,
                ))
            }),
            high: std::array::from_fn(|_| {
                BiquadF64::new(design(
                    BiquadKind::Highpass,
                    rate,
                    hz,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.,
                ))
            }),
        }
    }
    fn update(&mut self, rate: f64, hz: f64) {
        let lp = design(
            BiquadKind::Lowpass,
            rate,
            hz.min(rate * 0.45),
            std::f64::consts::FRAC_1_SQRT_2,
            0.,
        );
        let hp = design(
            BiquadKind::Highpass,
            rate,
            hz.min(rate * 0.45),
            std::f64::consts::FRAC_1_SQRT_2,
            0.,
        );
        for b in &mut self.low {
            b.set_coeffs(lp);
        }
        for b in &mut self.high {
            b.set_coeffs(hp);
        }
    }
    fn next(&mut self, x: f32) -> [f32; 2] {
        let mut low = x;
        let mut high = x;
        for b in &mut self.low {
            low = b.next(low);
        }
        for b in &mut self.high {
            high = b.next(high);
        }
        [low, high]
    }
    fn reset(&mut self) {
        for b in self.low.iter_mut().chain(&mut self.high) {
            b.reset();
        }
    }
}
pub(super) struct Crossover {
    first: Split,
    upper: Split,
    compensation: Split,
}
impl Crossover {
    pub fn new(rate: f64, low: f64, high: f64) -> Self {
        Self {
            first: Split::new(rate, low),
            upper: Split::new(rate, high),
            compensation: Split::new(rate, high),
        }
    }
    pub fn update(&mut self, rate: f64, low: f64, high: f64) {
        self.first.update(rate, low);
        self.upper.update(rate, high);
        self.compensation.update(rate, high);
    }
    pub fn next(&mut self, x: f32) -> [f32; 3] {
        let [low, upper] = self.first.next(x);
        let [mid, high] = self.upper.next(upper);
        let [a, b] = self.compensation.next(low);
        [a + b, mid, high]
    }
    pub fn reset(&mut self) {
        self.first.reset();
        self.upper.reset();
        self.compensation.reset();
    }
}
