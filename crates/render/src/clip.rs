//! Sample-clip runtime node: wraps a `SampleClipPlayer` with the clip
//! window gate, tempo-factor/ratio computation from the baked table, the
//! per-clip `tone` tilt filter, and smoothed level/gain/pan (03-audio-
//! runtime-spec.md §Sample player). Loop `until` gating and seek
//! fast-forward live here.

use oxitone_core::wire::TempoSync;
use oxitone_dsp::biquad::{design, Biquad, BiquadKind};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::compile::SampleClipPlan;
use oxitone_transport::tempo::CompiledTempoMap;

use crate::player::SampleClipPlayer;

/// Tilt center frequency and range (±6 dB shelves at `tone` ±1).
const TILT_HZ: f64 = 1000.0;
const TILT_Q: f64 = std::f64::consts::FRAC_1_SQRT_2;
const TILT_MAX_DB: f64 = 6.0;

pub struct ClipNode {
    pub player: SampleClipPlayer,
    pub channels: Vec<usize>,
    pub start_frame: u64,
    pub end_frame: u64,
    /// Loop end gate (clip `loop` count/lastBeat); `u64::MAX` when no loop.
    pub until_frame: u64,
    pub enabled: bool,
    pub tempo_sync: TempoSync,
    pub static_rate: f64,
    /// Physical automation rate (0.25..4), multiplied onto `static_rate`.
    pub automation_rate: f64,
    pub level: OnePoleSmoother,
    pub gain: OnePoleSmoother,
    pub pan: OnePoleSmoother,
    pub tone: f32,
    tilt: [[Biquad; 2]; 2],
    tilt_tone: f32,
    pub content_frames: u64,
    pub content_beats: f64,
    pub duration_beats: f64,
    sample_rate: u32,
    activated: bool,
}

fn tilt_coeffs(sample_rate: f64, tone: f32) -> [[Biquad; 2]; 2] {
    let gain = f64::from(tone) * TILT_MAX_DB;
    let make = |kind, g| Biquad::new(design(kind, sample_rate, TILT_HZ, TILT_Q, g));
    [
        [
            make(BiquadKind::LowShelf, -gain),
            make(BiquadKind::HighShelf, gain),
        ],
        [
            make(BiquadKind::LowShelf, -gain),
            make(BiquadKind::HighShelf, gain),
        ],
    ]
}

impl ClipNode {
    pub fn new(plan: &SampleClipPlan, sample_rate: u32, max_block: usize) -> Self {
        let sr = f64::from(sample_rate);
        let mut level = OnePoleSmoother::new(sr, 20.0);
        level.snap(1.0);
        let mut gain = OnePoleSmoother::new(sr, 20.0);
        gain.snap(plan.gain);
        let mut pan = OnePoleSmoother::new(sr, 20.0);
        pan.snap(plan.pan);
        Self {
            player: SampleClipPlayer::new(plan, sr, max_block),
            channels: plan.channels.clone(),
            start_frame: plan.start_frame,
            end_frame: plan.end_frame,
            until_frame: plan.loop_region.map(|l| l.until_frame).unwrap_or(u64::MAX),
            enabled: plan.enabled,
            tempo_sync: plan.tempo_sync,
            static_rate: plan.rate,
            automation_rate: 1.0,
            level,
            gain,
            pan,
            tone: 0.0,
            tilt: tilt_coeffs(sr, 0.0),
            tilt_tone: 0.0,
            content_frames: plan.sample.frames(),
            content_beats: plan.content_beats,
            duration_beats: plan.duration_beats.to_f64(),
            sample_rate,
            activated: false,
        }
    }

    /// Seek flush.
    pub fn reset(&mut self) {
        self.player.reset();
        self.activated = false;
        self.automation_rate = 1.0;
        for channel in &mut self.tilt {
            for stage in channel {
                stage.reset();
            }
        }
    }

