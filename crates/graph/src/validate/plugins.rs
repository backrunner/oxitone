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
    instance_id: Option<&str>,
) -> Result<&'a PluginDescriptor, OxitoneError> {
    registry
        .instance_descriptor(plugin_id, plugin_version, instance_id)
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
        instrument.instance_id.as_deref(),
    )?;
    if descriptor.kind != PluginKind::Instrument {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("plugin {:?} is not an instrument", instrument.plugin_id),
            path,
        ));
    }
    check_vst3_resources(path, &instrument.plugin_id, instrument.resources.as_ref())?;
    check_parameters(path, descriptor, &instrument.parameters)?;
    Ok(descriptor)
}

fn check_effect(
    registry: &PluginRegistry,
    snapshot: &ProjectSnapshot,
    path: &str,
    effect: &EffectRef,
) -> Result<(), OxitoneError> {
    let descriptor = resolve(
        registry,
        path,
        &effect.plugin_id,
        &effect.plugin_version,
        effect.instance_id.as_deref(),
    )?;
    if descriptor.kind != PluginKind::Effect {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("plugin {:?} is not an effect", effect.plugin_id),
            path,
        ));
    }
    if effect.state.is_some() && descriptor.state_schema.is_none() {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "effect declares no state schema and cannot carry state",
            format!("{path}.state"),
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
    check_vst3_resources(path, &effect.plugin_id, effect.resources.as_ref())?;
    check_parameters(path, descriptor, &effect.parameters)
}

fn check_vst3_resources(
    path: &str,
    id: &str,
    resources: Option<&BTreeMap<String, String>>,
) -> Result<(), OxitoneError> {
    if id.starts_with("vst3.") && resources.is_some_and(|r| !r.is_empty()) {
        return Err(OxitoneError::with_path(
            "PluginCapabilityUnsupported",
            "VST3 project sample bindings are unsupported; use the plugin's persisted configuration",
            format!("{path}.resources"),
        ));
    }
    Ok(())
}

pub(super) fn validate_plugins(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
) -> Result<(), OxitoneError> {
    for (i, channel) in snapshot.channels.iter().enumerate() {
        let path = format!("$.channels[{i}]");
        check_instrument(registry, &format!("{path}.instrument"), &channel.instrument)?;
        let plugin = registry
            .lookup_instance(
                &channel.instrument.plugin_id,
                &channel.instrument.plugin_version,
                channel.instrument.instance_id.as_deref(),
            )
            .expect("validated instrument");
        for (index, destination) in channel.output_routes.iter().flatten() {
            let bus = index.parse::<usize>().ok().filter(|bus| {
                (1..=15).contains(bus)
                    && bus.to_string() == *index
                    && *bus < plugin.output_bus_count()
            });
            if bus.is_none() || !snapshot.mixer_channels.iter().any(|b| b.id == *destination) {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "invalid instrument output bus or mixer destination",
                    format!("{path}.outputRoutes.{index}"),
                ));
            }
        }
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
