use crate::{
    mixer_routes::{self, Route},
    model::ViewProject,
};
use oxitone_core::wire::EffectRef;

pub const STRIP_WIDTH: f32 = 92.;
#[derive(Clone)]
pub struct EffectSlot {
    pub instance: Option<String>,
    pub name: String,
    pub bypass: bool,
}
pub struct Strip {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub instrument: bool,
    pub index: usize,
    pub color_index: usize,
    pub level: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    pub effects: Vec<EffectSlot>,
    pub outputs: Vec<Route>,
    pub inputs: Vec<Route>,
}
impl Strip {
    pub fn sends(&self) -> impl Iterator<Item = &Route> {
        self.outputs.iter().filter(|r| r.is_send())
    }
    pub fn output(&self) -> Option<&Route> {
        self.outputs.iter().find(|r| !r.is_send())
    }
    pub fn relation(&self, id: &str) -> Option<&'static str> {
        if self.outputs.iter().any(|r| r.destination == id) {
            Some("OUT")
        } else if self.inputs.iter().any(|r| r.source == id) {
            Some("IN")
        } else {
            None
        }
    }
}
pub fn strips(project: &ViewProject) -> &[Strip] {
    project.mixer_strips.get_or_init(|| build(project))
}
fn build(project: &ViewProject) -> Vec<Strip> {
    let s = &project.snapshot;
    let routes = mixer_routes::connections(s);
    let effects = |chain: &[EffectRef]| {
        chain
            .iter()
            .map(|e| EffectSlot {
                instance: e.instance_id.clone(),
                name: plugin_name(&e.plugin_id),
                bypass: e.bypass.unwrap_or(false),
            })
            .collect()
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
            instrument: true,
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
            effects: effects(&c.effect_chain),
            outputs: vec![],
            inputs: vec![],
        })
        .collect();
    for (i, b) in s.mixer_channels.iter().enumerate() {
        result.push(Strip {
            id: b.id.clone(),
            name: mixer_routes::name(s, &b.id),
            kind: if b.id == "mix_master" {
                "Stereo output"
            } else {
                "Mix bus"
            }
            .into(),
            instrument: false,
            index: s.channels.len() + i + 1,
            color_index: s.tracks.len() + i,
            level: b.level,
            pan: b.balance,
            mute: b.mute.unwrap_or(false),
            solo: b.solo.unwrap_or(false),
            effects: effects(&b.inserts),
            outputs: vec![],
            inputs: vec![],
        });
    }
    if !result.iter().any(|s| s.id == "mix_master") {
        result.push(Strip {
            id: "mix_master".into(),
            name: "Master".into(),
            kind: "Stereo output".into(),
            instrument: false,
            index: 0,
            color_index: 0,
            level: 1.,
            pan: 0.,
            mute: false,
            solo: false,
            effects: vec![],
            outputs: vec![],
            inputs: vec![],
        });
    }
    for strip in &mut result {
        strip.outputs = routes
            .iter()
            .filter(|r| r.source == strip.id)
            .cloned()
            .collect();
        strip.inputs = routes
            .iter()
            .filter(|r| r.destination == strip.id)
            .cloned()
            .collect();
    }
    result
}
pub fn plugin_name(id: &str) -> String {
    if id == "oxitone.eq" {
        return "EQ".into();
    }
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
