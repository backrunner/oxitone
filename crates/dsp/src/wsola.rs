//! `wsola-v1`: pitch-preserving time-stretcher (WSOLA) for `tempoSync:
//! stretch`. All windows and buffers are allocated in `new`; the ratio is
//! applied at call (block) granularity; realtime and offline paths run the
//! same code with the same ratio sequence, so output is bit-identical
//! (02-domain-spec.md §tempoSync, 03 §offline parity).
//!
//! Valid ratios are 0.25..=4.0; out-of-range input returns
//! `SampleStretchRange` so the compiler/caller can reject the clip.
//!
//! Layout: analysis hop `Ha = W/4`, synthesis hop `Hs = Ha · ratio`
//! accumulated in `f64`. Each window searches `δ ∈ 0..=W/4` for the offset
//! maximizing the plain cross-correlation between the already-synthesized
//! overlap and the candidate input segment, then Hann-overlap-adds; output
//! is divided by the accumulated window sum, which keeps the amplitude flat
//! at every ratio (Hann is COLA at ratio 1, so identity holds there).
//!
//! `process` is RT-safe. Caller contract: at most `max_input_block` frames
//! per call; frames beyond the stretch latency are delivered on later calls
//! (feed silence to flush).

use oxitone_core::{codes, OxitoneError};

use crate::resample::Processed;

pub const MIN_RATIO: f64 = 0.25;
pub const MAX_RATIO: f64 = 4.0;
pub const DEFAULT_WINDOW: usize = 1024;

/// Mono streaming WSOLA stretcher. Instantiate one per channel.
pub struct Wsola {
    window: usize,
    hop_a: usize,
    search: usize,
    ratio: f64,
    win: Vec<f32>,
    fifo: Vec<f32>,
    fifo_base: u64,
    ring: Vec<f32>,
    norm: Vec<f32>,
    mask: u64,
    next_in: f64,
    next_out: f64,
    written_up_to: u64,
    finalized_up_to: u64,
    next_final: u64,
}

impl Wsola {
    /// Allocate all state for the given analysis window. Control thread.
    pub fn new(window: usize, max_input_block: usize) -> Self {
        let window = window.max(64);
        let hop_a = window / 4;
        let search = window / 4;
        let win: Vec<f32> = (0..window)
            .map(|i| {
                (0.5 - 0.5 * (2.0 * core::f64::consts::PI * i as f64 / window as f64).cos()) as f32
            })
            .collect();
        let ring_len = (4 * window).next_power_of_two();
        Self {
            window,
            hop_a,
            search,
            ratio: 1.0,
            win,
            fifo: Vec::with_capacity(max_input_block + window + search + hop_a + 16),
            fifo_base: 0,
            ring: vec![0.0; ring_len],
            norm: vec![0.0; ring_len],
            mask: ring_len as u64 - 1,
            next_in: 0.0,
            next_out: 0.0,
            written_up_to: 0,
            finalized_up_to: 0,
            next_final: 0,
        }
    }

    pub fn window_size(&self) -> usize {
        self.window
    }

    /// Restore initial streaming state without allocating or releasing buffers.
    pub fn reset(&mut self) {
        self.ratio = 1.0;
        self.fifo.clear();
        self.fifo_base = 0;
        self.ring.fill(0.0);
        self.norm.fill(0.0);
        self.next_in = 0.0;
        self.next_out = 0.0;
        self.written_up_to = 0;
        self.finalized_up_to = 0;
        self.next_final = 0;
    }

    /// Frames of input consumed internally but not yet reflected in output.
    pub fn latency_frames(&self) -> u64 {
        (self.window + self.search) as u64
    }

    /// Push `input`, produce up to `output.len()` stretched frames at
    /// `ratio` (output/input duration). Errors with `SampleStretchRange`
    /// outside 0.25..=4.0. RT-safe on the Ok path.
    ///
    /// Caller contract: output space per call must be at least
    /// `input.len() · ratio + 2 · window`, otherwise the fixed-size input
    /// fifo backs up (debug-asserted). Call with empty input to drain.
    pub fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ratio: f64,
    ) -> Result<Processed, OxitoneError> {
        if !ratio.is_finite() || !(MIN_RATIO..=MAX_RATIO).contains(&ratio) {
            return Err(OxitoneError::new(
                codes::SAMPLE_STRETCH_RANGE,
                format!("stretch ratio {ratio} outside {MIN_RATIO}..={MAX_RATIO}"),
            ));
        }
        self.ratio = ratio;
        self.discard_consumed();
        debug_assert!(self.fifo.len() + input.len() <= self.fifo.capacity());
        self.fifo.extend_from_slice(input);

        let mut produced = 0;
        loop {
            while produced < output.len() && self.next_final < self.finalized_up_to {
                let idx = (self.next_final & self.mask) as usize;
                let n = self.norm[idx];
                output[produced] = if n > 1e-6 { self.ring[idx] / n } else { 0.0 };
                self.next_final += 1;
                produced += 1;
            }
            if produced == output.len() {
                break;
            }
            let a = self.next_in.round() as u64;
            if a + (self.search + self.window) as u64 > self.fifo_base + self.fifo.len() as u64 {
                break;
            }
            self.place_window();
        }
        Ok(Processed {
            consumed: input.len(),
            produced,
        })
    }

    fn discard_consumed(&mut self) {
        let a = self.next_in.round() as u64;
        if a > self.fifo_base {
            let drop = ((a - self.fifo_base) as usize).min(self.fifo.len());
            if drop >= self.hop_a {
                self.fifo.drain(..drop);
                self.fifo_base += drop as u64;
            }
        }
    }

    #[inline]
    fn input_at(&self, abs: u64) -> f32 {
        self.fifo[(abs - self.fifo_base) as usize]
    }

    fn place_window(&mut self) {
        let w = self.window;
        let s = self.next_out.round() as u64;
        let a = self.next_in.round() as u64;
        let s_next = self.next_out + self.hop_a as f64 * self.ratio;
        let overlap = w.saturating_sub((s_next.round() as u64 - s) as usize);

        let mut best = 0usize;
        if overlap >= 16 {
            let mut best_corr = f64::NEG_INFINITY;
            for delta in 0..=self.search {
                let mut corr = 0.0f64;
                for i in 0..overlap {
                    let out = self.ring[((s + i as u64) & self.mask) as usize] as f64;
                    corr += out * self.input_at(a + delta as u64 + i as u64) as f64;
                }
                if corr > best_corr {
                    best_corr = corr;
                    best = delta;
                }
            }
        }

        let end = s + w as u64;
        let mut n = self.written_up_to;
        while n < end {
            let idx = (n & self.mask) as usize;
            self.ring[idx] = 0.0;
            self.norm[idx] = 0.0;
            n += 1;
        }
        for (i, &wv) in self.win.iter().enumerate() {
            let idx = ((s + i as u64) & self.mask) as usize;
            self.ring[idx] += wv * self.input_at(a + best as u64 + i as u64);
            self.norm[idx] += wv;
        }
        self.written_up_to = end;
        self.next_in += self.hop_a as f64;
        self.next_out = s_next;
        self.finalized_up_to = s_next.round() as u64;
    }
}
