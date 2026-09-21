use crate::{
    native,
    wire::{Configuration, Event, MAX_STATE},
    Result,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::BTreeMap;
use vst3_host::Plugin;

pub fn apply(
    plugin: &mut Plugin,
    configuration: Option<&Configuration>,
    values: &BTreeMap<String, f64>,
    events: &[Event],
) -> Result<()> {
    let mut refresh_values = false;
    if let Some(configuration) = configuration {
        let state = STANDARD
            .decode(&configuration.state_base64)
            .map_err(crate::invalid)?;
        if state.len() > MAX_STATE {
            return Err(crate::invalid("VST3 state exceeds 4 MiB"));
        }
        plugin.load_state(&state).map_err(native)?;
        refresh_values = crate::silent_processing::service(plugin)?.param_values_changed();
    }
    // A restored preset may replace the controller's parameter table. Validate against that table.
    let descriptors = crate::inspection::parameters(plugin)?;
    let parameters: BTreeMap<_, _> = descriptors.iter().map(|p| (p.id, p)).collect();
    let validate = |id: u32, automate: bool| -> Result<()> {
        let parameter = parameters
            .get(&id)
            .ok_or_else(|| crate::invalid("Unknown VST3 parameter"))?;
        if parameter.is_read_only || (automate && !parameter.can_automate) {
            return Err(crate::invalid("VST3 parameter is not writable/automatable"));
        }
        Ok(())
    };
    for id in values
        .keys()
        .chain(configuration.iter().flat_map(|c| c.parameters.keys()))
    {
        validate(id.parse().map_err(crate::invalid)?, false)?;
    }
    for event in events {
        if let Event::Parameter { parameter_id, .. } = event {
            validate(*parameter_id, true)?;
        } else if !plugin.info().has_midi_input {
            return Err(crate::unsupported("VST3 plugin has no note input"));
        }
    }
    let mut merged: BTreeMap<String, f64> = if refresh_values {
        descriptors
            .iter()
            .filter(|p| !p.is_read_only)
            .map(|p| (p.id.to_string(), p.value))
            .collect()
    } else {
        BTreeMap::new()
    };
    if let Some(configuration) = configuration {
        merged.extend(configuration.parameters.clone());
    }
    merged.extend(values.clone());
    // Coalesce overrides before enqueueing, so a full 4096-parameter preset fits the native queue.
    for (id, value) in &merged {
        plugin
            .set_parameter(id.parse().map_err(crate::invalid)?, *value)
            .map_err(native)?;
    }
    Ok(())
}
