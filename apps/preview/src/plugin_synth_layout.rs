//! Synth modules follow the sound path; modulation and matrix have separate pages.
use crate::plugin_layout::{Choice, Control, Group, Page};
#[path = "plugin_synth_matrix.rs"]
mod matrix;
#[path = "plugin_synth_oscillator.rs"]
mod oscillator;

fn knob(id: &str, label: &str) -> Control {
    Control::Knob {
        parameter: id.into(),
        label: Some(label.into()),
    }
}
fn choice(id: &str, label: &str, names: &[&str]) -> Control {
    Control::Choice {
        parameter: id.into(),
        label: Some(label.into()),
        options: names
            .iter()
            .enumerate()
            .map(|(i, n)| Choice {
                value: i as f64,
                label: (*n).into(),
            })
            .collect(),
    }
}
fn group(id: &str, title: &str, columns: u32, controls: Vec<Control>) -> Group {
    Group {
        id: id.into(),
        title: title.into(),
        columns,
        controls,
    }
}
pub fn pages() -> Vec<Page> {
    vec![
        Page {
            id: "sound".into(),
            title: "Oscillators".into(),
            groups: vec![
                oscillator::group("oscA", "Oscillator A"),
                oscillator::group("oscB", "Oscillator B"),
            ],
        },
        shaping(),
        modulation(),
        matrix::page(),
    ]
}

fn shaping() -> Page {
    Page {
        id: "shaping".into(),
        title: "Tone".into(),
        groups: vec![
            group(
                "filter",
                "Filter",
                3,
                vec![
                    Control::FilterResponse {
                        label: None,
                        mode: "filter.type".into(),
                        cutoff: "filter.cutoff".into(),
                        resonance: "filter.resonance".into(),
                    },
                    choice(
                        "filter.type",
                        "Mode",
                        &["Low pass", "High pass", "Band pass"],
                    ),
                    knob("filter.cutoff", "Cutoff"),
                    knob("filter.resonance", "Resonance"),
                    knob("filterEnv.amount", "Envelope depth"),
                ],
            ),
            group(
                "sub",
                "Sub & noise",
                3,
                vec![
                    Control::SubOscillator {
                        label: None,
                        wave: "sub.wave".into(),
                        octave: "sub.octave".into(),
                        level: "sub.level".into(),
                    },
                    choice(
                        "sub.wave",
                        "Wave",
                        &["Sine", "Triangle", "Saw", "Square", "Pulse", "Rounded"],
                    ),
                    knob("sub.octave", "Octave"),
                    knob("sub.level", "Sub level"),
                    knob("noise.level", "Noise level"),
                ],
            ),
            group(
                "mix",
                "Oscillator mix",
                3,
                vec![
                    knob("osc.mix", "A / B"),
                    knob("fm", "FM from B"),
                    knob("ring", "Ring modulation"),
                ],
            ),
            group(
                "voice",
                "Voice & output",
                3,
                vec![
                    choice("voiceMode", "Voicing", &["Poly", "Mono", "Legato"]),
                    knob("glide", "Glide"),
                    knob("level", "Volume"),
                    knob("pan", "Pan"),
                ],
            ),
        ],
    }
}

fn modulation() -> Page {
    Page {
        id: "modulation".into(),
        title: "Modulation".into(),
        groups: vec![
            envelope("amp", "Amplitude envelope"),
            envelope("filterEnv", "Filter envelope"),
            lfo("lfo", "LFO 1"),
            group(
                "depths",
                "LFO 1 destinations",
                3,
                vec![
                    knob("lfo.cutoff", "Filter"),
                    knob("lfo.pitch", "Pitch"),
                    knob("lfo.level", "Amplitude"),
                    knob("lfo.positionA", "A position"),
                    knob("lfo.positionB", "B position"),
                ],
            ),
            lfo("lfo2", "LFO 2"),
            envelope("modEnv", "Modulation envelope"),
        ],
    }
}
fn lfo(id: &str, title: &str) -> Group {
    group(
        id,
        title,
        2,
        vec![
            Control::LfoCurve {
                label: None,
                shape: format!("{id}.shape"),
                rate: format!("{id}.rateHz"),
                phase: format!("{id}.phase"),
            },
            choice(
                &format!("{id}.shape"),
                "Shape",
                &["Sine", "Triangle", "Ramp", "Square"],
            ),
            knob(&format!("{id}.rateHz"), "Rate"),
            knob(&format!("{id}.phase"), "Phase"),
        ],
    )
}
fn envelope(id: &str, title: &str) -> Group {
    group(
        id,
        title,
        4,
        vec![
            Control::Envelope {
                label: None,
                attack: format!("{id}.attack"),
                decay: format!("{id}.decay"),
                sustain: format!("{id}.sustain"),
                release: format!("{id}.release"),
            },
            knob(&format!("{id}.attack"), "Attack"),
            knob(&format!("{id}.decay"), "Decay"),
            knob(&format!("{id}.sustain"), "Sustain"),
            knob(&format!("{id}.release"), "Release"),
            knob(&format!("{id}.attackCurve"), "Attack curve"),
            knob(&format!("{id}.decayCurve"), "Decay curve"),
            knob(&format!("{id}.releaseCurve"), "Release curve"),
        ],
    )
}
