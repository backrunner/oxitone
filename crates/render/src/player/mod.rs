//! Sample clip player node (03-audio-runtime-spec.md §Sample player). One
//! player per `SampleClipPlan`; renders the clip's dry stereo signal for the
//! intersection of the block with the clip window. Mode chains:
//!
//! - `off` / `repitch`: [`VarispeedStereo`] — `off` uses only the clip's
//!   `rate`; `repitch` multiplies in the tempo factor from the baked table
//!   (same varispeed path, f64 position integration, bounded rate ramps).
//! - `stretch`: [`StretchStereo`] — WSOLA with the per-block ratio from the
//!   baked table so content beats map linearly onto clip beats.
//!
//! Gain/pan/level smoothing and the per-clip `tone` tilt filter live in the
//! renderer's clip node (`crate::clip`), not here.

mod feeder;
mod stretch;
mod varispeed;

use std::sync::Arc;

use oxitone_graph::compile::SampleClipPlan;
use oxitone_samples::PreparedSample;

pub use feeder::ClipFeeder;

use stretch::StretchStereo;
use varispeed::VarispeedStereo;

enum Chain {
    Direct(VarispeedStereo),
    Stretch(StretchStereo),
}

/// One sample-clip playback voice, preallocated at compile time.
pub struct SampleClipPlayer {
    feeder: ClipFeeder,
    chain: Chain,
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
    has_loop: bool,
    looping: bool,
}

impl SampleClipPlayer {
    pub fn new(plan: &SampleClipPlan, sample_rate: f64, max_block: usize) -> Self {
        let sample: Arc<PreparedSample> = plan.sample.clone();
        let feeder = ClipFeeder::new(
            sample,
            plan.loop_region.map(|l| (l.start_frame, l.end_frame)),
        );
        let chain = match plan.tempo_sync {
            oxitone_core::wire::TempoSync::Stretch => Chain::Stretch(StretchStereo::new(max_block)),
            _ => Chain::Direct(VarispeedStereo::new(sample_rate, max_block)),
        };
        Self {
            feeder,
            chain,
            scratch_l: vec![0.0; max_block],
            scratch_r: vec![0.0; max_block],
            has_loop: plan.loop_region.is_some(),
            looping: plan.loop_region.is_some(),
        }
    }

    /// Seek flush: content restarts at frame 0 of the clip; the renderer
    /// fast-forwards with [`SampleClipPlayer::skip_chunk`] when the seek
    /// target lands inside the clip.
    pub fn reset(&mut self) {
        self.feeder.reset();
        match &mut self.chain {
            Chain::Direct(chain) => chain.reset(),
            Chain::Stretch(chain) => chain.reset(),
        }
        self.looping = self.has_loop;
    }

    /// Stop looping (clip `loop` count/lastBeat reached).
    pub fn disable_loop(&mut self) {
        if self.looping {
            self.looping = false;
            self.feeder.disable_loop();
        }
    }

    /// Per-block rate update (`off`/`repitch` effective rate, post tempo
    /// factor and automation).
    pub fn set_rate(&mut self, rate: f64) {
        if let Chain::Direct(chain) = &mut self.chain {
            chain.set_rate(rate);
        }
    }

    /// Per-block stretch ratio update.
    pub fn set_ratio(&mut self, ratio: f64) {
        if let Chain::Stretch(chain) = &mut self.chain {
            chain.set_ratio(ratio);
        }
    }

    /// Fast-forward `frames` of clip-timeline output without emitting it
    /// (seek landing mid-clip). `set_rate`/`set_ratio` must be updated per
    /// chunk by the caller through [`SampleClipPlayer::skip_chunk`].
    pub fn skip_chunk(&mut self, frames: usize) {
        let frames = frames.min(self.scratch_l.len());
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        self.render_into(frames, &mut scratch_l[..frames], &mut scratch_r[..frames]);
        self.scratch_l = scratch_l;
        self.scratch_r = scratch_r;
    }

    /// Render exactly `frames` dry clip frames (overwrite). RT-safe.
    pub fn render(&mut self, frames: usize, out_l: &mut [f32], out_r: &mut [f32]) {
        self.render_into(frames, out_l, out_r);
    }

    fn render_into(&mut self, frames: usize, out_l: &mut [f32], out_r: &mut [f32]) {
        match &mut self.chain {
            Chain::Direct(chain) => chain.render(&mut self.feeder, frames, out_l, out_r),
            Chain::Stretch(chain) => chain.render(&mut self.feeder, frames, out_l, out_r),
        }
    }
}
