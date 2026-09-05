//! Windowed-sinc polyphase resampling, shared by sample decode, device-rate
//! adaptation, and varispeed (`rate`/`repitch`) playback. Quality target is
//! ≥ 100 dB SNR (03-audio-runtime-spec.md §数值精度); the playback position
//! is an `f64` advanced once per output sample.
//!
//! The continuous kernel `φ(t) = sinc(2·fc·t)·kaiser(t/(T/2))` is tabulated
//! at `SINC_PHASES` sub-sample phases (linear interpolation between table
//! entries). Downsampling by `d > 1` stretches the kernel (`φ(t/d)`, `T·d`
//! taps) so the cutoff tracks `fc/d`; tap weights are renormalized per
//! output sample for exact unity DC gain.
//!
//! All buffers are allocated in `new`; `process`/`push_frame`/`produce` are
//! RT-safe. Caller contract: at most `max_input_block` frames are pushed per
//! `process` call, and `step` never exceeds `max_step`.

use crate::gain_pan::OnePoleSmoother;

/// Base kernel taps (half-support 32 input samples).
pub const SINC_TAPS: usize = 64;
/// Kernel table resolution: entries per input-sample interval.
pub const SINC_PHASES: usize = 512;
/// Kernel cutoff, cycles per input sample (0.45 = 90% of Nyquist).
pub const CUTOFF: f64 = 0.45;
const KAISER_BETA: f64 = 10.0;
const TABLE_LEN: usize = SINC_TAPS * SINC_PHASES + 1;

/// Frames consumed/produced by a streaming call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Processed {
    pub consumed: usize,
    pub produced: usize,
}

fn bessel_i0(x: f64) -> f64 {
    let y = x * x / 4.0;
    let (mut sum, mut term) = (1.0f64, 1.0f64);
    for k in 1..64u32 {
        term *= y / (k * k) as f64;
        sum += term;
        if term < 1e-16 * sum {
            break;
        }
    }
    sum
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let px = core::f64::consts::PI * x;
        px.sin() / px
    }
}

fn kaiser(u: f64) -> f64 {
    if u.abs() >= 1.0 {
        0.0
    } else {
        bessel_i0(KAISER_BETA * (1.0 - u * u).sqrt()) / bessel_i0(KAISER_BETA)
    }
}

fn build_table() -> Vec<f32> {
    let half = (SINC_TAPS / 2) as f64;
    (0..TABLE_LEN)
        .map(|i| {
            let t = i as f64 / SINC_PHASES as f64 - half;
            (sinc(2.0 * CUTOFF * t) * kaiser(t / half)) as f32
        })
        .collect()
}

fn even_ceil(x: f64) -> usize {
    let t = x.ceil() as usize;
    (t + 1) & !1
}

#[inline]
fn kernel(table: &[f32], t: f64) -> f64 {
    let idx =
        ((t + (SINC_TAPS / 2) as f64) * SINC_PHASES as f64).clamp(0.0, (TABLE_LEN - 2) as f64);
    let i = idx as usize;
    let frac = idx - i as f64;
    let a = table[i] as f64;
    a + (table[i + 1] as f64 - a) * frac
}

/// Streaming windowed-sinc resampler over a mono `f32` stream.
pub struct SincResampler {
    table: Vec<f32>,
    weights: Vec<f32>,
    ring: Vec<f32>,
    mask: u64,
    written: u64,
    read_pos: f64,
    max_taps: usize,
    started: bool,
}

impl SincResampler {
    /// Allocate tables/ring for `step <= max_step` and pushes of at most
    /// `max_input_block` frames. Control thread only.
    pub fn new(max_step: f64, max_input_block: usize) -> Self {
        let max_taps = even_ceil(SINC_TAPS as f64 * max_step.max(1.0));
        let capacity = (max_taps + 2 * max_input_block + 8).next_power_of_two();
        Self {
            table: build_table(),
            weights: vec![0.0; max_taps],
            ring: vec![0.0; capacity],
            mask: capacity as u64 - 1,
            written: 0,
            read_pos: 0.0,
            max_taps,
            started: false,
        }
    }

