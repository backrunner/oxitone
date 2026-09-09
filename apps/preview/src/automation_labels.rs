//! Musical target names shared by the curve editor, Browser and Playlist.
use crate::model::ViewProject;
use oxitone_core::wire::{AutomationLaneSpec, EffectRef, ParameterScope};
use oxitone_graph::{builtin_params, insert_params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetLabel {
    pub key: String,
    pub owner: String,
    pub device: Option<String>,
    pub parameter: String,
}
impl TargetLabel {
    pub fn context(&self) -> String {
        self.device.as_ref().map_or_else(
            || self.owner.clone(),
            |device| format!("{} / {device}", self.owner),
        )
    }
}

impl ViewProject {
    pub fn automation_label(&self, lane: &AutomationLaneSpec) -> String {
        let label = self.automation_target(lane);
        format!("{} — {}", label.parameter, label.context())
    }

    pub fn automation_target(&self, lane: &AutomationLaneSpec) -> TargetLabel {
        let target = &lane.target;
        let entity = target.entity_id.as_str();
        let parameter = target.parameter_id.as_str();
        for (index, channel) in self.snapshot.channels.iter().enumerate() {
            let owner = channel
                .name
                .clone()
                .unwrap_or_else(|| format!("Channel {}", index + 1));
            let instrument = &channel.instrument;
            if instrument.instance_id.as_deref() == Some(entity)
                || (channel.id == entity
                    && builtin_params::find_parameter(
                        builtin_params::BuiltinEntityKind::Channel,
                        parameter,
                    )
                    .is_none()
                    && insert_params::parse(parameter).is_none())
            {
                return TargetLabel {
                    key: format!("instrument:{}", channel.id),
                    owner,
                    device: Some(crate::mixer_model::plugin_name(&instrument.plugin_id)),
                    parameter: self.plugin_parameter_label(
                        &instrument.plugin_id,
                        &instrument.plugin_version,
                        parameter,
                    ),
                };
            }
            if let Some(label) =
                self.effect_target(lane, &channel.id, &owner, &channel.effect_chain)
            {
                return label;
            }
            if channel.id == entity {
                return base(entity, owner, parameter);
            }
        }
        for (index, bus) in self.snapshot.mixer_channels.iter().enumerate() {
            let owner = bus.name.clone().unwrap_or_else(|| {
                if bus.id == "mix_master" {
                    "Master".into()
                } else {
                    format!("Bus {}", index + 1)
                }
            });
            if let Some(label) = self.effect_target(lane, &bus.id, &owner, &bus.inserts) {
                return label;
            }
            if bus.id == entity {
                let mut label = base(entity, owner, parameter);
                if let Some(destination) = builtin_params::parse_send_ratio_parameter(parameter) {
                    let name = self
                        .snapshot
                        .mixer_channels
                        .iter()
                        .position(|b| b.id == destination)
                        .map(|i| {
                            self.snapshot.mixer_channels[i]
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("Bus {}", i + 1))
                        })
                        .unwrap_or_else(|| "Master".into());
                    label.parameter = format!("Send to {name}");
                }
                return label;
            }
        }
        if let Some(index) = self
            .snapshot
            .sample_clips
            .iter()
            .position(|clip| clip.id == entity)
        {
            return base(entity, format!("Audio clip {}", index + 1), parameter);
        }
        base(
            entity,
            if entity == self.snapshot.id {
                "Project"
            } else if entity == "mix_master" {
                "Master"
            } else {
                "Automation"
            }
            .into(),
            parameter,
        )
    }

    fn effect_target(
        &self,
        lane: &AutomationLaneSpec,
        owner_id: &str,
        owner: &str,
        effects: &[EffectRef],
    ) -> Option<TargetLabel> {
        let target = &lane.target;
        let (index, parameter, host) = if target.entity_id == owner_id {
            let (index, param) = insert_params::parse(&target.parameter_id)?;
            match param {
                insert_params::InsertParameter::Mix => (index, "mix", true),
                insert_params::InsertParameter::Bypass => (index, "bypass", true),
                insert_params::InsertParameter::Plugin(id) => (index, id, false),
            }
        } else {
            (
                effects
                    .iter()
                    .position(|effect| effect.instance_id.as_ref() == Some(&target.entity_id))?,
                target.parameter_id.as_str(),
                target.scope == Some(ParameterScope::EffectHost),
            )
        };
        let effect = effects.get(index)?;
        Some(TargetLabel {
            key: effect
                .instance_id
                .clone()
                .unwrap_or_else(|| format!("insert:{owner_id}:{index}")),
            owner: owner.into(),
            device: Some(format!(
                "{} {}",
                index + 1,
                crate::mixer_model::plugin_name(&effect.plugin_id)
            )),
            parameter: if host {
                match parameter {
                    "mix" => "Dry / wet".into(),
                    "bypass" => "Bypass".into(),
                    _ => parameter.into(),
                }
            } else {
                self.plugin_parameter_label(&effect.plugin_id, &effect.plugin_version, parameter)
            },
        })
    }

    fn plugin_parameter_label(&self, id: &str, version: &str, parameter: &str) -> String {
        self.plugins
            .get(&(id.into(), version.into()))
            .and_then(|info| {
                info.descriptor
                    .parameters
                    .iter()
                    .find(|spec| spec.id == parameter)
            })
            .map_or_else(|| parameter.into(), |spec| spec.label.clone())
    }
}

fn base(key: &str, owner: String, parameter: &str) -> TargetLabel {
    TargetLabel {
        key: key.into(),
        owner,
        device: None,
        parameter: match parameter {
            "level" => "Volume",
            "pan" => "Pan",
            "balance" => "Balance",
            "mute" => "Mute",
            "swing" => "Swing",
            "tempo" => "Tempo",
            "masterSendRatio" => "Master send",
            "gain" => "Gain",
            "tone" => "Tone",
            "rate" => "Playback rate",
            other => other,
        }
        .into(),
    }
}
