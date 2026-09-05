//! 1 LSB TPDF dither for the 16/24-bit export boundary. Driven by the
//! versioned `pcg32-v1` PRNG from oxitone-core so the same project seed
//! yields byte-identical renders (03-audio-runtime-spec.md §数值精度).
//! RT-safe (no allocation; deterministic PRNG only).

use oxitone_core::Pcg32;

/// Triangular-probability dither + quantizer, ±1 LSB peak-to-peak.
pub struct TpdfDither {
    rng: Pcg32,
}

impl TpdfDither {
    /// `seed` is derived from the project seed by the caller (export node).
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Pcg32::new(seed),
        }
    }

    /// One quantization step for `bits` (16 or 24), treating full scale as
    /// -1..1-LSB.
    pub fn lsb(bits: u8) -> f32 {
        assert!(
            bits == 16 || bits == 24,
            "only 16/24-bit export is dithered"
        );
        1.0 / (1u32 << (bits - 1)) as f32
    }

    /// Add 1 LSB TPDF noise and quantize `buf` in place to the `bits` grid.
    /// 32-bit float export and realtime output must not call this. RT-safe.
    pub fn process(&mut self, buf: &mut [f32], bits: u8) {
        let lsb = Self::lsb(bits);
        let scale = 1.0 / lsb;
        for x in buf.iter_mut() {
            let noise = (self.rng.next_f64() - self.rng.next_f64()) as f32;
            let q = ((*x + noise * lsb) * scale).round() * lsb;
            *x = q.clamp(-1.0, 1.0 - lsb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_is_byte_identical() {
        let input: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.001).sin() * 0.8).collect();
        let mut a = input.clone();
        let mut b = input.clone();
        TpdfDither::new(42).process(&mut a, 16);
        TpdfDither::new(42).process(&mut b, 16);
        assert_eq!(a, b);
        let mut c = input;
        TpdfDither::new(43).process(&mut c, 16);
        assert_ne!(a, c);
    }

    #[test]
    fn noise_rms_is_about_half_lsb_on_silence() {
        let mut buf = vec![0.0f32; 1 << 16];
        let lsb = TpdfDither::lsb(16);
        TpdfDither::new(7).process(&mut buf, 16);
        let rms = (buf.iter().map(|&x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
        assert!(
            rms > 0.35 * lsb && rms < 0.65 * lsb,
            "rms {rms} vs lsb {lsb}"
        );
    }

    #[test]
    fn dither_linearizes_dc() {
        let mut buf = vec![0.3f32; 1 << 16];
        let lsb = TpdfDither::lsb(16);
        TpdfDither::new(9).process(&mut buf, 16);
        let mean = buf.iter().map(|&x| f64::from(x)).sum::<f64>() / buf.len() as f64;
        assert!((mean - 0.3).abs() < f64::from(lsb) / 4.0, "mean {mean}");
    }

    #[test]
    fn output_stays_on_grid_and_in_range() {
        let mut buf = vec![1.5f32, -1.5, 0.999_999, -0.999_999];
        let lsb = TpdfDither::lsb(16);
        TpdfDither::new(1).process(&mut buf, 16);
        for &x in &buf {
            assert!((-1.0..=1.0 - lsb).contains(&x));
            assert!((x / lsb).fract().abs() < 1e-3);
        }
    }
}
