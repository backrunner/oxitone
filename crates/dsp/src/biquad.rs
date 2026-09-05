//! RBJ biquad family. Coefficients are computed in `f64` (`design`); the
//! default processor keeps `f32` state, and `BiquadF64` keeps `f64` state
//! for low cutoffs where `f32` coefficients/state diverge
//! (03-audio-runtime-spec.md §数值精度). All `next`/`process` are RT-safe.

use core::f64::consts::PI;

use crate::ftz::{flush_denormal, flush_denormal_f64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiquadKind {
    Lowpass,
    Highpass,
    /// Constant 0 dB peak gain.
    Bandpass,
    Peak,
    LowShelf,
    HighShelf,
}

/// Normalized coefficients (`a0 == 1`), always `f64`.
#[derive(Debug, Clone, Copy)]
pub struct BiquadCoeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

/// RBJ audio-eq-cookbook design. `gain_db` is used by Peak/LowShelf/
/// HighShelf. `cutoff_hz` is clamped to (0, fs/2), `q` to > 1e-3.
/// Control-thread/prepare only (transcendental math).
pub fn design(
    kind: BiquadKind,
    sample_rate: f64,
    cutoff_hz: f64,
    q: f64,
    gain_db: f64,
) -> BiquadCoeffs {
    let f = cutoff_hz.clamp(1e-3, sample_rate / 2.0 - 1e-3);
    let q = q.max(1e-3);
    let w0 = 2.0 * PI * f / sample_rate;
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * q);
    let a = 10f64.powf(gain_db / 40.0);
    let sa = 2.0 * a.sqrt() * alpha;
    let (b0, b1, b2, a0, a1, a2) = match kind {
        BiquadKind::Lowpass => (
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ),
        BiquadKind::Highpass => (
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ),
        BiquadKind::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
        BiquadKind::Peak => (
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        ),
        BiquadKind::LowShelf => (
            a * ((a + 1.0) - (a - 1.0) * cos + sa),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - sa),
            (a + 1.0) + (a - 1.0) * cos + sa,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - sa,
        ),
        BiquadKind::HighShelf => (
            a * ((a + 1.0) + (a - 1.0) * cos + sa),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - sa),
            (a + 1.0) - (a - 1.0) * cos + sa,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - sa,
        ),
    };
    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Transposed direct-form-II biquad with `f32` state. States are flushed to
/// zero on entry into the subnormal range. RT-safe.
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl Biquad {
    pub fn new(coeffs: BiquadCoeffs) -> Self {
        let mut b = Self {
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            s1: 0.0,
            s2: 0.0,
        };
        b.set_coeffs(coeffs);
        b
    }

    /// Block-boundary coefficient update (control rate).
    pub fn set_coeffs(&mut self, c: BiquadCoeffs) {
        self.b0 = c.b0 as f32;
        self.b1 = c.b1 as f32;
        self.b2 = c.b2 as f32;
        self.a1 = c.a1 as f32;
        self.a2 = c.a2 as f32;
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline]
    pub fn next(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = flush_denormal(self.b1 * x - self.a1 * y + self.s2);
        self.s2 = flush_denormal(self.b2 * x - self.a2 * y);
        y
    }

    /// In-place block processing. RT-safe.
    pub fn process(&mut self, buf: &mut [f32]) {
        for x in buf.iter_mut() {
            *x = self.next(*x);
        }
    }
}

/// `f64`-state variant for low cutoffs (03-audio-runtime-spec.md §数值精度).
/// States below 1e-20 are flushed to keep the tail out of any denormal
/// range. RT-safe.
pub struct BiquadF64 {
    coeffs: BiquadCoeffs,
    s1: f64,
    s2: f64,
}

