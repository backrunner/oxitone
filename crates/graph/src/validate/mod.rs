//! Snapshot validation (02-domain-spec.md, 04-api-contracts.md). Runs fully
//! off the audio thread at compile time; the first failure is returned with a
//! stable error code and a JSON path.

mod automation;
mod clips;
mod ids;
mod levels;
mod numeric;
mod playlist;
mod plugins;
mod refs;
mod state;

use oxitone_core::error::OxitoneError;
use oxitone_core::wire::ProjectSnapshot;

use crate::registry::PluginRegistry;
use crate::topology::build_mixer_routing;

/// Validate a decoded snapshot against the domain rules and the plugin
/// registry. Checks run in a fixed order (ID structure → numeric domains →
/// references → plugin tables → plugin state → automation targets → mixer
/// topology) so the same snapshot always fails with the same error.
pub fn validate(snapshot: &ProjectSnapshot, registry: &PluginRegistry) -> Result<(), OxitoneError> {
    validate_structure(snapshot)?;
    plugins::validate_plugins(snapshot, registry)?;
    crate::midi::compile(&snapshot.channels, Some(registry))?;
    crate::insert_routes::validate_capabilities(&snapshot.mixer_channels, registry)?;
    state::validate_states(snapshot, registry)?;
    automation::validate_automation(snapshot, registry)?;
    build_mixer_routing(&snapshot.mixer_channels)?;
    Ok(())
}

/// Structural checks before configuration-dependent native capability discovery.
pub fn validate_structure(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    oxitone_core::wire::check_snapshot_version(snapshot)?;
    ids::validate_ids(snapshot)?;
    numeric::validate_numeric(snapshot)?;
    refs::validate_refs(snapshot)?;
    crate::midi::compile(&snapshot.channels, None)?;
    crate::insert_routes::validate_references(&snapshot.mixer_channels)?;
    Ok(())
}
