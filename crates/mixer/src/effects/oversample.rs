//! 2x polyphase oversampling for nonlinear processors
//! (03-audio-runtime-spec.md §数值精度: Clipper、带饱和级的 Limit、
//! waveshaper、含反馈的 Phaser 必须 2x/4x oversample). Taps are a 49-tap
//! Blackman-windowed sinc lowpass at 0.9× the base-rate Nyquist, designed on
//! the control thread; `upsample`/`downsample` are RT-safe and allocation
//! free. Round-trip group delay is exactly 24 base-rate frames per stage.

/// FIR length (odd, so the round-trip latency is an integer frame count).
const TAPS: usize = 49;
const HALF: usize = (TAPS - 1) / 2;

/// 49-tap Blackman-windowed sinc lowpass at 0.9× the base-rate Nyquist, DC
/// sum 1. Control thread (transcendental math); shared by the oversampling
/// stages and the true-peak meter's composite 4x phase bank.
pub(crate) fn lowpass_taps() -> [f64; TAPS] {
    let mut taps = [0.0f64; TAPS];
    let fc = 0.225; // 0.9 * base Nyquist, in cycles per 2x-rate sample
    let mut sum = 0.0;
    for (i, tap) in taps.iter_mut().enumerate() {
        let n = i as f64 - HALF as f64;
        let sinc = if n == 0.0 {
            2.0 * fc
        } else {
            (2.0 * core::f64::consts::PI * fc * n).sin() / (core::f64::consts::PI * n)
        };
        let window = 0.42
            - 0.5 * (2.0 * core::f64::consts::PI * i as f64 / (TAPS - 1) as f64).cos()
            + 0.08 * (4.0 * core::f64::consts::PI * i as f64 / (TAPS - 1) as f64).cos();
        *tap = sinc * window;
        sum += *tap;
    }
    for tap in taps.iter_mut() {
        *tap /= sum;
    }
    taps
}

/// One 2x oversampling stage for a single channel.
pub struct Oversampler2x {
    /// Lowpass taps, DC sum 1. Upsampling applies the ×2 stuffing gain.
    taps: [f64; TAPS],
    /// Even-phase upsampler taps (`taps[0], taps[2], ...`).
    even_taps: [f64; HALF + 1],
    /// Odd-phase upsampler taps (`taps[1], taps[3], ...`).
    odd_taps: [f64; HALF],
    /// Last HALF base-rate inputs (polyphase upsampler history).
    up_hist: [f32; HALF],
    /// Last TAPS-1 band samples (downsampler FIR history).
    down_hist: [f32; TAPS - 1],
    /// Concat scratch: `up_hist` followed by the current input block, so the
    /// polyphase inner loops read a contiguous slice per block.
    up_ext: Vec<f32>,
    /// Concat scratch: `down_hist` followed by the current band block.
    down_ext: Vec<f32>,
    /// 2x-rate working buffer (upsampled signal, filtered in place).
    band: Vec<f32>,
    /// Base-rate decimated output.
    out: Vec<f32>,
}

impl Oversampler2x {
    /// Control thread: designs the taps and allocates scratch for
    /// `max_block` base-rate frames.
    pub fn new(max_block: usize) -> Self {
        let taps = lowpass_taps();
        let mut even_taps = [0.0f64; HALF + 1];
        let mut odd_taps = [0.0f64; HALF];
        for (k, tap) in even_taps.iter_mut().enumerate() {
            *tap = taps[2 * k];
        }
        for (k, tap) in odd_taps.iter_mut().enumerate() {
            *tap = taps[2 * k + 1];
        }
        Self {
            taps,
            even_taps,
            odd_taps,
            up_hist: [0.0; HALF],
            down_hist: [0.0; TAPS - 1],
            up_ext: vec![0.0; HALF + max_block],
            down_ext: vec![0.0; TAPS - 1 + 2 * max_block],
            band: vec![0.0; 2 * max_block],
            out: vec![0.0; max_block],
        }
    }

    /// Round-trip latency in base-rate frames: (TAPS-1)/2 = 24.
    pub fn latency_frames(&self) -> u64 {
        (HALF / 2) as u64 * 2 // up (HALF/2 base frames) + down
    }

    pub fn reset(&mut self) {
        self.up_hist = [0.0; HALF];
        self.down_hist = [0.0; TAPS - 1];
    }

    /// Zero-stuff + lowpass into the internal 2x band; returns it for the
    /// nonlinear stage to mutate in place. RT-safe.
    pub fn upsample(&mut self, input: &[f32]) -> &mut [f32] {
        let frames = input.len();
        self.up_ext[..HALF].copy_from_slice(&self.up_hist);
        self.up_ext[HALF..HALF + frames].copy_from_slice(input);
        for n in 0..frames {
            // Polyphase: even/odd output phases use even/odd taps against
            // consecutive base-rate history (z[2n-2k] = x[n-k]).
            let base = HALF + n;
            let mut even = 0.0f64;
            for (k, &tap) in self.even_taps.iter().enumerate() {
                even += tap * self.up_ext[base - k] as f64;
            }
            let mut odd = 0.0f64;
            for (k, &tap) in self.odd_taps.iter().enumerate() {
                odd += tap * self.up_ext[base - k] as f64;
            }
            self.band[2 * n] = (2.0 * even) as f32;
            self.band[2 * n + 1] = (2.0 * odd) as f32;
        }
        self.up_hist
            .copy_from_slice(&self.up_ext[frames..frames + HALF]);
        &mut self.band[..2 * frames]
    }

