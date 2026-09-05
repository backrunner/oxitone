//! Linear gain, equal-power pan, and block-boundary control smoothers.
//! All functions are RT-safe.

/// Multiply `buf` by a constant linear gain. RT-safe.
pub fn apply_gain(buf: &mut [f32], gain: f32) {
    for x in buf.iter_mut() {
        *x *= gain;
    }
}

/// Multiply `buf` by a gain ramping linearly from `from` to `to`, reaching
/// `to` on the last sample. RT-safe.
pub fn apply_gain_ramp(buf: &mut [f32], from: f32, to: f32) {
    let len = buf.len();
    if len == 0 {
        return;
    }
    for (i, x) in buf.iter_mut().enumerate() {
        let t = (i as f32 + 1.0) / len as f32;
        *x *= from + (to - from) * t;
    }
}

/// Constant-power pan gains for `pan` in -1 (left)..1 (right).
/// `left² + right² == 1` for any pan. RT-safe.
pub fn equal_power_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) as f64 + 1.0) * core::f64::consts::FRAC_PI_4;
    (angle.cos() as f32, angle.sin() as f32)
}

/// Pan a mono buffer into stereo with the equal-power law. RT-safe.
pub fn pan_mono_to_stereo(input: &[f32], left: &mut [f32], right: &mut [f32], pan: f32) {
    let (gl, gr) = equal_power_gains(pan);
    for ((x, l), r) in input.iter().zip(left.iter_mut()).zip(right.iter_mut()) {
        *l = *x * gl;
        *r = *x * gr;
    }
}

/// One-pole (exponential) gain smoother. The target is updated at block
/// boundaries (control rate); `next`/`process_gain` are RT-safe.
pub struct OnePoleSmoother {
    value: f32,
    target: f32,
    coeff: f32,
}

impl OnePoleSmoother {
    /// `time_constant_ms` is the time to cover ~63% of a step.
    pub fn new(sample_rate: f64, time_constant_ms: f64) -> Self {
        let tc = (time_constant_ms.max(0.01)) / 1000.0;
        let coeff = 1.0 - (-1.0 / (tc * sample_rate)).exp();
        Self {
            value: 0.0,
            target: 0.0,
            coeff: coeff as f32,
        }
    }

    /// Set both value and target without smoothing (e.g. on graph swap).
    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
    }

    /// Block-boundary (control-rate) target update.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    /// Advance one sample. Snaps to the target once the residual is below
    /// 1e-5 so the tail never decays into the denormal range. RT-safe.
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        let d = self.target - self.value;
        if d.abs() < 1e-5 {
            self.value = self.target;
        } else {
            self.value += d * self.coeff;
        }
        self.value
    }

    /// Multiply `buf` by the smoothed gain. RT-safe.
    pub fn process_gain(&mut self, buf: &mut [f32]) {
        for x in buf.iter_mut() {
            *x *= self.next_sample();
        }
    }
}

/// Linear ramp smoother with a fixed sample count per segment.
pub struct LinearSmoother {
    value: f32,
    step: f32,
    remaining: u32,
}

impl LinearSmoother {
    pub fn new(initial: f32) -> Self {
        Self {
            value: initial,
            step: 0.0,
            remaining: 0,
        }
    }

    /// Block-boundary update: ramp to `target` over `ramp_samples` samples.
    pub fn set_target(&mut self, target: f32, ramp_samples: u32) {
        if ramp_samples == 0 {
            self.value = target;
            self.step = 0.0;
            self.remaining = 0;
        } else {
            self.step = (target - self.value) / ramp_samples as f32;
            self.remaining = ramp_samples;
        }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    /// RT-safe.
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        if self.remaining > 0 {
            self.value += self.step;
            self.remaining -= 1;
        }
        self.value
    }

    /// Multiply `buf` by the smoothed gain. RT-safe.
    pub fn process_gain(&mut self, buf: &mut [f32]) {
        for x in buf.iter_mut() {
            *x *= self.next_sample();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_power_center_is_minus_3db() {
        let (l, r) = equal_power_gains(0.0);
        assert!((l - core::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert!((r - core::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        for i in -10..=10 {
            let (l, r) = equal_power_gains(i as f32 / 10.0);
            assert!((l * l + r * r - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn ramp_reaches_target_on_last_sample() {
        let mut buf = vec![1.0f32; 8];
        apply_gain_ramp(&mut buf, 0.0, 1.0);
        assert_eq!(buf[7], 1.0);
        assert_eq!(buf[0], 0.125);
    }

    #[test]
    fn one_pole_converges_and_snaps() {
        let mut s = OnePoleSmoother::new(48_000.0, 1.0);
        s.snap(0.0);
        s.set_target(1.0);
        for _ in 0..100_000 {
            s.next_sample();
        }
        assert_eq!(s.value(), 1.0);
    }

    #[test]
    fn linear_smoother_hits_target_exactly() {
        let mut s = LinearSmoother::new(0.0);
        s.set_target(0.7, 64);
        for _ in 0..64 {
            s.next_sample();
        }
        assert!((s.value() - 0.7).abs() < 1e-6);
        assert_eq!(s.next_sample(), s.value());
    }
}
