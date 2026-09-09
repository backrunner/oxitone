//! Resolve typed plugin targets once on the control side. No source indices are persisted in bindings.
use crate::{compile::BindingTarget, registry::PluginRegistry};
use oxitone_core::{
    error::{codes, OxitoneError},
    wire::{ParameterScope, ParameterSpec, ProjectSnapshot},
};

pub fn resolve(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    instance: &str,
    scope: ParameterScope,
    parameter: &str,
) -> Result<(BindingTarget, ParameterSpec), OxitoneError> {
    for channel in &snapshot.channels {
        if channel.instrument.instance_id.as_deref() == Some(instance) {
            if scope != ParameterScope::Plugin {
                return Err(invalid("instrument has no insert host parameters"));
            }
            let descriptor = registry
                .lookup_descriptor(
                    &channel.instrument.plugin_id,
                    &channel.instrument.plugin_version,
                )
                .ok_or_else(|| invalid("missing instrument definition"))?;
            let spec = descriptor
                .parameters
                .iter()
                .find(|spec| spec.id == parameter)
                .cloned()
                .ok_or_else(|| invalid("instrument parameter is not declared"))?;
            let index = snapshot
                .channels
                .iter()
                .filter(|other| other.id < channel.id)
                .count();
            return Ok((
                BindingTarget::InstrumentParam {
                    channel: index,
                    parameter_id: parameter.into(),
                },
                spec,
            ));
        }
    }
    let chains = snapshot
        .channels
        .iter()
        .map(|channel| (&channel.id, &channel.effect_chain))
        .chain(
            snapshot
                .mixer_channels
                .iter()
                .map(|bus| (&bus.id, &bus.inserts)),
        );
    for (owner, effects) in chains {
        if let Some(index) = effects
            .iter()
            .position(|effect| effect.instance_id.as_deref() == Some(instance))
        {
            let address = match scope {
                ParameterScope::Plugin => format!("insert.{index}.parameter.{parameter}"),
                ParameterScope::EffectHost if parameter == "mix" || parameter == "bypass" => {
                    format!("insert.{index}.{parameter}")
                }
                _ => return Err(invalid("effect host parameter must be mix or bypass")),
            };
            let spec = crate::insert_params::resolve(effects, registry, &address)
                .ok_or_else(|| invalid("effect parameter is not declared"))?;
            return Ok((
                BindingTarget::EffectInsert {
                    entity_id: owner.clone(),
                    parameter_id: address,
                },
                spec,
            ));
        }
    }
    Err(invalid(
        "plugin instance is missing or belongs to another project",
    ))
}
fn invalid(message: &str) -> OxitoneError {
    OxitoneError::new(codes::AUTOMATION_TARGET_INVALID, message)
}
