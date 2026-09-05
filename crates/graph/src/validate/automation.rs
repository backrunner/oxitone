//! Automation target validation (02-domain-spec.md §Automation,
//! 04-api-contracts.md §内建参数全集). A target must name an existing entity
//! and a parameter that exists and declares `automation: true`; Sample
//! structured-edit fields are explicitly not automatable. Tempo lanes get the
//! conflict and transport-invariance rules.

use std::collections::BTreeMap;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{AutomationSourceSpec, ParameterSpec, ProjectSnapshot};

use crate::builtin_params::{find_parameter, BuiltinEntityKind, NON_AUTOMATABLE_SAMPLE_FIELDS};
use crate::descriptor::PluginKind;
use crate::registry::PluginRegistry;
use crate::topology::MASTER_MIXER_CHANNEL_ID;

fn target_invalid(path: &str, message: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::AUTOMATION_TARGET_INVALID, message, path)
}

fn is_tempo_target(snapshot: &ProjectSnapshot, entity_id: &str, parameter_id: &str) -> bool {
    entity_id == snapshot.id && parameter_id == "tempo"
}

/// Resolve a lane target to its parameter spec.
fn resolve_target(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    path: &str,
    entity_id: &str,
    parameter_id: &str,
) -> Result<ParameterSpec, OxitoneError> {
    if entity_id == snapshot.id {
        return find_parameter(BuiltinEntityKind::Project, parameter_id).ok_or_else(|| {
            target_invalid(path, format!("project has no parameter {parameter_id:?}"))
        });
    }
    if let Some(channel) = snapshot.channels.iter().find(|c| c.id == entity_id) {
        if let Some(spec) = find_parameter(BuiltinEntityKind::Channel, parameter_id) {
            return Ok(spec);
        }
        if let Some((index, param)) = crate::builtin_params::parse_insert_parameter(parameter_id) {
            if index < channel.effect_chain.len() {
                return Ok(crate::builtin_params::insert_parameter(param));
            }
            return Err(target_invalid(
                path,
                format!("channel has no insert #{index} for parameter {parameter_id:?}"),
            ));
        }
        let descriptor = registry
            .lookup_descriptor(
                &channel.instrument.plugin_id,
                &channel.instrument.plugin_version,
            )
            .expect("instrument references are validated before automation");
        debug_assert_eq!(descriptor.kind, PluginKind::Instrument);
        return descriptor
            .parameters
            .iter()
            .find(|spec| spec.id == parameter_id)
            .cloned()
            .ok_or_else(|| {
                target_invalid(
                    path,
                    format!("channel instrument declares no parameter {parameter_id:?}"),
                )
            });
    }
    if let Some(bus) = snapshot.mixer_channels.iter().find(|m| m.id == entity_id) {
        let spec =
            find_parameter(BuiltinEntityKind::MixerChannel, parameter_id).ok_or_else(|| {
                target_invalid(
                    path,
                    format!("mixer channel has no parameter {parameter_id:?}"),
                )
            })?;
        if let Some(destination) = crate::builtin_params::parse_send_ratio_parameter(parameter_id) {
            let known = snapshot
                .mixer_channels
                .iter()
                .any(|m| m.id == destination && m.id != MASTER_MIXER_CHANNEL_ID)
                && bus.sends.iter().any(|s| s.destination_id == destination);
            if !known {
                return Err(target_invalid(
                    path,
                    format!("send ratio target {parameter_id:?} names no send of this bus"),
                ));
            }
        }
        return Ok(spec);
    }
    if snapshot.sample_clips.iter().any(|c| c.id == entity_id) {
        return find_parameter(BuiltinEntityKind::SampleClip, parameter_id).ok_or_else(|| {
            target_invalid(
                path,
                format!("sample clip has no parameter {parameter_id:?}"),
            )
        });
    }
    if snapshot.samples.iter().any(|s| s.id == entity_id) {
        if NON_AUTOMATABLE_SAMPLE_FIELDS.contains(&parameter_id) {
            return Err(target_invalid(
                path,
                format!(
                    "sample field {parameter_id:?} is a structured edit baked at prepare and is not automatable"
                ),
            ));
        }
        return Err(target_invalid(
            path,
            format!("sample has no parameter {parameter_id:?}"),
        ));
    }
    Err(target_invalid(
        path,
        format!("unknown automation target entity {entity_id:?}"),
    ))
}

/// Tempo lane sources must be transport-invariant: no `chance` and no
/// play/loop-dependent `restart` semantics (02-domain-spec.md §Project).
fn check_tempo_source(path: &str, source: &AutomationSourceSpec) -> Result<(), OxitoneError> {
    let restricted = match source {
        AutomationSourceSpec::Chance(_) => true,
        AutomationSourceSpec::Map { input, .. } | AutomationSourceSpec::Unary { input, .. } => {
            return check_tempo_source(path, input)
        }
        AutomationSourceSpec::Binary { left, right, .. } => {
            check_tempo_source(path, left)?;
            return check_tempo_source(path, right);
        }
        _ => false,
    };
    if restricted {
        return Err(OxitoneError::with_path(
            codes::AUTOMATION_TEMPO_RESTRICTION,
            "tempo lane sources must be transport-invariant (no chance/restart)",
            path,
        ));
    }
    Ok(())
}

pub(super) fn validate_automation(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
) -> Result<(), OxitoneError> {
    let mut lanes_per_target: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for (i, lane) in snapshot.automation.iter().enumerate() {
        let path = format!("$.automation[{i}].target");
        lane.source.validate().map_err(|mut err| {
            err.path = Some(format!("$.automation[{i}].source"));
            err
        })?;
        let spec = resolve_target(
            snapshot,
            registry,
            &path,
            &lane.target.entity_id,
            &lane.target.parameter_id,
        )?;
        if spec.automation != Some(true) {
            return Err(target_invalid(
                &format!("{path}.parameterId"),
                format!(
                    "parameter {:?} does not declare automation: true",
                    lane.target.parameter_id
                ),
            ));
        }
        lanes_per_target
            .entry((
                lane.target.entity_id.as_str(),
                lane.target.parameter_id.as_str(),
            ))
            .or_default()
            .push(i);
    }

    let tempo_lanes: Vec<usize> = snapshot
        .automation
        .iter()
        .enumerate()
        .filter(|(_, lane)| {
            is_tempo_target(snapshot, &lane.target.entity_id, &lane.target.parameter_id)
        })
        .map(|(i, _)| i)
        .collect();
    if tempo_lanes.len() > 1 {
        return Err(OxitoneError::with_path(
            codes::TEMPO_AUTOMATION_CONFLICT,
            "at most one tempo automation lane is allowed",
            format!("$.automation[{}]", tempo_lanes[1]),
        ));
    }
    for i in tempo_lanes {
        check_tempo_source(
            &format!("$.automation[{i}].source"),
            &snapshot.automation[i].source,
        )?;
    }

    for ((entity, parameter), lanes) in &lanes_per_target {
        if lanes.len() > 1
            && lanes
                .iter()
                .any(|i| snapshot.automation[*i].combine.is_none())
        {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_TARGET_INVALID,
                format!(
                    "multiple lanes target {entity:?}.{parameter:?}; every lane must declare combine"
                ),
                format!("$.automation[{}].combine", lanes[0]),
            ));
        }
    }
    Ok(())
}
