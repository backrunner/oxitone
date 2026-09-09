//! Pure slot resolution against accepted source data. No runtime plugin access.
use crate::{model::ViewProject, plugin_catalog::PluginInfo};
use oxitone_core::wire::{ParameterSpec, ProjectSnapshot};
use oxitone_graph::builtin_params::{
    find_parameter, insert_parameter, BuiltinEntityKind, InsertParam,
};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DetailTarget {
    Instrument(String),
    ChannelInsert(String, usize),
    BusInsert(String, usize),
}
impl DetailTarget {
    pub fn owner(&self) -> &str {
        match self {
            Self::Instrument(id) | Self::ChannelInsert(id, _) | Self::BusInsert(id, _) => id,
        }
    }
    pub fn slot(&self) -> Option<usize> {
        match self {
            Self::Instrument(_) => None,
            Self::ChannelInsert(_, i) | Self::BusInsert(_, i) => Some(*i),
        }
    }
    pub fn label(&self) -> String {
        self.slot()
            .map_or_else(|| "Instrument".into(), |i| format!("Insert {:02}", i + 1))
    }
    pub fn parameter_path(&self, id: &str) -> String {
        self.slot()
            .map_or_else(|| id.to_owned(), |i| format!("insert.{i}.parameter.{id}"))
    }
}

#[derive(Clone)]
pub struct ParameterDetail {
    pub spec: ParameterSpec,
    pub value: f64,
    pub explicit: bool,
    pub host: bool,
    pub automation: Vec<String>,
}
impl ParameterDetail {
    pub fn fraction(&self) -> f32 {
        let span = self.spec.max - self.spec.min;
        if span <= 0. {
            return 0.;
        }
        if self.spec.mapping == Some(oxitone_core::wire::ParameterMapping::Log)
            && self.spec.min > 0.
            && self.value > 0.
        {
            return ((self.value / self.spec.min).ln() / (self.spec.max / self.spec.min).ln())
                .clamp(0., 1.) as f32;
        }
        ((self.value - self.spec.min) / span).clamp(0., 1.) as f32
    }
}

pub struct PluginDetails {
    pub target: DetailTarget,
    pub owner_name: String,
    pub name: String,
    pub info: PluginInfo,
    pub parameters: Vec<ParameterDetail>,
    pub settings: Vec<(String, String)>,
    pub resources: Vec<(String, String)>,
    pub state: Option<Value>,
    pub source: Value,
}

pub fn resolve(project: &ViewProject, target: &DetailTarget) -> Option<PluginDetails> {
    let snapshot = &project.snapshot;
    let mut settings = Vec::new();
    let (owner_name, id, version, parameters, resources, state, source, host) = match target {
        DetailTarget::Instrument(owner) => {
            let channel = snapshot.channels.iter().find(|c| c.id == *owner)?;
            let instrument = &channel.instrument;
            settings.extend([
                (
                    "Channel level".into(),
                    crate::mixer_model::db(channel.level as f32) + " dB",
                ),
                ("Pan".into(), crate::parameter_format::number(channel.pan)),
                (
                    "Swing".into(),
                    crate::parameter_format::number(channel.swing.unwrap_or(0.)),
                ),
                (
                    "Mute / Solo".into(),
                    format!(
                        "{} / {}",
                        channel.mute.unwrap_or(false),
                        channel.solo.unwrap_or(false)
                    ),
                ),
                (
                    "Output bus".into(),
                    bus_name(snapshot, &channel.mixer_channel_id),
                ),
            ]);
            (
                channel.name.clone().unwrap_or_else(|| owner.clone()),
                &instrument.plugin_id,
                &instrument.plugin_version,
                &instrument.parameters,
                &instrument.resources,
                instrument.state.clone(),
                serde_json::to_value(instrument).ok()?,
                None,
            )
        }
        DetailTarget::ChannelInsert(owner, index) | DetailTarget::BusInsert(owner, index) => {
            let (owner_name, effect) = match target {
                DetailTarget::ChannelInsert(..) => {
                    let channel = snapshot.channels.iter().find(|c| c.id == *owner)?;
                    (
                        channel.name.clone().unwrap_or_else(|| owner.clone()),
                        channel.effect_chain.get(*index)?,
                    )
                }
                _ => {
                    let bus = snapshot.mixer_channels.iter().find(|b| b.id == *owner)?;
                    (bus_name(snapshot, owner), bus.inserts.get(*index)?)
                }
            };
            settings.push((
                "Slot".into(),
                format!(
                    "{} · {}",
                    target.label(),
                    if effect.bypass.unwrap_or(false) {
                        "Bypassed"
                    } else {
                        "Active"
                    }
                ),
            ));
            (
                owner_name,
                &effect.plugin_id,
                &effect.plugin_version,
                &effect.parameters,
                &effect.resources,
                None,
                serde_json::to_value(effect).ok()?,
                Some((effect.mix, effect.bypass)),
            )
        }
    };
    let info = project.plugins.get(&(id.clone(), version.clone()))?.clone();
    let mut rows = Vec::new();
    if let Some((mix, bypass)) = host {
        for (kind, value) in [
            (InsertParam::Mix, mix),
            (InsertParam::Bypass, bypass.map(|v| if v { 1. } else { 0. })),
        ] {
            let spec = insert_parameter(kind);
            let path = format!("insert.{}.{}", target.slot().unwrap(), spec.id);
            rows.push(parameter(
                snapshot,
                target.owner(),
                spec,
                value,
                &path,
                true,
            ));
        }
    }
    for spec in &info.descriptor.parameters {
        let path = target.parameter_path(&spec.id);
        let mut row = parameter(
            snapshot,
            target.owner(),
            spec.clone(),
            parameters.get(&spec.id).copied(),
            &path,
            false,
        );
        // Channel built-ins take precedence over identically named instrument parameters.
        if target.slot().is_none() && find_parameter(BuiltinEntityKind::Channel, &spec.id).is_some()
        {
            row.automation.clear();
        }
        rows.push(row);
    }
    let mut resources: Vec<_> = resources
        .iter()
        .flat_map(|r| r.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(sample_id) = state
        .as_ref()
        .and_then(|s| s.get("sampleId"))
        .and_then(Value::as_str)
    {
        resources.push(("state.sampleId".into(), sample_id.into()));
    }
    Some(PluginDetails {
        target: target.clone(),
        owner_name,
        name: crate::mixer_model::plugin_name(id),
        info,
        parameters: rows,
        settings,
        resources,
        state,
        source,
    })
}

fn parameter(
    snapshot: &ProjectSnapshot,
    owner: &str,
    spec: ParameterSpec,
    value: Option<f64>,
    path: &str,
    host: bool,
) -> ParameterDetail {
    let automation = snapshot
        .automation
        .iter()
        .filter(|lane| lane.target.entity_id == owner && lane.target.parameter_id == path)
        .map(|lane| lane.id.clone())
        .collect();
    ParameterDetail {
        value: value.unwrap_or(spec.default),
        explicit: value.is_some(),
        spec,
        host,
        automation,
    }
}
fn bus_name(snapshot: &ProjectSnapshot, id: &str) -> String {
    snapshot
        .mixer_channels
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
}
