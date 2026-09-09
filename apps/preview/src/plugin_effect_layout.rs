//! Purpose-built groupings for every bundled effect; descriptors own ranges and defaults.
use crate::plugin_layout::{Group, Page};
use oxitone_graph::PluginDescriptor;

pub fn pages(descriptor: &PluginDescriptor) -> Option<Vec<Page>> {
    if descriptor.plugin_version != "1.0.0" {
        return None;
    }
    let groups: &[(&str, &[&str])] = match descriptor.plugin_id.as_str() {
        "oxitone.eq" => &[
            ("Low shelf", &["band1.freqHz", "band1.gainDb"]),
            ("Low mid", &["band2.freqHz", "band2.q", "band2.gainDb"]),
            ("High mid", &["band3.freqHz", "band3.q", "band3.gainDb"]),
            ("High shelf", &["band4.freqHz", "band4.gainDb"]),
        ],
        "oxitone.filter" => &[("Filter", &["mode", "cutoffHz", "resonance"])],
        "oxitone.nonlinear-filter" => &[
            ("Filter", &["mode", "cutoffHz", "resonance"]),
            ("Drive", &["driveDb", "outputDb"]),
        ],
        "oxitone.compressor" => &[
            (
                "Compression",
                &["thresholdDb", "ratio", "kneeDb", "makeupDb"],
            ),
            (
                "Detector",
                &["detector", "attackMs", "releaseMs", "sidechainHighpassHz"],
            ),
        ],
        "oxitone.gate" => &[
            ("Gate", &["thresholdDb", "hysteresisDb", "rangeDb"]),
            ("Envelope", &["attackMs", "holdMs", "releaseMs"]),
        ],
        "oxitone.limit" => &[("Peak control", &["ceilingDb", "releaseMs", "saturate"])],
        "oxitone.limiter" => &[("Peak control", &["inputDb", "ceilingDb", "releaseMs"])],
        "oxitone.clipper" => &[("Clipping", &["mode", "driveDb", "outputDb"])],
        "oxitone.saturator" => &[
            ("Color", &["curve", "driveDb", "outputDb"]),
            ("Quality", &["oversample"]),
        ],
        "oxitone.distortion" => &[
            ("Shaping", &["mode", "driveDb", "bias"]),
            ("Output", &["toneHz", "outputDb"]),
        ],
        "oxitone.multiband" => &[
            ("Dynamics", &["depth", "upwardDb", "downwardRatio"]),
            (
                "Detector",
                &[
                    "lowerThresholdDb",
                    "upperThresholdDb",
                    "attackMs",
                    "releaseMs",
                ],
            ),
            ("Crossovers", &["lowHz", "highHz"]),
            (
                "Band levels",
                &["lowGainDb", "midGainDb", "highGainDb", "outputDb"],
            ),
        ],
        "oxitone.multiband-dynamics" => &[
            ("Dynamics", &["depth", "upwardDb", "downwardRatio", "time"]),
            ("Crossovers", &["lowHz", "highHz"]),
            (
                "Band levels",
                &["lowGainDb", "midGainDb", "highGainDb", "outputDb"],
            ),
        ],
        "oxitone.compactor" => &[
            ("Density", &["thresholdDb", "upwardDb", "transient"]),
            ("Timing & output", &["attackMs", "releaseMs", "outputDb"]),
        ],
        "oxitone.delay" => &[
            ("Time", &["timeBeats", "timeSeconds", "pingPong"]),
            (
                "Feedback",
                &["feedback", "feedbackFilterHz", "highpassHz", "ducking"],
            ),
        ],
        "oxitone.reverb" => &[
            ("Space", &["decaySeconds", "predelayMs", "damping"]),
            (
                "Wet signal",
                &["highpassHz", "lowpassHz", "width", "ducking"],
            ),
        ],
        "oxitone.convolver" => &[
            ("Space", &["predelayMs", "outputDb"]),
            ("Bandwidth", &["highpassHz", "lowpassHz"]),
        ],
        "oxitone.resonator" => &[
            ("Modes", &["frequencyHz", "inharmonicity", "decaySeconds"]),
            ("Color", &["brightness", "spread", "outputDb"]),
        ],
        "oxitone.chorus" => &[("Voices", &["rateHz", "depth", "delayMs"])],
        "oxitone.flanger" => &[
            ("Sweep", &["rateHz", "depthMs", "delayMs"]),
            ("Feedback & stereo", &["feedback", "stereo"]),
        ],
        "oxitone.phaser" => &[
            ("Sweep", &["rateHz", "depth", "centerHz"]),
            ("Notches", &["stages", "feedback"]),
        ],
        "oxitone.tape" => &[
            ("Color", &["driveDb", "toneHz", "outputDb"]),
            ("Motion", &["wow", "flutter"]),
        ],
        "oxitone.frequency-shifter" => &[("Translation", &["shiftHz", "stereoHz", "outputDb"])],
        "oxitone.pitch-shifter" => &[("Transpose", &["semitones", "cents", "outputDb"])],
        "oxitone.bitcrush" => &[("Digital color", &["bits", "rateHz", "jitter", "outputDb"])],
        "oxitone.spreader" => &[("Stereo", &["width", "amount", "bassMonoHz"])],
        "oxitone.utility" => &[("Signal", &["gainDb", "width", "mono", "polarity"])],
        _ => return None,
    };
    Some(vec![Page {
        id: "main".into(),
        title: "Controls".into(),
        groups: groups
            .iter()
            .enumerate()
            .map(|(index, (title, ids))| {
                let controls: Vec<_> = ids
                    .iter()
                    .map(|id| crate::plugin_builtin_controls::control(descriptor, id))
                    .collect();
                Group {
                    id: format!("group-{index}"),
                    title: (*title).into(),
                    columns: controls
                        .iter()
                        .filter(|c| !matches!(c, crate::plugin_layout::Control::Choice { .. }))
                        .count()
                        .clamp(1, 4) as u32,
                    controls,
                }
            })
            .collect(),
    }])
}
