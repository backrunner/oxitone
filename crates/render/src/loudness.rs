//! Export metering: sample peak, 4x oversampled true peak, and EBU R128
//! integrated loudness (06-format-and-export.md §Render report). Computed
//! streaming on the same rendered blocks that go to the WAV files — never a
//! second render pass.
//!
//! K-weighting uses the RBJ high-shelf (+4 dB at 1681.974 Hz, Q 0.7071)
//! and high-pass (38.135 Hz, Q 0.5003) designed at the working sample rate;
//! at 48 kHz these match the standard BS.1770 coefficients to within
//! floating-point design error (documented approximation for other rates).
//! Gating: 400 ms blocks with 75 % overlap, absolute gate −70 LUFS,
//! relative gate −10 LU, stereo channels weighted 1.0.

use oxitone_dsp::biquad::{design, Biquad, BiquadKind};
use oxitone_mixer::TruePeakMeter;

const SHELF_HZ: f64 = 1681.974450955533;
const SHELF_Q: f64 = 0.7071752369554196;
const SHELF_GAIN_DB: f64 = 3.99984385397;
const HP_HZ: f64 = 38.13547087602444;
const HP_Q: f64 = 0.5003270373238773;
/// Block/hop structure: 400 ms blocks, 100 ms hops.
const BLOCK_HOPS: usize = 4;
const ABSOLUTE_GATE: f64 = -70.0;
const RELATIVE_GATE_LU: f64 = 10.0;

/// Per-file accumulating loudness/peak meter. `add_block` is RT-safe except
/// for the 100 ms hop push (control-rate bookkeeping on the render thread,
/// bounded by the render length).
pub struct FileMeter {
    k: [[Biquad; 2]; 2],
    hop: usize,
    hop_pos: usize,
    acc: [f64; 2],
    hops: Vec<f64>,
    peak: f32,
    true_peak: TruePeakMeter,
    frames: u64,
}

impl FileMeter {
    pub fn new(sample_rate: u32, max_block: usize) -> Self {
        let sr = f64::from(sample_rate);
        let stage = |kind, hz, q, gain| Biquad::new(design(kind, sr, hz, q, gain));
        Self {
            k: [
                [
                    stage(BiquadKind::HighShelf, SHELF_HZ, SHELF_Q, SHELF_GAIN_DB),
                    stage(BiquadKind::Highpass, HP_HZ, HP_Q, 0.0),
                ],
                [
                    stage(BiquadKind::HighShelf, SHELF_HZ, SHELF_Q, SHELF_GAIN_DB),
                    stage(BiquadKind::Highpass, HP_HZ, HP_Q, 0.0),
                ],
            ],
            hop: (sample_rate as usize) / 10,
            hop_pos: 0,
            acc: [0.0; 2],
            hops: Vec::new(),
            peak: 0.0,
            true_peak: TruePeakMeter::new(max_block),
            frames: 0,
        }
    }