    /// Full round trip: upsample, run `stage` on the 2x band, downsample.
    /// RT-safe.
    pub fn process_roundtrip(
        &mut self,
        input: &[f32],
        stage: &mut dyn FnMut(&mut [f32]),
    ) -> &[f32] {
        let frames = input.len();
        let band = self.upsample(input);
        stage(band);
        self.downsample(frames)
    }

    /// Lowpass the 2x band and decimate back to base rate. RT-safe.
    pub fn downsample(&mut self, frames: usize) -> &[f32] {
        self.down_ext[..TAPS - 1].copy_from_slice(&self.down_hist);
        self.down_ext[TAPS - 1..TAPS - 1 + 2 * frames].copy_from_slice(&self.band[..2 * frames]);
        for n in 0..frames {
            // The decimator keeps the even 2x-rate positions: the FIR is
            // evaluated with band[2n] as its newest sample.
            let base = TAPS - 1 + 2 * n;
            let mut acc = 0.0f64;
            for (i, tap) in self.taps.iter().enumerate() {
                acc += tap * self.down_ext[base - i] as f64;
            }
            self.out[n] = acc as f32;
        }
        self.down_hist
            .copy_from_slice(&self.down_ext[2 * frames..2 * frames + TAPS - 1]);
        &self.out[..frames]
    }
}

/// Cascaded 4x oversampling (two 2x stages) for one channel. The `shaper`
/// stage selects whether the nonlinearity runs at 4x or, for a cheaper 2x
/// mode, at the intermediate rate — the second stage then stays linear, so
/// latency is identical either way.
pub struct Oversampler4x {
    lo: Oversampler2x,
    hi: Oversampler2x,
}

impl Oversampler4x {
    /// Prepare-owned 4x band, for stereo-linked dynamics.
    pub fn upsample(&mut self, input: &[f32]) -> &mut [f32] {
        self.hi.upsample(self.lo.upsample(input))
    }

    pub fn downsample(&mut self, frames: usize) -> &[f32] {
        let back = self.hi.downsample(2 * frames);
        self.lo.band[..2 * frames].copy_from_slice(back);
        self.lo.downsample(frames)
    }

    pub fn new(max_block: usize) -> Self {
        Self {
            lo: Oversampler2x::new(max_block),
            hi: Oversampler2x::new(2 * max_block),
        }
    }

    /// Constant round-trip latency: 24 + 12 base-rate frames.
    pub fn latency_frames(&self) -> u64 {
        self.lo.latency_frames() + self.hi.latency_frames() / 2
    }

    pub fn reset(&mut self) {
        self.lo.reset();
        self.hi.reset();
    }

    /// Run `nonlinear` on the upsampled band and return base-rate output.
    /// `at_4x` selects the 4x band or the intermediate 2x band (2x mode).
    pub fn process(
        &mut self,
        input: &[f32],
        at_4x: bool,
        nonlinear: &mut dyn FnMut(&mut [f32]),
    ) -> &[f32] {
        let frames = input.len();
        let band2x = self.lo.upsample(input);
        if !at_4x {
            nonlinear(band2x);
        }
        // The second stage always runs (linear in 2x mode) so the reported
        // latency never depends on the oversample parameter.
        let band4x = self.hi.upsample(band2x);
        if at_4x {
            nonlinear(band4x);
        }
        let back2x = self.hi.downsample(2 * frames);
        self.lo.band[..2 * frames].copy_from_slice(back2x);
        self.lo.downsample(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_dc_and_latency() {
        let mut os = Oversampler2x::new(128);
        assert_eq!(os.latency_frames(), 24);
        let input = [0.5f32; 128];
        let band = os.upsample(&input);
        assert_eq!(band.len(), 256);
        let out = os.downsample(128);
        // After the group delay the constant signal is restored.
        assert!((out[64] - 0.5).abs() < 1e-3, "dc {}", out[64]);
    }

    #[test]
    fn sine_round_trip_is_transparent_below_cutoff() {
        let mut os = Oversampler2x::new(512);
        let sr = 48_000.0f64;
        let input: Vec<f32> = (0..512)
            .map(|i| (2.0 * core::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32)
            .collect();
        os.upsample(&input);
        let out = os.downsample(512).to_vec();
        let delay = os.latency_frames() as usize;
        let mut max_err = 0.0f32;
        for i in delay..512 {
            max_err = max_err.max((out[i] - input[i - delay]).abs());
        }
        assert!(max_err < 0.02, "max_err {max_err}");
    }

    #[test]
    fn four_x_latency_is_36_frames() {
        let os = Oversampler4x::new(128);
        assert_eq!(os.latency_frames(), 36);
    }
}
