//! Compact descriptor-based fallback, with a purpose-built Wavetable front panel.
use crate::{
    plugin_details::PluginDetails,
    plugin_layout::{Choice, Control, Group, Layout, Page, PanelSize},
};
use oxitone_core::wire::ParameterUnit;

pub fn panel(details: &PluginDetails) -> Layout {
    let descriptor = &details.info.descriptor;
    let pages = if descriptor.plugin_id == "oxitone.wavetable"
        && descriptor.plugin_version == "1.0.0"
    {
        wavetable()
    } else {
        let mut groups: Vec<Group> = Vec::new();
        for spec in &descriptor.parameters {
            let (group, label) = spec
                .id
                .rsplit_once('.')
                .unwrap_or(("Controls", &spec.label));
            if !groups.iter().any(|g| g.id == group) {
                groups.push(Group {
                    id: group.into(),
                    title: group.into(),
                    columns: 4,
                    controls: vec![],
                });
            }
            let control = if spec.unit == ParameterUnit::Enum && spec.min == 0. && spec.max == 1. {
                Control::Toggle {
                    parameter: spec.id.clone(),
                    label: Some(label.into()),
                }
            } else if spec.unit == ParameterUnit::Enum {
                Control::Readout {
                    parameter: spec.id.clone(),
                    label: Some(label.into()),
                }
            } else {
                knob(&spec.id, label)
            };
            groups
                .iter_mut()
                .find(|g| g.id == group)
                .unwrap()
                .controls
                .push(control);
        }
        for group in &mut groups {
            group.columns = (group.controls.len() as u32).clamp(1, 4);
        }
        vec![Page {
            id: "main".into(),
            title: "Controls".into(),
            groups,
        }]
    };
    let count = descriptor.parameters.len();
    Layout {
        ui_version: "1.0".into(),
        plugin_id: descriptor.plugin_id.clone(),
        plugin_version: descriptor.plugin_version.clone(),
        title: details.name.clone(),
        size: if descriptor.plugin_id == "oxitone.wavetable" {
            PanelSize {
                width: 920,
                height: 556,
            }
        } else {
            PanelSize {
                width: if count > 8 { 720 } else { 520 },
                height: (230 + count.div_ceil(4) as u32 * 94).clamp(280, 640),
            }
        },
        pages,
    }
}
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
            .map(|(i, name)| Choice {
                value: i as f64,
                label: (*name).into(),
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
fn wavetable() -> Vec<Page> {
    let oscillator = |id: &str, title: &str| {
        group(
            id,
            title,
            3,
            vec![
                choice(
                    &format!("{id}.wavetable"),
                    "Wave",
                    &["Sine", "Saw", "Square", "Triangle"],
                ),
                knob(&format!("{id}.pitch"), "Pitch"),
                Control::Readout {
                    parameter: format!("{id}.unison"),
                    label: Some("Voices".into()),
                },
                knob(&format!("{id}.detune"), "Detune"),
                knob(&format!("{id}.spread"), "Spread"),
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
                    label: Some(title.into()),
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
            title: "Sound".into(),
            groups: vec![
                oscillator("oscA", "Oscillator A"),
                oscillator("oscB", "Oscillator B"),
                group(
                    "filter",
                    "Filter",
                    3,
                    vec![
                        choice(
                            "filter.type",
                            "Mode",
                            &["Low pass", "High pass", "Band pass"],
                        ),
                        knob("filter.cutoff", "Cutoff"),
                        knob("filter.resonance", "Resonance"),
                        knob("filterEnv.amount", "Env amount"),
                        knob("osc.mix", "A / B blend"),
                    ],
                ),
                group(
                    "voice",
                    "Voice & output",
                    4,
                    vec![
                        choice("voiceMode", "Voice mode", &["Poly", "Mono", "Legato"]),
                        knob("glide", "Glide"),
                        knob("level", "Level"),
                        knob("pan", "Pan"),
                    ],
                ),
            ],
        },
        Page {
            id: "envelopes".into(),
            title: "Envelopes".into(),
            groups: vec![
                envelope("amp", "Amplitude"),
                envelope("filterEnv", "Filter envelope"),
            ],
        },
    ]
}
