//! Insert parameter tables are resolved off-thread; application uses only indices.

use crate::{
    bindings::RtTarget,
    channel::ChannelNode,
    graph::{MixerBeatParam, RenderGraph},
};
use oxitone_core::{codes, wire::ParameterSpec, OxitoneError};
use oxitone_graph::{
    builtin_params::{insert_parameter, InsertParam},
    insert_params::{parse, InsertParameter},
};
use oxitone_mixer::MixerEngine;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Copy)]
pub enum EffectOwner {
    Channel(usize),
    Mixer(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum EffectParameter {
    Mix,
    Bypass,
    Plugin(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct EffectTarget {
    pub owner: EffectOwner,
    pub insert: usize,
    pub parameter: EffectParameter,
}

struct Slot {
    owner: EffectOwner,
    specs: Arc<Vec<ParameterSpec>>,
}

#[derive(Clone, Default)]
pub(crate) struct EffectTargetIndex(Arc<BTreeMap<String, Vec<Slot>>>);

impl EffectTargetIndex {
    pub fn from_graph(graph: &RenderGraph) -> Self {
        let mut entries = BTreeMap::new();
        for (channel, node) in graph.channels.iter().enumerate() {
            entries.insert(
                node.id.clone(),
                node.inserts
                    .iter()
                    .map(|insert| Slot {
                        owner: EffectOwner::Channel(channel),
                        specs: insert.specs.clone(),
                    })
                    .collect(),
            );
        }
        for (bus, id) in graph.mixer.bus_order().into_iter().enumerate() {
            entries.insert(
                id,
                graph
                    .mixer
                    .insert_specs(bus)
                    .into_iter()
                    .map(|specs| Slot {
                        owner: EffectOwner::Mixer(bus),
                        specs,
                    })
                    .collect(),
            );
        }
        Self(Arc::new(entries))
    }

    pub fn resolve(
        &self,
        entity: &str,
        id: &str,
    ) -> Result<(RtTarget, ParameterSpec), OxitoneError> {
        let invalid = || {
            OxitoneError::new(codes::AUTOMATION_TARGET_INVALID,
            format!("invalid insert target {entity:?}.{id:?}; plugin parameters require automation: true"))
        };
        let (insert, parameter) = parse(id).ok_or_else(invalid)?;
        let slot = self
            .0
            .get(entity)
            .and_then(|slots| slots.get(insert))
            .ok_or_else(invalid)?;
        let (parameter, spec) = match parameter {
            InsertParameter::Mix => (EffectParameter::Mix, insert_parameter(InsertParam::Mix)),
            InsertParameter::Bypass => (
                EffectParameter::Bypass,
                insert_parameter(InsertParam::Bypass),
            ),
            InsertParameter::Plugin(id) => {
                let (index, spec) = slot
                    .specs
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.id == id && p.automation == Some(true))
                    .ok_or_else(invalid)?;
                (EffectParameter::Plugin(index), spec.clone())
            }
        };
        Ok((
            RtTarget::EffectInsert(EffectTarget {
                owner: slot.owner,
                insert,
                parameter,
            }),
            spec,
        ))
    }
}

pub(crate) fn apply(
    channels: &mut [ChannelNode],
    mixer: &mut MixerEngine,
    beats: &mut [MixerBeatParam],
    target: EffectTarget,
    value: f64,
) {
    match target.owner {
        EffectOwner::Channel(channel) => {
            let node = &mut channels[channel].inserts[target.insert];
            match target.parameter {
                EffectParameter::Mix => node.mix.set_target(value as f32),
                EffectParameter::Bypass => node.bypass = value >= 0.5,
                EffectParameter::Plugin(index) => node.set_parameter(index, value),
            }
        }
        EffectOwner::Mixer(bus) => match target.parameter {
            EffectParameter::Mix => mixer.set_insert_mix_at(bus, target.insert, value),
            EffectParameter::Bypass => mixer.set_insert_bypass_at(bus, target.insert, value >= 0.5),
            EffectParameter::Plugin(index) => {
                for beat in beats {
                    if beat.bus_index != bus || beat.insert != target.insert {
                        continue;
                    }
                    if beat.state.parameter_index == index {
                        beat.state.beats = value;
                        beat.state.active = true;
                        return;
                    }
                    if beat.state.seconds_index == index {
                        beat.state.active = false;
                        beat.state.last_sent = f64::NAN;
                    }
                }
                mixer.set_insert_parameter_at(bus, target.insert, index, value);
            }
        },
    }
}
