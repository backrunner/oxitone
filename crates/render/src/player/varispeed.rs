//! Stereo varispeed playback chain for `tempoSync: off` and `repitch`,
//! built on `oxitone_dsp::resample::VarispeedReader`. Replicates the
//! startup/rate-safety strategy of `oxitone-instruments`' sample voices
//! (that module is private to the instruments crate; the strategy is
//! restated here so clip onsets are sample-accurate at any rate):
//!
//! - **Startup alignment**: the sinc reader is causal; its first output
//!   frame is centered `taps/2 - 1` input frames into the pushed stream and
//!   its rate smoother restarts at 1.0. Startup therefore runs at rate 1.0:
//!   feed enough zeros for the starting-rate kernel, discard the preroll,
//!   then snap to the starting rate. The first kept output frame is centered
//!   exactly on content frame 0, without a startup rate ramp delaying later notes.
//! - **Bounded rate ramps**: for `rate > 1` the anti-alias kernel stretches
//!   to `~64 * rate` taps and the read position must stay ahead of
//!   `32 * rate - 1` input frames. Increases that would violate this are
//!   applied as a bounded ramp (`RATE_RAMP_PER_FRAME` per produced frame).

use oxitone_dsp::resample::{VarispeedReader, SINC_TAPS};

use super::feeder::ClipFeeder;

/// Playback-rate clamps for the varispeed path (mirrors sample voices).
pub const MIN_RATE: f64 = 0.125;
pub const MAX_RATE: f64 = 16.0;
/// Rate-smoother time constant; settles within ~50 samples at 48 kHz.
const RATE_SMOOTHING_MS: f64 = 0.05;

/// Preroll zero frames fed at rate 1.0 (the rate-1 kernel is `SINC_TAPS` taps).
const STARTUP_ZEROS: u64 = SINC_TAPS as u64;
/// Output frames produced by the preroll and discarded, leaving the first
/// kept frame centered on content frame 0.
const STARTUP_DISCARD: u64 = STARTUP_ZEROS - (STARTUP_ZEROS / 2 - 1);
/// Maximum per-produced-frame rate increase.
const RATE_RAMP_PER_FRAME: f64 = 1.0 / 32.0;

#[derive(Debug, Clone, Copy)]
struct RateRamp {
    from: f64,
    to: f64,
    total: u32,
    done: u32,
}

pub struct VarispeedStereo {
    readers: [VarispeedReader; 2],
    zeros: Vec<f32>,
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    discard_l: Vec<f32>,
    discard_r: Vec<f32>,
    discard_remaining: u64,
    preroll_started: bool,
    /// Unconsumed input frames carried in `in_l`/`in_r` between calls.
    pending: usize,
    rate: f64,
    target_rate: f64,
    rate_initialized: bool,
    ramp: Option<RateRamp>,
}

impl VarispeedStereo {
    pub fn new(sample_rate: f64, max_block: usize) -> Self {
        Self {
            readers: [
                VarispeedReader::new(MAX_RATE, max_block, sample_rate, RATE_SMOOTHING_MS),
                VarispeedReader::new(MAX_RATE, max_block, sample_rate, RATE_SMOOTHING_MS),
            ],
            zeros: vec![0.0; max_block],
            in_l: vec![0.0; max_block],
            in_r: vec![0.0; max_block],
            discard_l: vec![0.0; max_block],
            discard_r: vec![0.0; max_block],
            discard_remaining: STARTUP_DISCARD,
            preroll_started: false,
            pending: 0,
            rate: 1.0,
            target_rate: 1.0,
            rate_initialized: false,
            ramp: None,
        }
    }

    /// Seek flush: restart the startup sequence. `VarispeedReader::reset`
    /// only rewrites preallocated buffers (no allocation).
    pub fn reset(&mut self) {
        for reader in &mut self.readers {
            reader.reset();
        }
        self.discard_remaining = STARTUP_DISCARD;
        self.preroll_started = false;
        self.pending = 0;
        self.rate = 1.0;
        self.rate_initialized = false;
        self.ramp = None;
    }

