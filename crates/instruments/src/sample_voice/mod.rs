//! Preallocated varispeed sample-playback voice shared by Sampler and Slicer
//! (03-audio-runtime-spec.md §Sample player / §Slicer). Each voice owns two
//! [`VarispeedReader`]s (one per output channel, driven with identical rate
//! sequences so they stay in lockstep) and renders from an immutable prepared
//! sample with zero allocation.
//!
//! ## Startup alignment and rate safety
//!
//! The sinc reader is causal: its first output frame is centered
//! `taps/2 - 1` input frames into the pushed stream, and its internal rate
//! smoother always restarts at 1.0. `start` therefore runs the whole startup
//! at rate 1.0: feed [`STARTUP_ZEROS`] zero frames, discard the
//! [`STARTUP_DISCARD`] output frames they produce — the first kept output
//! frame is centered exactly on stream frame 0, so the plugin reports zero
//! latency and note-onsets are sample-accurate at any rate.
//!
//! The target rate is only applied after startup, via [`SampleVoice::set_rate`].
//! For `rate > 1` the reader's anti-alias kernel stretches to
//! `~64 * rate` taps, which requires the read position to stay ahead of
//! `32 * rate - 1` input frames. A rate increase that would violate this is
//! applied as a bounded ramp (≤ 1/32 per produced frame, derived from that
//! invariant); small increases and all decreases apply directly. The ramp is
//! a sub-10 ms pitch glide in the worst case (rate 16) and zero for
//! rates ≤ ~2.
//!
//! ## Stream coordinates
//!
//! A voice plays stream coordinates `[0, stream_len)`; sample frame =
//! `base + s` forward or `base + stream_len - 1 - s` reversed (reverse is a
//! playback-time reversed feed, no new asset). Optional forward loop points
//! are stream coordinates. When the stream ends without a loop the voice
//! keeps feeding zeros until the reader window has passed the last frame
//! (`drained`), so the tail is not truncated by the group delay.

use oxitone_dsp::envelope::Adsr;
use oxitone_dsp::resample::{VarispeedReader, SINC_TAPS};

/// Playback-rate clamps for pitch/override/tempo-factor products.
pub const MIN_RATE: f64 = 0.125;
pub const MAX_RATE: f64 = 16.0;
/// Rate-smoother time constant; settles within ~50 samples at 48 kHz.
const RATE_SMOOTHING_MS: f64 = 0.05;

/// Preroll zero frames fed at rate 1.0 (the rate-1 kernel is `SINC_TAPS` taps).
const STARTUP_ZEROS: u64 = SINC_TAPS as u64;
/// Output frames produced by the preroll and discarded: the reader starts
/// reading at `taps/2 - 1`, so this leaves the first kept frame centered on
/// stream frame 0.
const STARTUP_DISCARD: u64 = STARTUP_ZEROS - (STARTUP_ZEROS / 2 - 1);
/// Maximum per-produced-frame rate increase; keeps the stretched kernel
/// behind the read position (`32 * step - 1 <= read_pos` holds inductively
/// once `read_pos >= 32`, which startup guarantees).
const RATE_RAMP_PER_FRAME: f64 = 1.0 / 32.0;

/// Everything needed to (re)trigger a voice. Control-rate data only.
#[derive(Debug, Clone, Copy)]
pub struct VoiceStart {
    pub note: u8,
    pub rate: f64,
    /// Sample frame index of stream coordinate 0.
    pub base: u64,
    pub stream_len: u64,
    /// Forward loop `[start, end)` in stream coordinates.
    pub loop_region: Option<(u64, u64)>,
    pub reverse: bool,
    /// Velocity/slice-level product applied with the amp envelope.
    pub gain: f32,
    /// Per-voice pan (slicer slice override; 0 otherwise).
    pub pan: f32,
    pub adsr: (f64, f64, f32, f64),
}

/// Bounded rate increase in progress.
#[derive(Debug, Clone, Copy)]
struct RateRamp {
    from: f64,
    to: f64,
    total: u32,
    done: u32,
}

pub struct SampleVoice {
    readers: [VarispeedReader; 2],
    reverse_buf: Vec<f32>,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pub amp: Adsr,
    pub note: u8,
    pub active: bool,
    pub age: u64,
    /// Steal metric (amp level × gain), updated once per block by the host.
    pub level: f32,
    rate: f64,
    target_rate: f64,
    rate_initialized: bool,
    ramp: Option<RateRamp>,
    gain: f32,
    pan: f32,
    base: u64,
    preroll: u64,
    zeros_fed: u64,
    discard_remaining: u64,
    stream_pos: u64,
    stream_len: u64,
    loop_region: Option<(u64, u64)>,
    reverse: bool,
    drained: bool,
}

impl SampleVoice {
    /// Preallocate readers and scratch for `max_block` frames. Control thread.
    pub fn new(sample_rate: f64, max_block: usize) -> Self {
        Self {
            readers: [
                VarispeedReader::new(MAX_RATE, max_block, sample_rate, RATE_SMOOTHING_MS),
                VarispeedReader::new(MAX_RATE, max_block, sample_rate, RATE_SMOOTHING_MS),
            ],
            reverse_buf: vec![0.0; max_block],
            buf_l: vec![0.0; max_block],
            buf_r: vec![0.0; max_block],
            amp: Adsr::new(sample_rate),
            note: 0,
            active: false,
            age: 0,
            level: 0.0,
            rate: 1.0,
            target_rate: 1.0,
            rate_initialized: false,
            ramp: None,
            gain: 0.0,
            pan: 0.0,
            base: 0,
            preroll: 0,
            zeros_fed: 0,
            discard_remaining: 0,
            stream_pos: 0,
            stream_len: 0,
            loop_region: None,
            reverse: false,
            drained: true,
        }
    }

    /// (Re)trigger. Runs inside `process` on note-on: `VarispeedReader::reset`
    /// is a bounded fill of preallocated buffers (no allocation/locks), which
    /// is required for voice reuse on the audio thread.
    pub fn start(&mut self, start: VoiceStart) {
        for reader in &mut self.readers {
            reader.reset();
        }
        self.preroll = STARTUP_ZEROS;
        self.zeros_fed = 0;
        self.discard_remaining = STARTUP_DISCARD;
        self.rate = 1.0;
        self.rate_initialized = false;
        self.ramp = None;
        self.target_rate = start.rate.clamp(MIN_RATE, MAX_RATE);
        self.gain = start.gain;
        self.pan = start.pan;
        self.base = start.base;
        self.stream_pos = 0;
        self.stream_len = start.stream_len;
        self.loop_region = start
            .loop_region
            .filter(|&(s, e)| s < e && e <= start.stream_len);
        self.reverse = start.reverse;
        self.drained = start.stream_len == 0;
        let (a, d, s, r) = start.adsr;
        self.amp.set_params(a, d, s, r);
        self.amp.note_on();
        self.note = start.note;
        self.level = start.gain;
        self.active = true;
    }

    /// Control-rate rate update (tempo factor / repitch). Before startup
    /// completes this only records the target; afterwards it applies the
    /// rate directly or through a bounded ramp (see module docs).
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

    /// Advance an active ramp after `produced` output frames.
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

    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn is_drained(&self) -> bool {
        self.drained
    }

    /// Frames of stream content not yet traversed (for tail estimates).
    pub fn remaining_input_frames(&self) -> u64 {
        self.stream_len.saturating_sub(self.stream_pos)
    }
}

mod pool;
mod render;

pub use pool::SampleVoicePool;
