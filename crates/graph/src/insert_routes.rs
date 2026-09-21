//! Mixer insert endpoint validation. Physical capability checks use graph-local restored factories.
use crate::{topology::MASTER_MIXER_CHANNEL_ID as MASTER, PluginRegistry};
use oxitone_core::{wire::MixerChannelSpec, OxitoneError};
use std::collections::BTreeSet;

pub fn bus_index(value: &str) -> Option<usize> {
    value
        .parse::<usize>()
        .ok()
        .filter(|i| (1..16).contains(i) && i.to_string() == value)
}

fn invalid(message: &str, owner: &str, instance: &str) -> OxitoneError {
    OxitoneError::with_path(
        "InvalidProject",
        message,
        format!("$.mixerChannels.{owner}.insertRoutes.{instance}"),
    )
}

pub fn validate_references(buses: &[MixerChannelSpec]) -> Result<(), OxitoneError> {
    let ids: BTreeSet<_> = buses
        .iter()
        .map(|b| b.id.as_str())
        .chain([MASTER])
        .collect();
    for bus in buses {
        for (instance, routes) in bus.insert_routes.iter().flatten() {
            if !bus
                .inserts
                .iter()
                .any(|effect| effect.instance_id.as_ref() == Some(instance))
            {
                return Err(invalid(
                    "route must address an insert of its own mixer bus",
                    &bus.id,
                    instance,
                ));
            }
            for (input, targets) in [(true, &routes.inputs), (false, &routes.outputs)] {
                for (index, target) in targets.iter().flatten() {
                    if bus_index(index).is_none() {
                        return Err(invalid(
                            "auxiliary bus index must be canonical 1..15",
                            &bus.id,
                            instance,
                        ));
                    }
                    if !ids.contains(target.as_str())
                        || target == &bus.id
                        || (input && target == MASTER)
                        || (!input && bus.id == MASTER)
                    {
                        return Err(invalid(
                            "invalid insert bus route endpoint",
                            &bus.id,
                            instance,
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn validate_capabilities(
    buses: &[MixerChannelSpec],
    registry: &PluginRegistry,
) -> Result<(), OxitoneError> {
    for bus in buses {
        for (instance, routes) in bus.insert_routes.iter().flatten() {
            let effect = bus
                .inserts
                .iter()
                .find(|effect| effect.instance_id.as_ref() == Some(instance))
                .expect("validated route owner");
            let plugin = registry
                .lookup_instance(&effect.plugin_id, &effect.plugin_version, Some(instance))
                .ok_or_else(|| invalid("unknown insert plugin", &bus.id, instance))?;
            for (targets, count) in [
                (&routes.inputs, plugin.input_bus_count()),
                (&routes.outputs, plugin.output_bus_count()),
            ] {
                for index in targets.iter().flatten().map(|(index, _)| index) {
                    if bus_index(index).is_none_or(|index| index >= count) {
                        return Err(OxitoneError::with_path(
                            "PluginCapabilityUnsupported",
                            "insert does not expose the routed physical bus",
                            format!("$.mixerChannels.{}.insertRoutes.{instance}.{index}", bus.id),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}