    /// Control-rate (per block) rate update. Before startup completes this
    /// only records the target; afterwards it applies directly or through a
    /// bounded ramp (see module docs).
    pub fn set_rate(&mut self, rate: f64) {
        self.target_rate = rate.clamp(MIN_RATE, MAX_RATE);
        if !self.rate_initialized {
            return;
        }
        let read_pos = self.readers[0].position();
        if self.target_rate > self.rate && 32.0 * self.target_rate - 1.0 > read_pos + 1.0 {
            let total = ((self.target_rate - self.rate) / RATE_RAMP_PER_FRAME)
                .ceil()
                .max(1.0) as u32;
            self.ramp = Some(RateRamp {
                from: self.rate,
                to: self.target_rate,
                total,
                done: 0,
            });
        } else {
            self.ramp = None;
            self.rate = self.target_rate;
            for reader in &mut self.readers {
                reader.set_rate(self.rate);
            }
        }
    }

    fn advance_ramp(&mut self, produced: u32) {
        let Some(ramp) = &mut self.ramp else {
            return;
        };
        let (from, to, total) = (ramp.from, ramp.to, ramp.total);
        ramp.done = (ramp.done + produced).min(total);
        let done = ramp.done;
        let t = f64::from(done) / f64::from(total);
        let rate = from + (to - from) * t;
        for reader in &mut self.readers {
            reader.set_rate(rate);
        }
        self.rate = rate;
        if done >= total {
            self.ramp = None;
            self.rate = to;
        }
    }

    /// Render exactly `frames` output frames into `out_l`/`out_r`
    /// (overwrite, not accumulate). RT-safe.
    pub fn render(
        &mut self,
        feeder: &mut ClipFeeder,
        frames: usize,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        if !self.preroll_started {
            let zeros = (SINC_TAPS as f64 * self.target_rate.max(1.0)).ceil() as u64;
            self.discard_remaining = zeros - (SINC_TAPS as u64 / 2 - 1);
            self.preroll_started = true;
        }
        while self.discard_remaining > 0 {
            let n = (self.discard_remaining as usize).min(self.zeros.len());
            let rl = self.readers[0].process(&self.zeros[..n], &mut self.discard_l[..n]);
            let rr = self.readers[1].process(&self.zeros[..n], &mut self.discard_r[..n]);
            debug_assert_eq!(rl, rr);
            if rl.produced == 0 {
                break;
            }
            self.discard_remaining -= rl.produced as u64;
        }
        if !self.rate_initialized {
            self.rate_initialized = true;
            self.rate = self.target_rate;
            for reader in &mut self.readers {
                reader.snap_rate(self.target_rate);
            }
        }
        let mut produced = 0;
        while produced < frames {
            if self.pending == 0 {
                feeder.fill(&mut self.in_l, &mut self.in_r);
                self.pending = self.in_l.len();
            }
            let pending = self.pending;
            let rl = self.readers[0].process(&self.in_l[..pending], &mut out_l[produced..frames]);
            let rr = self.readers[1].process(&self.in_r[..pending], &mut out_r[produced..frames]);
            debug_assert_eq!(rl, rr);
            let remaining = pending - rl.consumed;
            if rl.consumed > 0 && remaining > 0 {
                self.in_l.copy_within(rl.consumed..pending, 0);
                self.in_r.copy_within(rl.consumed..pending, 0);
            }
            self.pending = remaining;
            produced += rl.produced;
            self.advance_ramp(rl.produced as u32);
            if rl.produced == 0 && rl.consumed == 0 {
                break;
            }
        }
        for slot in &mut out_l[produced..frames] {
            *slot = 0.0;
        }
        for slot in &mut out_r[produced..frames] {
            *slot = 0.0;
        }
    }
}