    pub fn add_block(&mut self, left: &[f32], right: &[f32]) {
        debug_assert_eq!(left.len(), right.len());
        // One pass over both channels: the two K-weighting chains are
        // independent recurrences, so interleaving them in a single loop
        // overlaps their latencies. Per-channel operation order (and the
        // hop-boundary bookkeeping) is unchanged from channel-major
        // processing, so results are identical.
        let [k_l, k_r] = &mut self.k;
        let mut acc_l = self.acc[0];
        let mut acc_r = self.acc[1];
        let mut peak = self.peak;
        for (&xl, &xr) in left.iter().zip(right.iter()) {
            peak = peak.max(xl.abs()).max(xr.abs());
            let sl = k_l[0].next(xl);
            let yl = k_l[1].next(sl);
            let sr = k_r[0].next(xr);
            let yr = k_r[1].next(sr);
            acc_l += (yl as f64) * (yl as f64);
            acc_r += (yr as f64) * (yr as f64);
            self.hop_pos += 1;
            if self.hop_pos == self.hop {
                self.hop_pos = 0;
                self.hops.push(acc_l + acc_r);
                acc_l = 0.0;
                acc_r = 0.0;
            }
        }
        self.acc = [acc_l, acc_r];
        self.peak = peak;
        self.true_peak.add_block(left, right);
        self.frames += left.len() as u64;
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// `(peak_dbfs, true_peak_dbfs, integrated_lufs)`. Empty/silent renders
    /// report `-inf` loudness and peaks.
    pub fn finish(&self) -> (f64, f64, f64) {
        let to_db = |v: f32| {
            if v > 0.0 {
                20.0 * (v as f64).log10()
            } else {
                f64::NEG_INFINITY
            }
        };
        (
            to_db(self.peak),
            to_db(self.true_peak.true_peak()),
            self.integrated(),
        )
    }

    /// EBU R128 integrated loudness with absolute + relative gating.
    fn integrated(&self) -> f64 {
        if self.hops.len() < BLOCK_HOPS {
            return f64::NEG_INFINITY;
        }
        let hop = self.hop as f64;
        let block_z: Vec<f64> = self
            .hops
            .windows(BLOCK_HOPS)
            .map(|w| w.iter().sum::<f64>() / (BLOCK_HOPS as f64 * hop))
            .collect();
        let loudness = |z: f64| -0.691 + 10.0 * z.log10();
        let gated: Vec<f64> = block_z
            .iter()
            .copied()
            .filter(|&z| z > 0.0 && loudness(z) >= ABSOLUTE_GATE)
            .collect();
        if gated.is_empty() {
            return f64::NEG_INFINITY;
        }
        let mean_z = gated.iter().sum::<f64>() / gated.len() as f64;
        let threshold = loudness(mean_z) - RELATIVE_GATE_LU;
        let final_z: Vec<f64> = gated
            .iter()
            .copied()
            .filter(|&z| loudness(z) > threshold)
            .collect();
        if final_z.is_empty() {
            return f64::NEG_INFINITY;
        }
        loudness(final_z.iter().sum::<f64>() / final_z.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render `frames` of a stereo sine through the meter.
    fn meter_sine(sample_rate: u32, seconds: f64, amplitude: f32, hz: f64) -> FileMeter {
        let mut meter = FileMeter::new(sample_rate, 128);
        let total = (seconds * f64::from(sample_rate)) as usize;
        let mut done = 0;
        while done < total {
            let n = (total - done).min(128);
            let l: Vec<f32> = (0..n)
                .map(|i| {
                    amplitude
                        * (2.0 * std::f32::consts::PI * hz as f32 * (done + i) as f32
                            / sample_rate as f32)
                            .sin()
                })
                .collect();
            let r = l.clone();
            meter.add_block(&l, &r);
            done += n;
        }
        meter
    }

    #[test]
    fn integrated_loudness_of_1khz_sine_matches_r128_reference() {
        // BS.1770 reference: a 1 kHz sine at 0.5 FS per channel measures
        // about -6.0 LUFS integrated (stereo, equal weights).
        let meter = meter_sine(48_000, 4.0, 0.5, 1000.0);
        let (peak, _tp, lufs) = meter.finish();
        assert!((peak - (-6.02)).abs() < 0.1, "peak {peak}");
        assert!((lufs - (-6.0)).abs() < 0.3, "lufs {lufs}");
    }

    #[test]
    fn silence_reports_minus_infinity() {
        let meter = FileMeter::new(48_000, 128);
        let (peak, tp, lufs) = meter.finish();
        assert!(peak.is_infinite() && peak < 0.0);
        assert!(tp.is_infinite() && tp < 0.0);
        assert!(lufs.is_infinite() && lufs < 0.0);
    }

    #[test]
    fn louder_signal_measures_louder() {
        let quiet = meter_sine(48_000, 4.0, 0.1, 440.0);
        let loud = meter_sine(48_000, 4.0, 0.8, 440.0);
        let (_, _, quiet_lufs) = quiet.finish();
        let (_, _, loud_lufs) = loud.finish();
        let delta = loud_lufs - quiet_lufs;
        assert!((delta - 18.06).abs() < 0.5, "delta {delta}");
    }
}
