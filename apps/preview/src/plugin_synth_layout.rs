//! Synth signal flow: A/B and filter above voice/output, plus envelopes and LFO routing.
use crate::plugin_layout::{Choice, Control, Group, ModulationRoute, Page};
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
    let oscillator = |id: &str, title: &str| {
        group(
            id,
            title,
            3,
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
                },
                knob(&format!("{id}.position"), "WT position"),
                knob(&format!("{id}.pitch"), "Pitch"),
                knob(&format!("{id}.detune"), "Detune · ct"),
                knob(&format!("{id}.spread"), "Width"),
                knob(&format!("{id}.phase"), "Phase"),
                knob(&format!("{id}.phaseSpread"), "Phase spread"),
            ],
        )
    };
    let envelope = |id: &str, title: &str| {
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
            ],
        )
    };
    vec![
        Page {
            id: "sound".into(),
            title: "Oscillators".into(),
            groups: vec![
                oscillator("oscA", "OSC A"),
                oscillator("oscB", "OSC B"),
                group(
                    "filter",
                    "FILTER",
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
                        knob("filterEnv.amount", "ENV 2 depth"),
                        knob("osc.mix", "A / B mix"),
                        knob("glide", "Glide"),
                    ],
                ),
                group(
                    "voice",
                    "VOICE / OUTPUT",
                    6,
                    vec![
                        choice("voiceMode", "Mode", &["Poly", "Mono", "Legato"]),
                        knob("level", "Output"),
                        knob("pan", "Pan"),
                        knob("sub.level", "Sub"),
                        Control::Readout {
                            parameter: "sub.octave".into(),
                            label: Some("Sub octave".into()),
                        },
                        knob("noise.level", "Noise"),
                    ],
                ),
            ],
        },
        Page {
            id: "modulation".into(),
            title: "Modulation".into(),
            groups: vec![
                group(
                    "lfo",
                    "LFO 1",
                    3,
                    vec![
                        Control::LfoCurve {
                            label: None,
                            shape: "lfo.shape".into(),
                            rate: "lfo.rateHz".into(),
                            phase: "lfo.phase".into(),
                        },
                        choice(
                            "lfo.shape",
                            "Shape",
                            &["Sine", "Triangle", "Ramp", "Square"],
                        ),
                        knob("lfo.rateHz", "Rate"),
                        knob("lfo.phase", "Phase"),
                    ],
                ),
                group(
                    "depths",
                    "LFO 1 → DESTINATIONS",
                    3,
                    vec![
                        knob("lfo.cutoff", "Filter · st"),
                        knob("lfo.pitch", "Pitch · st"),
                        knob("lfo.level", "Amplitude"),
                        knob("lfo.positionA", "A position"),
                        knob("lfo.positionB", "B position"),
                    ],
                ),
                group(
                    "routes",
                    "MODULATION ROUTING",
                    3,
                    vec![Control::Modulation {
                        label: None,
                        routes: [
                            ("Filter cutoff", "lfo.cutoff"),
                            ("Oscillator pitch", "lfo.pitch"),
                            ("Osc A position", "lfo.positionA"),
                            ("Osc B position", "lfo.positionB"),
                            ("Amplitude", "lfo.level"),
                        ]
                        .map(|(label, amount)| ModulationRoute {
                            label: label.into(),
                            amount: amount.into(),
                        })
                        .into(),
                    }],
                ),
                envelope("amp", "ENV 1 · AMPLITUDE"),
                envelope("filterEnv", "ENV 2 · FILTER"),
            ],
        },
    ]
}
