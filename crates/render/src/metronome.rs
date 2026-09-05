//! Engine-synthesized metronome click (02-domain-spec.md §Playback 与
//! Export 语义: Rust 合成 click，accent 来自 time signature map，limiter
//! 前混入). No audio assets: clicks are short decaying sine bursts rendered
//! into the master path. The schedule derives from the baked tempo map and
//! the time-signature map at control time; per-block rendering is
//! allocation-free.

use oxitone_core::Beat;
use oxitone_graph::compile::RenderPlan;

/// Click frequencies (accent / normal) and envelope.
const ACCENT_HZ: f64 = 2093.0;
const NORMAL_HZ: f64 = 1567.98;
const CLICK_SECONDS: f64 = 0.025;
const DECAY_SECONDS: f64 = 0.006;
const ACCENT_GAIN: f32 = 1.0;
const NORMAL_GAIN: f32 = 0.6;
/// Maximum simultaneous clicks (overlapping tails at high BPM).
const MAX_ACTIVE: usize = 16;

struct ActiveClick {
    start_frame: u64,
    accent: bool,
}

pub struct Metronome {
    sample_rate: f64,
    level: f32,
    accent_click: Vec<f32>,
    normal_click: Vec<f32>,
    /// Next integer beat to schedule.
    next_beat: u64,
    active: Vec<ActiveClick>,
}

impl Metronome {
    /// `level` is the overall click gain (engine option; default 0.5).
    pub fn new(plan: &RenderPlan, level: f32) -> Self {
        let sample_rate = f64::from(plan.sample_rate);
        let synth = |hz: f64, gain: f32| -> Vec<f32> {
            let len = (CLICK_SECONDS * sample_rate).round() as usize;
            (0..len)
                .map(|i| {
                    let t = i as f64 / sample_rate;
                    (2.0 * std::f64::consts::PI * hz * t).sin() as f32
                        * (-t / DECAY_SECONDS).exp() as f32
                        * gain
                })
                .collect()
        };
        Self {
            sample_rate,
            level,
            accent_click: synth(ACCENT_HZ, ACCENT_GAIN),
            normal_click: synth(NORMAL_HZ, NORMAL_GAIN),
            next_beat: 0,
            active: Vec::with_capacity(MAX_ACTIVE),
        }
    }

    /// Seek: drop active clicks and resume scheduling at `frame`.
    pub fn seek(&mut self, plan: &RenderPlan, frame: u64) {
        self.active.clear();
        let beat = plan.tempo.frame_to_beat(frame).to_f64();
        self.next_beat = beat.ceil().max(0.0) as u64;
    }

    /// Mix the block's clicks into `out_l`/`out_r` (accumulate). RT-safe:
    /// the active list is capacity-bounded; clicks beyond it are dropped
    /// (only possible above ~600 BPM with 25 ms tails).
    pub fn render_add(
        &mut self,
        plan: &RenderPlan,
        frame_start: u64,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        let frames = out_l.len() as u64;
        let block_end = frame_start + frames;
        // Schedule clicks starting inside this block.
        loop {
            let frame = plan
                .tempo
                .beat_to_frame(Beat::new(self.next_beat as i64, 1).unwrap_or(Beat::ZERO));
            if frame >= block_end {
                break;
            }
            if frame >= frame_start && self.active.len() < MAX_ACTIVE {
                let (_, beat_in_bar) = plan
                    .time_signatures
                    .beat_to_bar_beat(Beat::new(self.next_beat as i64, 1).unwrap_or(Beat::ZERO));
                self.active.push(ActiveClick {
                    start_frame: frame,
                    accent: beat_in_bar == Beat::ZERO,
                });
            }
            self.next_beat += 1;
        }
        let _ = self.sample_rate;
        // Render active clicks; remove finished ones.
        let mut write = 0;
        for read in 0..self.active.len() {
            let click = &self.active[read];
            let buffer = if click.accent {
                &self.accent_click
            } else {
                &self.normal_click
            };
            let len = buffer.len() as u64;
            let from = click.start_frame.max(frame_start);
            let to = (click.start_frame + len).min(block_end);
            for frame in from..to {
                let pos = (frame - click.start_frame) as usize;
                let sample = buffer[pos] * self.level;
                let at = (frame - frame_start) as usize;
                out_l[at] += sample;
                out_r[at] += sample;
            }
            if click.start_frame + len > block_end {
                self.active[write] = ActiveClick {
                    start_frame: click.start_frame,
                    accent: click.accent,
                };
                write += 1;
            }
        }
        self.active.truncate(write);
    }
}
