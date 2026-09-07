//! Additive timbre controls. Fixed per-voice state; no graph lookup or allocation.
use super::params as p;
use crate::params::{enum_spec, spec, stepped_continuous};
use oxitone_core::wire::{
    ParameterMapping as Mapping, ParameterSmoothing as Smoothing, ParameterSpec,
    ParameterUnit as Unit,
};

pub fn parameters() -> Vec<ParameterSpec> {
    let semis = |id, label, range: f64| {
        spec(
            id,
            label,
            Unit::Semitones,
            -range,
            range,
            0.,
            Smoothing::None,
            Mapping::Bipolar,
        )
    };
    let mut result = Vec::new();
    for osc in ["oscA", "oscB"] {
        result.extend([
            enum_spec(&format!("{osc}.morphTo"), "Morph target", 0., 5., 3.),
            stepped_continuous(&format!("{osc}.position"), "WT Position", 0., 1., 0.),
            stepped_continuous(&format!("{osc}.phase"), "Start Phase", 0., 1., 0.),
            stepped_continuous(&format!("{osc}.phaseSpread"), "Phase Spread", 0., 1., 0.),
        ]);
    }
    result.extend([
        stepped_continuous("sub.level", "Sub Level", 0., 1., 0.),
        enum_spec("sub.octave", "Sub Octave", -4., 4., -1.),
        stepped_continuous("noise.level", "Noise Level", 0., 1., 0.),
        enum_spec("lfo.shape", "LFO Shape", 0., 3., 0.),
        spec(
            "lfo.rateHz",
            "LFO Rate",
            Unit::Hz,
            0.01,
            30.,
            1.,
            Smoothing::None,
            Mapping::Log,
        ),
        stepped_continuous("lfo.phase", "LFO Start Phase", 0., 1., 0.),
        semis("lfo.pitch", "LFO → Pitch", 12.),
        semis("lfo.cutoff", "LFO → Cutoff", 48.),
        spec(
            "lfo.positionA",
            "LFO → A Position",
            Unit::Normalized,
            -1.,
            1.,
            0.,
            Smoothing::None,
            Mapping::Bipolar,
        ),
        spec(
            "lfo.positionB",
            "LFO → B Position",
            Unit::Normalized,
            -1.,
            1.,
            0.,
            Smoothing::None,
            Mapping::Bipolar,
        ),
        stepped_continuous("lfo.level", "LFO → Level", 0., 1., 0.),
    ]);
    result
}

/// Bipolar source shape shared with the native UI's source-curve display.
pub fn lfo_value(shape: usize, phase: f64) -> f64 {
    let u = phase.rem_euclid(1.);
    match shape {
        1 => 1. - 4. * (u - 0.5).abs(),
        2 => 1. - 2. * u,
        3 => {
            if u < 0.5 {
                1.
            } else {
                -1.
            }
        }
        _ => (std::f64::consts::TAU * u).sin(),
    }
}

#[derive(Clone, Copy)]
pub struct Motion {
    pub positions: [f32; 2],
    pub sub_level: f32,
    pub sub_ratio: f64,
    pub noise_level: f32,
    pub shape: usize,
    pub increment: f64,
    pub pitch: f64,
    pub cutoff: f64,
    pub positions_depth: [f32; 2],
    pub level: f32,
    pub enabled: bool,
}
impl Motion {
    pub fn from_values(v: &[f64], sample_rate: f64) -> Self {
        Self {
            positions: [v[p::OSC_A_POSITION] as f32, v[p::OSC_B_POSITION] as f32],
            sub_level: v[p::SUB_LEVEL] as f32,
            sub_ratio: 2f64.powf(v[p::SUB_OCTAVE]),
            noise_level: v[p::NOISE_LEVEL] as f32,
            shape: v[p::LFO_SHAPE] as usize,
            increment: v[p::LFO_RATE] / sample_rate,
            pitch: v[p::LFO_PITCH],
            cutoff: v[p::LFO_CUTOFF],
            positions_depth: [v[p::LFO_POSITION_A] as f32, v[p::LFO_POSITION_B] as f32],
            level: v[p::LFO_LEVEL] as f32,
            enabled: v[p::LFO_PITCH..=p::LFO_LEVEL].iter().any(|v| *v != 0.),
        }
    }
    pub fn value(&self, phase: f64) -> f64 {
        if self.enabled {
            lfo_value(self.shape, phase)
        } else {
            0.
        }
    }
}

pub struct MotionState {
    pub phase: f64,
    pub sub_phase: f64,
    noise: u32,
}
impl MotionState {
    pub fn new() -> Self {
        Self {
            phase: 0.,
            sub_phase: 0.,
            noise: 0x6d2b79f5,
        }
    }
    pub fn reset(&mut self, note: u8, phase: f64) {
        self.phase = phase.rem_euclid(1.);
        self.sub_phase = 0.;
        self.noise = 0x6d2b79f5 ^ (note as u32).wrapping_mul(0x9e3779b9);
    }
    pub fn noise(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f64 / u32::MAX as f64 * 2. - 1.) as f32
    }
}