    /// Effective varispeed rate for the window `[win_start, win_end)`
    /// (`off` / `repitch`). `repitch` derives the block-average rate from
    /// the baked beat↔frame map — content beats advance linearly with
    /// project beats (tempo 因子 × clip rate, 02-domain-spec.md §tempoSync)
    /// — because a block-start value would integrate as a left Riemann sum
    /// and drift under tempo ramps.
    fn control_rate(&self, win_start: u64, win_end: u64, tempo: &CompiledTempoMap) -> f64 {
        match self.tempo_sync {
            TempoSync::Repitch => {
                let frames = (win_end - win_start).max(1) as f64;
                let beats =
                    tempo.frame_to_beat(win_end).to_f64() - tempo.frame_to_beat(win_start).to_f64();
                self.static_rate
                    * self.automation_rate
                    * (self.content_frames.max(1) as f64 / self.content_beats.max(1e-9))
                    * beats
                    / frames
            }
            _ => self.static_rate * self.automation_rate,
        }
    }

    /// WSOLA ratio at `bpm` (03-audio-runtime-spec.md §tempoSync stretch).
    fn stretch_ratio(&self, bpm: f64) -> f64 {
        self.duration_beats * 60.0 * f64::from(self.sample_rate)
            / (self.content_frames.max(1) as f64 * bpm.max(1e-9))
    }

    /// Fast-forward `offset` clip-timeline frames without emitting audio
    /// (seek landing mid-clip). Rates/ratios follow the baked table per
    /// chunk; rate-automation history during the skip uses the current
    /// value (documented approximation).
    fn fast_forward(&mut self, offset: u64, tempo: &CompiledTempoMap, max_block: usize) {
        let mut done = 0_u64;
        while done < offset {
            let n = ((offset - done) as usize).min(max_block) as u64;
            let bpm = tempo.bpm_at_frame(self.start_frame + done);
            self.player.set_rate(self.control_rate(
                self.start_frame + done,
                self.start_frame + done + n,
                tempo,
            ));
            self.player.set_ratio(self.stretch_ratio(bpm));
            self.player.skip_chunk(n as usize);
            done += n;
        }
    }

    /// Render the intersection of `[cur, cur + frames)` with the clip window
    /// into `out_l`/`out_r` (full-block overwrite; silence outside the
    /// window). Returns whether the window intersected the block.
    pub fn render(
        &mut self,
        cur: u64,
        frames: usize,
        tempo: &CompiledTempoMap,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) -> bool {
        for slot in out_l.iter_mut() {
            *slot = 0.0;
        }
        for slot in out_r.iter_mut() {
            *slot = 0.0;
        }
        if !self.enabled || self.channels.is_empty() {
            return false;
        }
        let win_start = self.start_frame.max(cur);
        let win_end = self.end_frame.min(cur + frames as u64);
        if win_start >= win_end {
            return false;
        }
        if cur >= self.until_frame {
            self.player.disable_loop();
        }
        if !self.activated {
            self.activated = true;
            if win_start > self.start_frame {
                self.fast_forward(win_start - self.start_frame, tempo, frames);
            }
        }
        let bpm = tempo.bpm_at_frame(win_start);
        self.player
            .set_rate(self.control_rate(win_start, win_end, tempo));
        self.player.set_ratio(self.stretch_ratio(bpm));
        let n = (win_end - win_start) as usize;
        let offset = (win_start - cur) as usize;
        self.player.render(
            n,
            &mut out_l[offset..offset + n],
            &mut out_r[offset..offset + n],
        );
        true
    }

    /// Apply the per-clip tone tilt filter in place (coefficients update at
    /// block boundaries when `tone` changed). RT-safe.
    pub fn apply_tone(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        if (self.tone - self.tilt_tone).abs() > 1e-6 {
            self.tilt = tilt_coeffs(f64::from(self.sample_rate), self.tone);
            self.tilt_tone = self.tone;
        }
        if self.tilt_tone.abs() < 1e-6 {
            return;
        }
        self.tilt[0][0].process(out_l);
        self.tilt[0][1].process(out_l);
        self.tilt[1][0].process(out_r);
        self.tilt[1][1].process(out_r);
    }
}