    /// Back to the initial state; keeps allocations. Control thread only.
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.written = 0;
        self.read_pos = 0.0;
        self.started = false;
    }

    /// Effective tap count for a step; `d = max(step, 1)` stretches the
    /// kernel so the anti-alias cutoff scales with the downsample factor.
    fn taps_for(&self, step: f64) -> usize {
        even_ceil(SINC_TAPS as f64 * step.max(1.0)).min(self.max_taps)
    }

    /// Group delay in input frames for a given step. The first produced
    /// frame corresponds to input position `latency(step)`.
    pub fn latency_input_frames(&self, step: f64) -> f64 {
        (self.taps_for(step) / 2 - 1) as f64
    }

    /// Absolute input position of the next output frame.
    pub fn input_position(&self) -> f64 {
        self.read_pos
    }

    /// Push one input frame. RT-safe.
    #[inline]
    fn push_frame(&mut self, x: f32) {
        self.ring[(self.written & self.mask) as usize] = x;
        self.written += 1;
    }

    /// Whether a full kernel window of input is available. RT-safe.
    #[inline]
    fn can_produce(&self, step: f64) -> bool {
        if !self.started {
            return self.written >= self.taps_for(step) as u64;
        }
        let m = self.read_pos.floor() as u64;
        m + (self.taps_for(step) / 2) as u64 <= self.written.saturating_sub(1)
    }

    /// Produce one output frame; `can_produce(step)` must hold. RT-safe.
    fn produce(&mut self, step: f64) -> f32 {
        let d = step.max(1.0);
        let taps = self.taps_for(step);
        if !self.started {
            self.read_pos = self.latency_input_frames(step);
            self.started = true;
        }
        let m = self.read_pos.floor() as u64;
        let frac = self.read_pos - m as f64;
        let j0 = m + 1 - (taps / 2) as u64;
        let mut sum = 0.0f64;
        let table = &self.table;
        for (k, w) in self.weights[..taps].iter_mut().enumerate() {
            let t = (k as f64 + 1.0 - taps as f64 * 0.5 - frac) / d;
            let v = kernel(table, t);
            *w = v as f32;
            sum += v;
        }
        let mut acc = 0.0f64;
        for (k, &w) in self.weights[..taps].iter().enumerate() {
            acc += w as f64 * self.ring[((j0 + k as u64) & self.mask) as usize] as f64;
        }
        self.read_pos += step;
        (acc / sum) as f32
    }

    /// Push `input`, then produce while input window and output space last.
    /// `step` is input frames per output frame (`in_rate / out_rate`).
    /// RT-safe.
    pub fn process(&mut self, input: &[f32], output: &mut [f32], step: f64) -> Processed {
        debug_assert!(step > 0.0 && self.taps_for(step) <= self.max_taps);
        let mut consumed = 0;
        let mut produced = 0;
        loop {
            while produced < output.len() && self.can_produce(step) {
                output[produced] = self.produce(step);
                produced += 1;
            }
            if produced == output.len() || consumed == input.len() {
                break;
            }
            self.push_frame(input[consumed]);
            consumed += 1;
        }
        Processed { consumed, produced }
    }
}

/// Varispeed playback path for `rate`/`repitch`: identical kernel to
/// `SincResampler`, with the rate updated at block boundaries and smoothed
/// per sample; the playback position is the resampler's `f64` read position
/// integrated once per output sample (03-audio-runtime-spec.md §Sample
/// player). RT-safe after `new`.
pub struct VarispeedReader {
    core: SincResampler,
    rate: OnePoleSmoother,
    max_rate: f64,
}

impl VarispeedReader {
    /// `max_rate` bounds the rate parameter (spec range 0.25..4).
    pub fn new(max_rate: f64, max_input_block: usize, sample_rate: f64, smoothing_ms: f64) -> Self {
        let mut rate = OnePoleSmoother::new(sample_rate, smoothing_ms);
        rate.snap(1.0);
        Self {
            core: SincResampler::new(max_rate.max(1.0), max_input_block),
            rate,
            max_rate: max_rate.max(1.0),
        }
    }

    /// Block-boundary (control-rate) rate update.
    pub fn set_rate(&mut self, rate: f64) {
        self.rate.set_target(rate.clamp(1e-3, self.max_rate) as f32);
    }

    /// Initialize a pre-rolled stream at its known starting rate without a ramp.
    /// Caller must provide enough history for the target kernel before producing.
    pub fn snap_rate(&mut self, rate: f64) {
        self.rate.snap(rate.clamp(1e-3, self.max_rate) as f32);
    }

    pub fn reset(&mut self) {
        self.core.reset();
        self.rate.snap(1.0);
    }

    /// Playback position in input frames (f64, per-sample integrated).
    pub fn position(&self) -> f64 {
        self.core.input_position()
    }

    /// Push `input` and produce up to `output.len()` frames at the smoothed
    /// rate. RT-safe.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Processed {
        let mut consumed = 0;
        let mut produced = 0;
        loop {
            while produced < output.len() {
                let step = self.rate.value() as f64;
                if !self.core.can_produce(step) {
                    break;
                }
                output[produced] = self.core.produce(step);
                self.rate.next_sample();
                produced += 1;
            }
            if produced == output.len() || consumed == input.len() {
                break;
            }
            self.core.push_frame(input[consumed]);
            consumed += 1;
        }
        Processed { consumed, produced }
    }
}
