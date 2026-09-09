//! One oscillator module with waveform selection, tuning, unison and phase controls.
use super::{choice, knob};
use crate::plugin_layout::{Control, Group};

pub fn group(id: &str, title: &str) -> Group {
    super::group(
        id,
        title,
        5,
        vec![
            Control::Oscillator {
                label: None,
                wave: format!("{id}.wavetable"),
                morph_to: format!("{id}.morphTo"),
                position: format!("{id}.position"),
                phase: format!("{id}.phase"),
                unison: format!("{id}.unison"),
                detune: format!("{id}.detune"),
                spread: format!("{id}.spread"),
                bank: Some(format!("{id}.bank")),
                warp_mode: Some(format!("{id}.warpMode")),
                warp: Some(format!("{id}.warp")),
                octave: Some(format!("{id}.octave")),
            },
            choice(
                &format!("{id}.bank"),
                "Bank",
                &["Pair", "Analog", "Digital", "Vowel"],
            ),
            choice(
                &format!("{id}.wavetable"),
                "Wave",
                &["Sine", "Saw", "Square", "Triangle", "Organ", "Glass"],
            ),
            choice(
                &format!("{id}.morphTo"),
                "Morph to",
                &["Sine", "Saw", "Square", "Triangle", "Organ", "Glass"],
            ),
            choice(
                &format!("{id}.warpMode"),
                "Warp",
                &["Off", "Bend", "Asymmetric", "Sync"],
            ),
            knob(&format!("{id}.position"), "Position"),
            knob(&format!("{id}.level"), "Level"),
            knob(&format!("{id}.octave"), "Octave"),
            knob(&format!("{id}.pitch"), "Pitch"),
            knob(&format!("{id}.phase"), "Phase"),
            knob(&format!("{id}.unison"), "Unison"),
            knob(&format!("{id}.detune"), "Detune (ct)"),
            knob(&format!("{id}.spread"), "Width"),
            knob(&format!("{id}.phaseSpread"), "Phase spread"),
            knob(&format!("{id}.warp"), "Warp amount"),
        ],
    )
}
