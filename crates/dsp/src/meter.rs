//! Peak/RMS metering helpers for mixer channels. All functions are RT-safe;
//! meters only write plain state, atomics/rings live in the mixer crate.

/// Absolute peak of a block. RT-safe.
pub fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()))
}

/// RMS of a block, computed with an `f64` accumulator. RT-safe.
pub fn rms(buf: &[f32]) -> f32 {
    if buf.is_empty() {
        return 0.0;
    }
    let sum: f64 = buf.iter().map(|&x| (x as f64) * (x as f64)).sum();
    (sum / buf.len() as f64).sqrt() as f32
}

/// Accumulating meter over multiple blocks; read and `reset` from the
/// control side at display rate.
pub struct Meter {
    peak: f32,
    sum_sq: f64,
    frames: u64,
}

impl Meter {
    pub fn new() -> Self {
        Self {
            peak: 0.0,
            sum_sq: 0.0,
            frames: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Fold one block into the accumulator. RT-safe.
    pub fn add_block(&mut self, buf: &[f32]) {
        for &x in buf {
            self.peak = self.peak.max(x.abs());
            self.sum_sq += (x as f64) * (x as f64);
        }
        self.frames += buf.len() as u64;
    }

    pub fn peak(&self) -> f32 {
        self.peak
    }

    pub fn rms(&self) -> f32 {
        if self.frames == 0 {
            0.0
        } else {
            (self.sum_sq / self.frames as f64).sqrt() as f32
        }
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_and_rms_of_known_block() {
        let buf = [0.5f32, -0.5, 0.5, -0.5];
        assert_eq!(peak(&buf), 0.5);
        assert!((rms(&buf) - 0.5).abs() < 1e-7);
    }

    #[test]
    fn accumulator_matches_single_shot() {
        let a = [0.25f32; 128];
        let b = [-0.75f32; 128];
        let mut m = Meter::new();
        m.add_block(&a);
        m.add_block(&b);
        let expect = (((0.25f64 * 0.25 + 0.75 * 0.75) / 2.0) as f32).sqrt();
        assert!((m.rms() - expect).abs() < 1e-6);
        assert_eq!(m.peak(), 0.75);
        assert_eq!(m.frames(), 256);
        m.reset();
        assert_eq!(m.rms(), 0.0);
    }
}
