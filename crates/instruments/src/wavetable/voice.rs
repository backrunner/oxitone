//! WavetableSynth voice: dual mip-mapped wavetable oscillators with unison,
//! stereo biquad filter, amp ADSR, and optional filter envelope. All state is
//! preallocated; `render` is RT-safe and allocation-free.

use super::motion::{Motion, MotionState};
use oxitone_dsp::biquad::{BiquadCoeffs, BiquadF64, BiquadKind};
#[path = "voice_render.rs"]
mod render;
use oxitone_dsp::envelope::Adsr;
use oxitone_dsp::gain_pan::equal_power_gains;
use oxitone_dsp::oscillator::{Wavetable, WavetableReader};
use oxitone_dsp::unison::{plan_unison, MAX_UNISON};

/// Filter coefficient update granularity in frames (control chunks). Filter
/// cutoff/resonance smoothers and the filter envelope are evaluated per
/// chunk, not per sample — transcendental coefficient math stays bounded.
pub const FILTER_CHUNK: usize = 32;

fn unity_coeffs() -> BiquadCoeffs {
    BiquadCoeffs {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    }
}

/// One oscillator's unison stack: readers plus the precomputed mix plan.
pub struct OscState {
    pub readers: [WavetableReader; MAX_UNISON],
    /// Per-unison frequency ratios, with the osc pitch offset folded in.
    pub ratios: [f64; MAX_UNISON],
    pub gain_l: [f32; MAX_UNISON],
    pub gain_r: [f32; MAX_UNISON],
    pub count: usize,
}

impl OscState {
    fn new() -> Self {
        Self {
            readers: core::array::from_fn(|_| WavetableReader::new()),
            ratios: [1.0; MAX_UNISON],
            gain_l: [0.0; MAX_UNISON],
            gain_r: [0.0; MAX_UNISON],
            count: 1,
        }
    }

    pub fn start_phases(&mut self, phase: f64, spread: f64) {
        for (u, reader) in self.readers.iter_mut().enumerate() {
            reader.set_phase(phase + spread * u as f64 / self.count.max(1) as f64);
        }
    }

    /// Rebuild the unison plan and re-select mip levels. Control rate
    /// (note-on or parameter change), never per sample.
    pub fn configure(
        &mut self,
        table: &Wavetable,
        base_freq: f64,
        pitch_semis: f64,
        unison: usize,
        detune_cents: f64,
        spread: f32,
    ) {
        let plan = plan_unison(unison, detune_cents, spread);
        let pitch_ratio = 2f64.powf(pitch_semis / 12.0);
        self.count = plan.count;
        for u in 0..plan.count {
            let ratio = plan.ratios[u] * pitch_ratio;
            self.ratios[u] = ratio;
            let (gl, gr) = equal_power_gains(plan.pans[u]);
            self.gain_l[u] = gl * plan.gains[u];
            self.gain_r[u] = gr * plan.gains[u];
            self.readers[u].prepare(table, base_freq * ratio);
        }
    }
}

/// Per-render shared inputs (tables, chunk envelopes) so `Voice::render`
/// stays a small argument list.
pub struct VoiceContext<'a> {
    pub table_a: &'a Wavetable,
    pub table_b: &'a Wavetable,
    pub morph_a: &'a Wavetable,
    pub morph_b: &'a Wavetable,
    pub motion: Motion,
    /// Per-sample osc-B mix (0 = A only), filled once per segment.
    pub mix: &'a [f32],
    /// Per-`FILTER_CHUNK` smoothed cutoff Hz / resonance (0..1).
    pub cutoff_chunks: &'a [f64],
    pub resonance_chunks: &'a [f64],
    pub filter_kind: BiquadKind,
    /// Filter envelope depth in semitones; 0 disables the filter envelope.
    pub fenv_amount_semis: f64,
    pub sample_rate: f64,
}

pub struct Voice {
    pub osc_a: OscState,
    pub osc_b: OscState,
    pub amp: Adsr,
    pub fenv: Adsr,
    filter_l: BiquadF64,
    filter_r: BiquadF64,
    pub note: u8,
    pub velocity: f32,
    /// Current pitch in MIDI semitones (glide integrates toward the target).
    pub pitch_semis: f64,
    pub pitch_target: f64,
    /// Semitones per sample; 0 when not gliding.
    pub glide_step: f64,
    pub base_freq: f64,
    pub motion: MotionState,
    control_remaining: usize,
    render_freq: f64,
}

impl Voice {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            osc_a: OscState::new(),
            osc_b: OscState::new(),
            amp: Adsr::new(sample_rate),
            fenv: Adsr::new(sample_rate),
            filter_l: BiquadF64::new(unity_coeffs()),
            filter_r: BiquadF64::new(unity_coeffs()),
            note: 0,
            velocity: 0.0,
            pitch_semis: 60.0,
            pitch_target: 60.0,
            glide_step: 0.0,
            base_freq: 261.63,
            motion: MotionState::new(),
            control_remaining: 0,
            render_freq: 261.63,
        }
    }

    /// Reset per-note playback state (oscillator phases, filter state).
    /// Called on non-legato voice (re)starts: a fresh note must not inherit
    /// the previous occupant's phase/filter memory — that state evolves
    /// during the release tail's silent render, which ends at segment
    /// (block) granularity and would otherwise make the rendered output
    /// depend on the host's block size (03-audio-runtime-spec.md §Offline
    /// parity). Envelope levels are intentionally kept (click-free
    /// retrigger starts the attack from the current level).
    pub fn reset_playback_state(&mut self) {
        self.control_remaining = 0;
        for reader in &mut self.osc_a.readers {
            reader.set_phase(0.0);
        }
        for reader in &mut self.osc_b.readers {
            reader.set_phase(0.0);
        }
        self.filter_l.reset();
        self.filter_r.reset();
    }

    pub fn invalidate_control(&mut self) {
        self.control_remaining = 0;
    }

    fn advance_glide(&mut self, frames: f64) {
        if self.glide_step == 0.0 || self.pitch_semis == self.pitch_target {
            return;
        }
        let next = self.pitch_semis + self.glide_step * frames;
        let overshot = (self.glide_step > 0.0) == (next > self.pitch_target);
        self.pitch_semis = if overshot { self.pitch_target } else { next };
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}
