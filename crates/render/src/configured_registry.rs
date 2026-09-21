//! Per-instance native capability negotiation, before validation or DSP graph allocation.
use oxitone_core::{wire::ProjectSnapshot, OxitoneError};
use oxitone_graph::{HostContext, PluginRegistry};
use std::borrow::Cow;

pub(crate) fn prepare<'a>(
    snapshot: &ProjectSnapshot,
    registry: &'a PluginRegistry,
    host: &HostContext,
) -> Result<Cow<'a, PluginRegistry>, OxitoneError> {
    if !registry.has_configured_factories() {
        return Ok(Cow::Borrowed(registry));
    }
    oxitone_graph::validate::validate_structure(snapshot)?;
    let mut configured = registry.clone();
    let instruments = snapshot.channels.iter().map(|c| {
        (
            &c.instrument.plugin_id,
            &c.instrument.plugin_version,
            c.instrument.instance_id.as_deref(),
            &c.instrument.parameters,
            c.instrument.state.as_ref(),
        )
    });
    let effects = snapshot
        .channels
        .iter()
        .flat_map(|c| &c.effect_chain)
        .chain(snapshot.mixer_channels.iter().flat_map(|c| &c.inserts))
        .map(|e| {
            (
                &e.plugin_id,
                &e.plugin_version,
                e.instance_id.as_deref(),
                &e.parameters,
                e.state.as_ref(),
            )
        });
    for (id, version, instance, parameters, state) in instruments.chain(effects) {
        let Some(plugin) = registry.lookup(id, version) else {
            continue;
        };
        if let Some(factory) = plugin.configured_factory(host, parameters, state)? {
            let descriptor = factory.descriptor();
            if descriptor.plugin_id != *id || descriptor.plugin_version != *version {
                return Err(OxitoneError::new(
                    "PluginManifestMismatch",
                    "configured factory changed plugin identity",
                ));
            }
            let instance = instance.ok_or_else(|| {
                OxitoneError::new(
                    "PluginConfigInvalid",
                    "configured native plugin requires an instanceId",
                )
            })?;
            configured.register_instance(instance, factory)?;
        }
    }
    Ok(Cow::Owned(configured))
}
