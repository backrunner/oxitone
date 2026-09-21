use crate::{native, wire::MAX_STATE, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::{BTreeMap, BTreeSet};
use vst3_host::{Parameter, Plugin};

pub(crate) fn channels(plugin: &Plugin) -> Result<(usize, usize)> {
    let layout = plugin.audio_bus_layout().map_err(native)?;
    if layout.inputs.len() > 1 || layout.outputs.len() != 1 {
        return Err(crate::unsupported("The stereo WAV API requires at most one input bus and exactly one output bus; use the native bus stream for multibus audio"));
    }
    let input = layout.inputs.first().map_or(0, |bus| bus.channel_count);
    let output = layout.outputs[0].channel_count;
    if input > 2 || !(1..=2).contains(&output) {
        return Err(crate::unsupported(
            "Only mono/stereo VST3 layouts are supported",
        ));
    }
    Ok((input, output))
}

pub(crate) fn parameters(plugin: &Plugin) -> Result<Vec<Parameter>> {
    let parameters = plugin.get_parameters().map_err(native)?;
    if parameters.len() > 4096 {
        return Err(crate::Error::new(
            "BudgetExceeded",
            "VST3 parameter count exceeds 4096",
        ));
    }
    let mut ids = BTreeSet::new();
    for parameter in &parameters {
        crate::wire::normalized(parameter.value)?;
        crate::wire::normalized(parameter.default)?;
        if parameter.step_count < 0 || !ids.insert(parameter.id) {
            return Err(crate::invalid(
                "Invalid or duplicate VST3 parameter descriptor",
            ));
        }
    }
    Ok(parameters)
}

pub fn inspect(plugin: &mut Plugin, hash: String) -> Result<serde_json::Value> {
    let buses = crate::bus_wire::inspect(plugin)?;
    inspect_with_buses(plugin, hash, buses)
}

pub(crate) fn inspect_frozen(plugin: &mut Plugin, hash: String) -> Result<serde_json::Value> {
    let buses = crate::bus_wire::declared(plugin)?;
    inspect_with_buses(plugin, hash, buses)
}

fn inspect_with_buses(
    plugin: &mut Plugin,
    hash: String,
    buses: crate::bus_wire::AudioBuses,
) -> Result<serde_json::Value> {
    let input = buses.inputs.first().map_or(0, |bus| bus.channels);
    let output = buses.outputs.first().map_or(0, |bus| bus.channels);
    if output == 0 && !plugin.info().has_midi_output {
        return Err(crate::unsupported("outputless VST3 requires MIDI output"));
    }
    let parameters = parameters(plugin)?;
    let values: BTreeMap<_, _> = parameters
        .iter()
        .filter(|p| !p.is_read_only)
        .map(|p| (p.id.to_string(), p.value))
        .collect();
    let state = plugin.save_state().map_err(native)?;
    if state.len() > MAX_STATE {
        return Err(crate::Error::new(
            "BudgetExceeded",
            "VST3 state exceeds 4 MiB",
        ));
    }
    let info = plugin.info();
    Ok(serde_json::json!({
        "protocolVersion": 1, "classId": info.uid.to_ascii_lowercase(),
        "name": info.name, "vendor": info.vendor, "version": info.version, "category": info.category,
        "sha256": hash, "inputChannels": input, "outputChannels": output, "noteInput": info.has_midi_input,
        "noteOutput": info.has_midi_output,
        "audioBuses": buses,
        "parameters": parameters.iter().map(|p| serde_json::json!({
            "id":p.id,"name":p.name,"unit":p.unit,"value":p.value,"default":p.default,
            "stepCount":p.step_count,"canAutomate":p.can_automate,"readOnly":p.is_read_only,
        })).collect::<Vec<_>>(),
        "configuration": {"formatVersion":1,"classId":info.uid.to_ascii_lowercase(),
            "sha256":hash,"stateBase64":STANDARD.encode(state),"parameters":values}
    }))
}