impl BiquadF64 {
    pub fn new(coeffs: BiquadCoeffs) -> Self {
        Self {
            coeffs,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline]
    pub fn next(&mut self, x: f32) -> f32 {
        let c = &self.coeffs;
        let y = c.b0 * x as f64 + self.s1;
        self.s1 = flush_small(c.b1 * x as f64 - c.a1 * y + self.s2);
        self.s2 = flush_small(c.b2 * x as f64 - c.a2 * y);
        y as f32
    }

    /// In-place block processing. RT-safe.
    pub fn process(&mut self, buf: &mut [f32]) {
        for x in buf.iter_mut() {
            *x = self.next(*x);
        }
    }
}

/// Process a stereo block through the paired left/right `BiquadF64`
/// instances of one band in a single loop. Each channel applies exactly the
/// same per-sample operations in the same order as [`BiquadF64::process`]
/// (bit-identical output); interleaving the two independent recurrences lets
/// the CPU overlap their loop-carried latencies. RT-safe.
pub fn process_stereo_f64(
    left: &mut BiquadF64,
    right: &mut BiquadF64,
    left_buf: &mut [f32],
    right_buf: &mut [f32],
) {
    debug_assert_eq!(left_buf.len(), right_buf.len());
    for (x, y) in left_buf.iter_mut().zip(right_buf.iter_mut()) {
        let xl = *x;
        let xr = *y;
        *x = left.next(xl);
        *y = right.next(xr);
    }
}

#[inline]
fn flush_small(x: f64) -> f64 {
    flush_denormal_f64(if x.abs() < 1e-20 { 0.0 } else { x })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::TAU;

    /// DFT magnitude of the impulse response at `freq_hz` (coarse probe).
    fn response_at(kind: BiquadKind, cutoff: f64, q: f64, gain_db: f64, freq: f64) -> f64 {
        let sr = 48_000.0;
        let mut b = Biquad::new(design(kind, sr, cutoff, q, gain_db));
        let n = 8192;
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..n {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let y = b.next(x) as f64;
            let ang = -TAU * freq * i as f64 / sr;
            re += y * ang.cos();
            im += y * ang.sin();
        }
        (re * re + im * im).sqrt()
    }

    #[test]
    fn lowpass_passes_low_rejects_high() {
        let low = response_at(BiquadKind::Lowpass, 1000.0, 0.707, 0.0, 100.0);
        let high = response_at(BiquadKind::Lowpass, 1000.0, 0.707, 0.0, 10_000.0);
        assert!((low - 1.0).abs() < 0.05, "passband {low}");
        assert!(high < 0.02, "stopband {high}");
    }

    #[test]
    fn highpass_is_the_mirror() {
        let low = response_at(BiquadKind::Highpass, 1000.0, 0.707, 0.0, 100.0);
        let high = response_at(BiquadKind::Highpass, 1000.0, 0.707, 0.0, 10_000.0);
        assert!(low < 0.02, "stopband {low}");
        assert!((high - 1.0).abs() < 0.05, "passband {high}");
    }

    #[test]
    fn bandpass_peaks_at_center() {
        let center = response_at(BiquadKind::Bandpass, 2000.0, 2.0, 0.0, 2000.0);
        let far = response_at(BiquadKind::Bandpass, 2000.0, 2.0, 0.0, 200.0);
        assert!((center - 1.0).abs() < 0.1, "center {center}");
        assert!(far < 0.2, "far {far}");
    }

    #[test]
    fn peak_boosts_by_gain_db() {
        let center = response_at(BiquadKind::Peak, 1000.0, 1.0, 6.0, 1000.0);
        let unity = response_at(BiquadKind::Peak, 1000.0, 1.0, 6.0, 100.0);
        assert!((center - 2.0).abs() < 0.2, "boost {center}");
        assert!((unity - 1.0).abs() < 0.1, "unity {unity}");
    }

    #[test]
    fn shelves_split_low_and_high() {
        let low = response_at(BiquadKind::LowShelf, 500.0, 0.707, 12.0, 50.0);
        let high = response_at(BiquadKind::LowShelf, 500.0, 0.707, 12.0, 10_000.0);
        assert!((low - 4.0).abs() < 0.8, "shelf {low}");
        assert!((high - 1.0).abs() < 0.1, "unity {high}");
        let low = response_at(BiquadKind::HighShelf, 2000.0, 0.707, 12.0, 50.0);
        let high = response_at(BiquadKind::HighShelf, 2000.0, 0.707, 12.0, 10_000.0);
        assert!((low - 1.0).abs() < 0.1, "unity {low}");
        assert!((high - 4.0).abs() < 0.8, "shelf {high}");
    }

    #[test]
    fn f64_state_handles_low_cutoff() {
        let mut b = BiquadF64::new(design(BiquadKind::Lowpass, 48_000.0, 10.0, 0.707, 0.0));
        let mut impulse = [0.0f32; 65_536];
        impulse[0] = 1.0;
        b.process(&mut impulse);
        assert!(impulse.iter().all(|x| x.is_finite()));
        let tail_energy: f64 = impulse[32_768..].iter().map(|&x| (x as f64).powi(2)).sum();
        let head_energy: f64 = impulse[..1024].iter().map(|&x| (x as f64).powi(2)).sum();
        assert!(tail_energy < head_energy * 1e-3, "tail {tail_energy}");
    }

    #[test]
    fn impulse_tail_flushes_to_exact_zero() {
        let mut b = Biquad::new(design(BiquadKind::Lowpass, 48_000.0, 100.0, 0.707, 0.0));
        let mut buf = [0.0f32; 300_000];
        buf[0] = 1.0;
        b.process(&mut buf);
        assert!(buf[200_000..].iter().all(|&x| x == 0.0));
    }
}
