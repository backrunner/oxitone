//! Stereo WSOLA chain for `tempoSync: stretch` (02-domain-spec.md
//! §tempoSync, 03-audio-runtime-spec.md §tempoSync 的运行时规则). The ratio
//! is recomputed per block from the baked beat↔frame table by the caller and
//! applied at block granularity; realtime and offline run the same algorithm
//! with the same ratio sequence (offline parity).
//!
//! Produced frames queue in a preallocated ring so the caller can drain
//! exactly `frames` per block while the WSOLA works at its own hop schedule.
//! The ratio is intentionally *not* smoothed beyond the block grid: the
//! baked tempo table is already piecewise-linear on a 1/64-beat grid, and
//! additional smoothing would break the exact content-beat ↔ clip-beat
//! integral.

use oxitone_dsp::wsola::{Wsola, DEFAULT_WINDOW};

use super::feeder::ClipFeeder;

pub struct StretchStereo {
    wsola: [Wsola; 2],
    window: usize,
    max_block: usize,
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    fifo_l: Vec<f32>,
    fifo_r: Vec<f32>,
    fifo_head: usize,
    fifo_len: usize,
    ratio: f64,
}

impl StretchStereo {
    pub fn new(max_block: usize) -> Self {
        Self::with_window(max_block, DEFAULT_WINDOW)
    }

    fn with_window(max_block: usize, window: usize) -> Self {
        // Per `Wsola::process` caller contract the output space must hold
        // `chunk * max_ratio + 2 * window`; the fifo holds one such chunk
        // plus two blocks of backlog.
        let chunk_out = max_block * 4 + 2 * window + 16;
        let fifo = chunk_out + 2 * max_block;
        Self {
            wsola: [Wsola::new(window, max_block), Wsola::new(window, max_block)],
            window,
            max_block,
            in_l: vec![0.0; max_block],
            in_r: vec![0.0; max_block],
            out_l: vec![0.0; chunk_out],
            out_r: vec![0.0; chunk_out],
            fifo_l: vec![0.0; fifo],
            fifo_r: vec![0.0; fifo],
            fifo_head: 0,
            fifo_len: 0,
            ratio: 1.0,
        }
    }

    /// Seek flush. `Wsola` has no in-place reset; re-creating it reuses the
    /// same fixed sizes (control path only — never inside a render block).
    pub fn reset(&mut self) {
        *self = Self::with_window(self.max_block, self.window);
    }

    /// Per-block ratio from the baked tempo table (0.25..=4; validated at
    /// compile time).
    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio;
    }

    fn fifo_push(&mut self, count: usize) {
        debug_assert!(self.fifo_len + count <= self.fifo_l.len());
        for i in 0..count {
            let at = (self.fifo_head + self.fifo_len) % self.fifo_l.len();
            self.fifo_l[at] = self.out_l[i];
            self.fifo_r[at] = self.out_r[i];
            self.fifo_len += 1;
        }
    }

    /// Render exactly `frames` output frames (overwrite). RT-safe.
    pub fn render(
        &mut self,
        feeder: &mut ClipFeeder,
        frames: usize,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        while self.fifo_len < frames {
            feeder.fill(&mut self.in_l, &mut self.in_r);
            let rl = self
                .wsola
                .get_mut(0)
                .map(|w| w.process(&self.in_l, &mut self.out_l, self.ratio));
            let rr = self
                .wsola
                .get_mut(1)
                .map(|w| w.process(&self.in_r, &mut self.out_r, self.ratio));
            let (pl, pr) = match (rl, rr) {
                (Some(Ok(a)), Some(Ok(b))) => (a.produced, b.produced),
                _ => (0, 0),
            };
            debug_assert_eq!(pl, pr);
            if pl == 0 {
                // Cannot progress without more input; the feeder never
                // blocks, so a zero here means the window cannot close yet
                // (startup). Feed another chunk.
                if self.in_l.is_empty() {
                    break;
                }
            }
            self.fifo_push(pl);
        }
        let n = frames.min(self.fifo_len);
        for (i, slot) in out_l.iter_mut().enumerate().take(frames) {
            if i < n {
                let at = (self.fifo_head + i) % self.fifo_l.len();
                *slot = self.fifo_l[at];
            } else {
                *slot = 0.0;
            }
        }
        for (i, slot) in out_r.iter_mut().enumerate().take(frames) {
            if i < n {
                let at = (self.fifo_head + i) % self.fifo_r.len();
                *slot = self.fifo_r[at];
            } else {
                *slot = 0.0;
            }
        }
        self.fifo_head = (self.fifo_head + n) % self.fifo_l.len();
        self.fifo_len -= n;
    }
}
