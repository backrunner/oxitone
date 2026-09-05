//! Content feeder over a prepared sample with an optional crossfaded loop
//! region (02-domain-spec.md §SampleClip: loop 边界 crossfade). A single
//! cursor feeds both output channels so they never drift apart. Past the
//! content end (without a loop) the feeder emits zeros, which also flushes
//! the resampler/stretcher windows.
//!
//! Loop crossfade strategy: on wrap, the first `xfade` frames of the loop
//! region are blended with the content immediately following the loop end
//! (linear gain), so no new asset is created and the wrap is click-free.
//! `xfade` is clamped to `min(256, region/4, frames past loop end)`.

use std::sync::Arc;

use oxitone_samples::PreparedSample;

/// Maximum loop-boundary crossfade in frames.
pub const MAX_LOOP_XFADE: usize = 256;

pub struct ClipFeeder {
    sample: Arc<PreparedSample>,
    pos: u64,
    len: u64,
    loop_region: Option<(u64, u64)>,
    xfade: usize,
    xfade_left: usize,
    looping: bool,
}

impl ClipFeeder {
    pub fn new(sample: Arc<PreparedSample>, loop_region: Option<(u64, u64)>) -> Self {
        let len = sample.frames();
        let (region, xfade) = match loop_region {
            Some((start, end)) if start < end && end <= len => {
                let region_len = end - start;
                let xfade = MAX_LOOP_XFADE
                    .min((region_len / 4) as usize)
                    .min((len - end) as usize);
                (Some((start, end)), xfade)
            }
            _ => (None, 0),
        };
        Self {
            sample,
            pos: 0,
            len,
            loop_region: region,
            xfade,
            xfade_left: 0,
            looping: region.is_some(),
        }
    }

    /// Back to the initial state (seek flush). Keeps all allocations.
    pub fn reset(&mut self) {
        self.pos = 0;
        self.xfade_left = 0;
        self.looping = self.loop_region.is_some();
    }

    /// Stop looping at the next region boundary (loop `count`/`lastBeat`
    /// reached); playback continues one-shot to the content end.
    pub fn disable_loop(&mut self) {
        self.looping = false;
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    #[inline]
    fn content(&self, chan: usize, frame: u64) -> f32 {
        self.sample
            .channels
            .get(chan)
            .and_then(|c| c.get(frame as usize))
            .copied()
            .unwrap_or(0.0)
    }

    #[inline]
    fn value(&self, chan: usize) -> f32 {
        let head = self.content(chan, self.pos);
        if self.xfade_left > 0 {
            let i = self.xfade - self.xfade_left;
            let tail = self
                .loop_region
                .map(|(_, end)| self.content(chan, end + i as u64))
                .unwrap_or(0.0);
            let g = i as f32 / self.xfade as f32;
            head * g + tail * (1.0 - g)
        } else {
            head
        }
    }

    /// Fill both outputs with `out_l.len()` content frames (mono content
    /// feeds both channels); zeros past the content end. RT-safe.
    pub fn fill(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let right_chan = if self.sample.channels.len() > 1 { 1 } else { 0 };
        for (l, r) in out_l.iter_mut().zip(out_r.iter_mut()) {
            let active = self.pos < self.len || self.looping;
            if active {
                *l = self.value(0);
                *r = self.value(right_chan);
            } else {
                *l = 0.0;
                *r = 0.0;
            }
            if self.xfade_left > 0 {
                self.xfade_left -= 1;
            }
            self.pos += 1;
            if let Some((start, end)) = self.loop_region {
                if self.looping && self.pos >= end {
                    self.pos = start;
                    self.xfade_left = self.xfade;
                }
            }
        }
    }
}
