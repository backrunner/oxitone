//! Mixer metering (03-audio-runtime-spec.md §Meter): every MixerChannel
//! exposes peak/RMS from the fader output; the Master bus additionally
//! exposes a 4x oversampled true-peak estimate. The audio thread only
//! writes plain state; the display layer throttles reads and resets.

use oxitone_dsp::meter::Meter;

/// Stereo peak/RMS accumulator for one mixer channel.
pub struct BusMeter {
    left: Meter,
    right: Meter,
}

impl BusMeter {
    pub fn new() -> Self {
        Self {
            left: Meter::new(),
            right: Meter::new(),
        }
    }

    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }

    /// Fold one fader-output block into the accumulators. RT-safe.
    pub fn add_block(&mut self, left: &[f32], right: &[f32]) {
        self.left.add_block(left);
        self.right.add_block(right);
    }

    /// Peak across both channels.
    pub fn peak(&self) -> f32 {
        self.left.peak().max(self.right.peak())
    }

    /// RMS averaged across both channels.
    pub fn rms(&self) -> f32 {
        let frames = self.left.frames() + self.right.frames();
        if frames == 0 {
            return 0.0;
        }
        let sum_sq = self.left.rms() as f64 * self.left.rms() as f64 * self.left.frames() as f64
            + self.right.rms() as f64 * self.right.rms() as f64 * self.right.frames() as f64;
        (sum_sq / frames as f64).sqrt() as f32
    }
}

impl Default for BusMeter {
    fn default() -> Self {
        Self::new()
    }
}

/// Master-only true-peak estimate: 4x upsampled peak. The two cascaded 2x
/// polyphase stages (upsampling only, no nonlinear stage between them) form
/// a pure LTI filter at 4x rate, so they are composed offline into a single
/// 4-branch polyphase bank over base-rate input — one pass, no intermediate
/// band buffers, vectorizer-friendly branches. The composite response is
/// the identical filter (conv of the zero-stuffed first-stage taps with the
/// second-stage taps, ×4 total stuffing gain); values differ from the
/// cascaded evaluation only by float rounding (~1e-6 relative).
pub struct TruePeakMeter {
    phases: [[f32; TP_PHASE_TAPS]; 4],
    hist_l: [f32; TP_HIST],
    hist_r: [f32; TP_HIST],
    /// Concat scratch: channel history followed by the current block.
    ext: Vec<f32>,
    peak: f32,
}

/// Base-rate history the composite 145-tap 4x filter reaches back:
/// ceil((145 - 1) / 4) = 36 samples.
const TP_HIST: usize = 36;
/// Branch length (phase 0 uses 37 taps; shorter branches are zero-padded).
const TP_PHASE_TAPS: usize = TP_HIST + 1;

/// Compose the two upsampling stages into 4 polyphase branches. Control
/// thread (O(TAPS²) convolution).
fn true_peak_phases() -> [[f32; TP_PHASE_TAPS]; 4] {
    let stage = crate::effects::oversample::lowpass_taps();
    // Composite 4x-rate impulse response: first-stage taps zero-stuffed to
    // 4x (index 2i) convolved with the second-stage taps (97 + 49 - 1 = 145).
    let mut comp = [0.0f64; 145];
    for (i, &a) in stage.iter().enumerate() {
        for (j, &b) in stage.iter().enumerate() {
            comp[2 * i + j] += a * b;
        }
    }
    let mut phases = [[0.0f32; TP_PHASE_TAPS]; 4];
    for (p, phase) in phases.iter_mut().enumerate() {
        for (t, tap) in phase.iter_mut().enumerate() {
            let k = p + 4 * t;
            if k < comp.len() {
                *tap = (4.0 * comp[k]) as f32;
            }
        }
    }
    phases
}

/// Upsample one channel 4x and fold the result into `peak`. RT-safe.
fn fold_channel(
    phases: &[[f32; TP_PHASE_TAPS]; 4],
    ext: &mut [f32],
    hist: &mut [f32; TP_HIST],
    input: &[f32],
    peak: &mut f32,
) {
    let frames = input.len();
    ext[..TP_HIST].copy_from_slice(hist);
    ext[TP_HIST..TP_HIST + frames].copy_from_slice(input);
    let mut acc_peak = *peak;
    for n in 0..frames {
        let base = TP_HIST + n;
        // Fixed-size window (history + current sample) so the branch loops
        // fully unroll without bounds checks.
        let w: &[f32; TP_PHASE_TAPS] = ext[base + 1 - TP_PHASE_TAPS..=base]
            .try_into()
            .expect("window length is TP_PHASE_TAPS");
        for phase in phases {
            // Four independent partial sums over stride-4 taps keep the
            // accumulator chains short enough to overlap.
            let (mut s0, mut s1, mut s2, mut s3) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for t in 0..TP_PHASE_TAPS / 4 {
                let b = 4 * t;
                s0 += phase[b] * w[TP_PHASE_TAPS - 1 - b];
                s1 += phase[b + 1] * w[TP_PHASE_TAPS - 2 - b];
                s2 += phase[b + 2] * w[TP_PHASE_TAPS - 3 - b];
                s3 += phase[b + 3] * w[TP_PHASE_TAPS - 4 - b];
            }
            s0 += phase[TP_PHASE_TAPS - 1] * w[0];
            let y = ((s0 + s2) + (s1 + s3)).abs();
            acc_peak = acc_peak.max(y);
        }
    }
    *peak = acc_peak;
    hist.copy_from_slice(&ext[frames..frames + TP_HIST]);
}

impl TruePeakMeter {
    pub fn new(max_block: usize) -> Self {
        Self {
            phases: true_peak_phases(),
            hist_l: [0.0; TP_HIST],
            hist_r: [0.0; TP_HIST],
            ext: vec![0.0; TP_HIST + max_block],
            peak: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
    }

    /// Upsample one block 4x and track the inter-sample peak. RT-safe.
    pub fn add_block(&mut self, left: &[f32], right: &[f32]) {
        fold_channel(
            &self.phases,
            &mut self.ext,
            &mut self.hist_l,
            left,
            &mut self.peak,
        );
        fold_channel(
            &self.phases,
            &mut self.ext,
            &mut self.hist_r,
            right,
            &mut self.peak,
        );
    }

    pub fn true_peak(&self) -> f32 {
        self.peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_meter_tracks_peak_and_rms() {
        let mut meter = BusMeter::new();
        meter.add_block(&[0.5, -0.5], &[0.25, -0.25]);
        assert_eq!(meter.peak(), 0.5);
        let expect = (((0.25f64 + 0.0625) / 2.0).sqrt()) as f32;
        assert!((meter.rms() - expect).abs() < 1e-6);
        meter.reset();
        assert_eq!(meter.peak(), 0.0);
    }

    #[test]
    fn true_peak_exceeds_sample_peak_on_intersample_maxima() {
        let mut meter = TruePeakMeter::new(256);
        // fs/4 sine sampled at ±√2/2: the reconstructed inter-sample peak
        // reaches ~1.0, above the 0.707 sample peak.
        let buf: Vec<f32> = (0..256)
            .map(|i| {
                (core::f64::consts::FRAC_PI_2 * i as f64 + core::f64::consts::FRAC_PI_4).sin()
                    as f32
            })
            .collect();
        meter.add_block(&buf, &buf);
        assert!(meter.true_peak() > 0.9, "true peak {}", meter.true_peak());
        meter.reset();
        assert_eq!(meter.true_peak(), 0.0);
    }
}
