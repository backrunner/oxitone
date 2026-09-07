//! Plugin reference and parameter-table validation against the registry
//! (04-api-contracts.md: unknown parameters/versions are compile errors;
//! missing parameters fall back to descriptor defaults).

use std::collections::BTreeMap;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EffectRef, InstrumentRef, ProjectSnapshot};

use crate::descriptor::{PluginDescriptor, PluginKind};
use crate::registry::PluginRegistry;

fn resolve<'a>(
    registry: &'a PluginRegistry,
    path: &str,
    plugin_id: &str,
    plugin_version: &str,
) -> Result<&'a PluginDescriptor, OxitoneError> {
    registry
        .lookup_descriptor(plugin_id, plugin_version)
        .ok_or_else(|| {
            let message = if registry.contains_id(plugin_id) {
                format!("plugin {plugin_id:?} has no registered version {plugin_version:?}")
            } else {
                format!("unknown plugin {plugin_id:?}")
            };
            OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
        })
}

fn check_parameters(
    path: &str,
    descriptor: &PluginDescriptor,
    parameters: &BTreeMap<String, f64>,
) -> Result<(), OxitoneError> {
    for (id, value) in parameters {
        let param_path = format!("{path}.parameters.{id}");
        let spec = descriptor
            .parameters
            .iter()
            .find(|spec| &spec.id == id)
            .ok_or_else(|| {
                OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!(
                        "parameter {id:?} is not declared by plugin {:?}",
                        descriptor.plugin_id
                    ),
                    &param_path,
                )
            })?;
        if !value.is_finite() || *value < spec.min || *value > spec.max {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!(
                    "parameter {id:?} must be finite within {}..={}, got {value}",
                    spec.min, spec.max
                ),
                param_path,
            ));
        }
    }
    Ok(())
}

fn check_instrument<'a>(
    registry: &'a PluginRegistry,
    path: &str,
    instrument: &InstrumentRef,
) -> Result<&'a PluginDescriptor, OxitoneError> {
    let descriptor = resolve(
        registry,
        path,
        &instrument.plugin_id,
        &instrument.plugin_version,
    )?;
    if descriptor.kind != PluginKind::Instrument {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("plugin {:?} is not an instrument", instrument.plugin_id),
            path,
        ));
    }
    check_parameters(path, descriptor, &instrument.parameters)?;
    Ok(descriptor)
}

fn check_effect(
    registry: &PluginRegistry,
    snapshot: &ProjectSnapshot,
    path: &str,
    effect: &EffectRef,
) -> Result<(), OxitoneError> {
    let descriptor = resolve(registry, path, &effect.plugin_id, &effect.plugin_version)?;
    if descriptor.kind != PluginKind::Effect {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("plugin {:?} is not an effect", effect.plugin_id),
            path,
        ));
    }
    if effect.plugin_id == "oxitone.convolver" {
        if let Some(resources) = &effect.resources {
            let id = resources.get("impulse").filter(|_| resources.len() == 1);
            if !id.is_some_and(|id| snapshot.samples.iter().any(|sample| &sample.id == id)) {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "convolver requires one impulse referencing a project sample",
                    format!("{path}.resources.impulse"),
                ));
            }
        }
    }
    check_parameters(path, descriptor, &effect.parameters)
}

pub(super) fn validate_plugins(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
) -> Result<(), OxitoneError> {
    for (i, channel) in snapshot.channels.iter().enumerate() {
        let path = format!("$.channels[{i}]");
        check_instrument(registry, &format!("{path}.instrument"), &channel.instrument)?;
        for (e, effect) in channel.effect_chain.iter().enumerate() {
            check_effect(
                registry,
                snapshot,
                &format!("{path}.effectChain[{e}]"),
                effect,
            )?;
        }
    }
    for (i, bus) in snapshot.mixer_channels.iter().enumerate() {
        for (e, insert) in bus.inserts.iter().enumerate() {
            check_effect(
                registry,
                snapshot,
                &format!("$.mixerChannels[{i}].inserts[{e}]"),
                insert,
            )?;
        }
    }
    Ok(())
}
