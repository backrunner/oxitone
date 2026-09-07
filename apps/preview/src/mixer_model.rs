use crate::model::ViewProject;

pub struct Strip {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub index: usize,
    pub color_index: usize,
    pub level: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    pub effects: Vec<(String, bool)>,
    pub routes: Vec<String>,
}
pub fn strips(project: &ViewProject) -> Vec<Strip> {
    let s = &project.snapshot;
    let bus_name = |id: &str| {
        s.mixer_channels
            .iter()
            .find(|b| b.id == id)
            .and_then(|b| b.name.clone())
            .unwrap_or_else(|| {
                if id == "mix_master" {
                    "Master".into()
                } else {
                    id.into()
                }
            })
    };
    let mut result: Vec<_> = s
        .channels
        .iter()
        .enumerate()
        .map(|(i, c)| Strip {
            id: c.id.clone(),
            name: c
                .name
                .clone()
                .unwrap_or_else(|| format!("Channel {}", i + 1)),
            kind: plugin_name(&c.instrument.plugin_id),
            index: i + 1,
            color_index: s
                .tracks
                .iter()
                .position(|t| t.channel_ids.contains(&c.id))
                .unwrap_or(i),
            level: c.level,
            pan: c.pan,
            mute: c.mute.unwrap_or(false),
            solo: c.solo.unwrap_or(false),
            effects: c
                .effect_chain
                .iter()
                .map(|e| (plugin_name(&e.plugin_id), e.bypass.unwrap_or(false)))
                .collect(),
            routes: vec![format!("→ {}", bus_name(&c.mixer_channel_id))],
        })
        .collect();
    for (i, b) in s.mixer_channels.iter().enumerate() {
        result.push(Strip {
            id: b.id.clone(),
            name: bus_name(&b.id),
            kind: "Mix bus".into(),
            index: s.channels.len() + i + 1,
            color_index: s.tracks.len() + i,
            level: b.level,
            pan: b.balance,
            mute: b.mute.unwrap_or(false),
            solo: b.solo.unwrap_or(false),
            effects: b
                .inserts
                .iter()
                .map(|e| (plugin_name(&e.plugin_id), e.bypass.unwrap_or(false)))
                .collect(),
            routes: b
                .sends
                .iter()
                .map(|send| {
                    format!(
                        "{} {} · {:.0}%{}",
                        if send.sidechain == Some(true) {
                            "SC →"
                        } else {
                            "→"
                        },
                        bus_name(&send.destination_id),
                        send.ratio * 100.,
                        if send.pre_fader == Some(true) {
                            " pre"
                        } else {
                            ""
                        }
                    )
                })
                .chain((b.id != "mix_master").then(|| {
                    format!(
                        "→ Master · {:.0}%",
                        b.master_send_ratio.unwrap_or(1.) * 100.
                    )
                }))
                .collect(),
        });
    }
    if !result.iter().any(|s| s.id == "mix_master") {
        result.push(Strip {
            id: "mix_master".into(),
            name: "Master".into(),
            kind: "Stereo output".into(),
            index: 0,
            color_index: 0,
            level: 1.,
            pan: 0.,
            mute: false,
            solo: false,
            effects: vec![],
            routes: vec![],
        });
    }
    result
}
pub fn plugin_name(id: &str) -> String {
    id.rsplit('.')
        .next()
        .unwrap_or(id)
        .split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn db(value: f32) -> String {
    if value <= 0.000001 {
        "−∞".into()
    } else {
        format!("{:.1}", 20. * value.log10())
    }
}
