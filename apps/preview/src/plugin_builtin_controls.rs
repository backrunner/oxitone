//! Musical labels and explicit enum choices, independent of DSP descriptor diagnostics.
use crate::plugin_layout::{Choice, Control};
use oxitone_core::wire::ParameterUnit;
use oxitone_graph::PluginDescriptor;

pub fn control(descriptor: &PluginDescriptor, id: &str) -> Control {
    let spec = descriptor
        .parameters
        .iter()
        .find(|p| p.id == id)
        .expect("bundled panel binding must exist in its descriptor");
    let label = match id {
        "timeBeats" => "Tempo time",
        "timeSeconds" => "Free time",
        "rootKey" => "Root note",
        "velocitySensitivity" => "Velocity",
        "sidechainHighpassHz" => "High pass",
        "upwardDb" => "Upward gain",
        "downwardRatio" => "Ratio",
        "upperThresholdDb" => "Upper",
        "lowerThresholdDb" => "Lower",
        "feedbackFilterHz" => "Low pass",
        "highpassHz" => "High pass",
        "lowpassHz" => "Low pass",
        "lowHz" => "Low split",
        "highHz" => "High split",
        "amp.attack" => "Attack",
        "amp.decay" => "Decay",
        "amp.sustain" => "Sustain",
        "amp.release" => "Release",
        id if id.starts_with("band") && id.ends_with("freqHz") => "Frequency",
        id if id.starts_with("band") && id.ends_with("gainDb") => "Gain",
        id if id.starts_with("band") && id.ends_with(".q") => "Q",
        _ => spec.label.split(" (").next().unwrap_or(&spec.label),
    };
    let labels: &[&str] = match (descriptor.plugin_id.as_str(), id) {
        ("oxitone.filter", "mode") => &["Low pass", "High pass", "Band pass"],
        ("oxitone.nonlinear-filter", "mode") => &["Low pass", "High pass", "Band pass", "Notch"],
        ("oxitone.distortion", "mode") => &["Soft", "Hard", "Fold", "Asymmetric"],
        ("oxitone.clipper", "mode") => &["Hard", "Soft"],
        ("oxitone.saturator", "curve") => &["Tanh", "Cubic"],
        ("oxitone.saturator", "oversample") => &["2×", "4×"],
        ("oxitone.compressor", "detector") => &["Peak", "RMS"],
        ("oxitone.phaser", "stages") => &["4 stages", "8 stages"],
        ("oxitone.sampler", "loop") => &["Off", "Forward"],
        (_, "polarity") => &["Normal", "Inverted"],
        _ => &[],
    };
    if !labels.is_empty() {
        return Control::Choice {
            parameter: id.into(),
            label: Some(label.into()),
            options: labels
                .iter()
                .enumerate()
                .map(|(i, label)| Choice {
                    value: i as f64,
                    label: (*label).into(),
                })
                .collect(),
        };
    }
    if id == "tempoFactor" {
        Control::Readout {
            parameter: id.into(),
            label: Some(label.into()),
        }
    } else if spec.unit == ParameterUnit::Enum && spec.min == 0. && spec.max == 1. {
        Control::Toggle {
            parameter: id.into(),
            label: Some(label.into()),
        }
    } else {
        Control::Knob {
            parameter: id.into(),
            label: Some(label.into()),
        }
    }
}

pub fn sampler_pages(descriptor: &PluginDescriptor) -> Option<Vec<crate::plugin_layout::Page>> {
    if descriptor.plugin_version != "1.0.0" {
        return None;
    }
    let groups: &[(&str, &[&str])] = match descriptor.plugin_id.as_str() {
        "oxitone.sampler" => &[
            ("Playback", &["loop", "rootKey", "start"]),
            (
                "Amplitude",
                &["amp.attack", "amp.decay", "amp.sustain", "amp.release"],
            ),
            ("Output", &["velocitySensitivity", "level", "pan"]),
        ],
        "oxitone.multisampler" => &[
            ("Playback", &["transpose", "start", "loop"]),
            (
                "Amplitude",
                &["amp.attack", "amp.decay", "amp.sustain", "amp.release"],
            ),
            ("Output", &["velocitySensitivity", "level", "pan"]),
        ],
        "oxitone.slicer" => &[("Playback", &["level", "pan", "tempoFactor"])],
        _ => return None,
    };
    Some(vec![crate::plugin_layout::Page {
        id: "main".into(),
        title: "Instrument".into(),
        groups: groups
            .iter()
            .enumerate()
            .map(|(i, (title, ids))| crate::plugin_layout::Group {
                id: format!("group-{i}"),
                title: (*title).into(),
                columns: ids.len() as u32,
                controls: ids.iter().map(|id| control(descriptor, id)).collect(),
            })
            .collect(),
    }])
}
