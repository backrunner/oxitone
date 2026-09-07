//! Compact native effect panels. Values come exclusively from source/watch.
use crate::plugin_layout::{Choice, Control, Group, Page};
use oxitone_graph::PluginDescriptor;

pub fn pages(descriptor: &PluginDescriptor) -> Option<Vec<Page>> {
    let groups: &[(&str, &[&str])] = match descriptor.plugin_id.as_str() {
        "oxitone.nonlinear-filter" => &[
            ("Filter", &["mode", "cutoffHz", "resonance"]),
            ("Drive", &["driveDb", "outputDb"]),
        ],
        "oxitone.multiband-dynamics" => &[
            ("Dynamics", &["depth", "upwardDb", "downwardRatio", "time"]),
            ("Crossovers", &["lowHz", "highHz"]),
            (
                "Band Levels",
                &["lowGainDb", "midGainDb", "highGainDb", "outputDb"],
            ),
        ],
        "oxitone.compactor" => &[
            ("Density", &["thresholdDb", "upwardDb", "transient"]),
            ("Timing & Output", &["attackMs", "releaseMs", "outputDb"]),
        ],
        "oxitone.resonator" => &[
            ("Modes", &["frequencyHz", "inharmonicity", "decaySeconds"]),
            ("Color", &["brightness", "spread", "outputDb"]),
        ],
        "oxitone.convolver" => &[
            ("Space", &["predelayMs", "outputDb"]),
            ("Bandwidth", &["highpassHz", "lowpassHz"]),
        ],
        "oxitone.tape" => &[
            ("Color", &["driveDb", "toneHz", "outputDb"]),
            ("Motion", &["wow", "flutter"]),
        ],
        "oxitone.flanger" => &[
            ("Sweep", &["rateHz", "depthMs", "delayMs"]),
            ("Feedback & Stereo", &["feedback", "stereo"]),
        ],
        "oxitone.limiter" => &[("Mastering", &["inputDb", "ceilingDb", "releaseMs"])],
        "oxitone.frequency-shifter" => &[("Frequency Shift", &["shiftHz", "stereoHz", "outputDb"])],
        "oxitone.pitch-shifter" => &[("Pitch Shift", &["semitones", "cents", "outputDb"])],
        "oxitone.bitcrush" => &[("Digital Color", &["bits", "rateHz", "jitter", "outputDb"])],
        "oxitone.spreader" => &[("Stereo", &["width", "amount", "bassMonoHz"])],
        _ => return None,
    };
    Some(vec![Page {
        id: "main".into(),
        title: "Controls".into(),
        groups: groups
            .iter()
            .enumerate()
            .map(|(index, (title, ids))| Group {
                id: format!("group-{index}"),
                title: (*title).into(),
                columns: ids.len().min(4) as u32,
                controls: ids
                    .iter()
                    .filter_map(|id| {
                        let spec = descriptor.parameters.iter().find(|p| p.id == *id)?;
                        Some(if *id == "mode" {
                            Control::Choice {
                                parameter: (*id).into(),
                                label: Some("Mode".into()),
                                options: ["Low Pass", "High Pass", "Band Pass", "Notch"]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, label)| Choice {
                                        value: i as f64,
                                        label: label.into(),
                                    })
                                    .collect(),
                            }
                        } else {
                            Control::Knob {
                                parameter: (*id).into(),
                                label: Some(spec.label.clone()),
                            }
                        })
                    })
                    .collect(),
            })
            .collect(),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_panels_bind_every_real_parameter_once() {
        for plugin in oxitone_mixer::builtin_effect_plugins() {
            let descriptor = plugin.descriptor();
            let Some(pages) = pages(descriptor) else {
                continue;
            };
            let bindings: Vec<_> = pages
                .iter()
                .flat_map(|p| &p.groups)
                .flat_map(|g| &g.controls)
                .flat_map(|c| c.bindings())
                .collect();
            assert_eq!(bindings.len(), descriptor.parameters.len());
            for spec in &descriptor.parameters {
                assert_eq!(bindings.iter().filter(|id| **id == spec.id).count(), 1);
            }
            let layout = crate::plugin_layout::Layout {
                ui_version: "1.0".into(),
                plugin_id: descriptor.plugin_id.clone(),
                plugin_version: descriptor.plugin_version.clone(),
                title: "Effect".into(),
                size: crate::plugin_layout::PanelSize {
                    width: 720,
                    height: 540,
                },
                pages,
            };
            crate::plugin_layout_validation::validate(&layout, descriptor).unwrap();
        }
    }
}
