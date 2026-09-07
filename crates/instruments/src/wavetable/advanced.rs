//! Fixed-size modulation matrix and oscillator controls. No runtime lookup/allocation.
use crate::params::{enum_spec, seconds, spec, stepped_continuous};
use oxitone_core::wire::{
    ParameterMapping as Map, ParameterSmoothing as Smooth, ParameterSpec, ParameterUnit as Unit,
};

pub const START: usize = 46;
pub const BANK_A: usize = START;
pub const WARP_MODE_A: usize = START + 1;
pub const WARP_A: usize = START + 2;
pub const BANK_B: usize = START + 3;
pub const WARP_MODE_B: usize = START + 4;
pub const WARP_B: usize = START + 5;
pub const FM: usize = START + 6;
pub const RING: usize = START + 7;
pub const LFO2: usize = START + 8;
pub const ENV: usize = START + 11;
pub const CURVES: usize = START + 15;
pub const MACROS: usize = START + 24;
pub const ROUTES: usize = START + 28;
pub const OCTAVE_A: usize = ROUTES + 8 * 4;
pub const OCTAVE_B: usize = OCTAVE_A + 1;
pub const SUB_WAVE: usize = OCTAVE_A + 2;
pub const LEVEL_A: usize = OCTAVE_A + 3;
pub const LEVEL_B: usize = OCTAVE_A + 4;
pub const COUNT: usize = OCTAVE_A + 5;

pub fn parameters() -> Vec<ParameterSpec> {
    let normalized = |id: &str, label: &str| stepped_continuous(id, label, 0., 1., 0.);
    let bipolar = |id: &str, label: &str| {
        spec(
            id,
            label,
            Unit::Normalized,
            -1.,
            1.,
            0.,
            Smooth::None,
            Map::Bipolar,
        )
    };
    let mut result = Vec::new();
    for osc in ["oscA", "oscB"] {
        result.extend([
            enum_spec(&format!("{osc}.bank"), "WT Bank", 0., 3., 0.),
            enum_spec(&format!("{osc}.warpMode"), "Warp Mode", 0., 3., 0.),
            normalized(&format!("{osc}.warp"), "Warp Amount"),
        ]);
    }
    result.extend([
        normalized("fm", "B → A Phase Modulation"),
        normalized("ring", "Ring Modulation"),
        enum_spec("lfo2.shape", "LFO 2 Shape", 0., 3., 0.),
        spec(
            "lfo2.rateHz",
            "LFO 2 Rate",
            Unit::Hz,
            0.01,
            30.,
            0.5,
            Smooth::None,
            Map::Log,
        ),
        normalized("lfo2.phase", "LFO 2 Phase"),
        seconds("modEnv.attack", "Mod Attack", 0.005),
        seconds("modEnv.decay", "Mod Decay", 0.3),
        normalized("modEnv.sustain", "Mod Sustain"),
        seconds("modEnv.release", "Mod Release", 0.2),
    ]);
    for env in ["amp", "filterEnv", "modEnv"] {
        for stage in ["attack", "decay", "release"] {
            result.push(bipolar(
                &format!("{env}.{stage}Curve"),
                &format!("{stage} Curve"),
            ));
        }
    }
    for i in 1..=4 {
        result.push(normalized(&format!("macro{i}"), &format!("Macro {i}")));
    }
    for i in 0..8 {
        result.extend([
            enum_spec(&format!("mod.{i}.source"), "Source", 0., 12., 0.),
            enum_spec(&format!("mod.{i}.target"), "Target", 0., 10., 0.),
            bipolar(&format!("mod.{i}.amount"), "Amount"),
            bipolar(&format!("mod.{i}.curve"), "Curve"),
        ]);
    }
    result.extend([
        enum_spec("oscA.octave", "Osc A Octave", -4., 4., 0.),
        enum_spec("oscB.octave", "Osc B Octave", -4., 4., 0.),
        enum_spec("sub.wave", "Sub Wave", 0., 5., 0.),
        stepped_continuous("oscA.level", "Osc A Level", 0., 1., 1.),
        stepped_continuous("oscB.level", "Osc B Level", 0., 1., 1.),
    ]);
    result
}

#[derive(Clone, Copy, Default)]
pub struct Route {
    source: usize,
    target: usize,
    amount: f32,
    curve: f32,
}
#[derive(Clone, Copy)]
pub struct Advanced {
    pub banks: [usize; 2],
    pub warp_modes: [usize; 2],
    pub warps: [f32; 2],
    pub fm: f32,
    pub ring: f32,
    pub lfo2_shape: usize,
    pub lfo2_increment: f64,
    pub macros: [f32; 4],
    routes: [Route; 8],
    pub enabled: bool,
    pub levels: [f32; 2],
}
impl Advanced {
    pub fn new(v: &[f64], sample_rate: f64) -> Self {
        let routes = std::array::from_fn(|i| {
            let p = ROUTES + 4 * i;
            Route {
                source: v[p] as usize,
                target: v[p + 1] as usize,
                amount: v[p + 2] as f32,
                curve: v[p + 3] as f32,
            }
        });
        Self {
            banks: [v[BANK_A] as usize, v[BANK_B] as usize],
            warp_modes: [v[WARP_MODE_A] as usize, v[WARP_MODE_B] as usize],
            warps: [v[WARP_A] as f32, v[WARP_B] as f32],
            fm: v[FM] as f32,
            ring: v[RING] as f32,
            lfo2_shape: v[LFO2] as usize,
            lfo2_increment: v[LFO2 + 1] / sample_rate,
            macros: std::array::from_fn(|i| v[MACROS + i] as f32),
            levels: [v[LEVEL_A] as f32, v[LEVEL_B] as f32],
            enabled: routes.iter().any(|r| r.source != 0 && r.amount != 0.)
                || v[BANK_A] != 0.
                || v[BANK_B] != 0.
                || v[WARP_MODE_A] != 0.
                || v[WARP_MODE_B] != 0.
                || v[FM] != 0.
                || v[RING] != 0.,
            routes,
        }
    }
    #[inline]
    pub fn evaluate(&self, sources: &[f32; 13]) -> [f32; 11] {
        let mut values = [0.; 11];
        for route in self.routes {
            if route.source == 0 || route.amount == 0. {
                continue;
            }
            let source = sources[route.source];
            let magnitude = source.abs();
            let shaped = source.signum() * (magnitude + route.curve * magnitude * (1. - magnitude));
            values[route.target] += shaped * route.amount;
        }
        values
    }
}
