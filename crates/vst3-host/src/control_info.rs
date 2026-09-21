//! Validate captured helper state against its immutable identity and current capability table.
use crate::{bus_wire::AudioBuses, stream_wire::Ready, wire::Configuration};
use serde::Deserialize;
use std::{collections::BTreeSet, io};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Info {
    protocol_version: u32,
    class_id: String,
    sha256: String,
    name: String,
    vendor: String,
    version: String,
    category: String,
    input_channels: usize,
    output_channels: usize,
    audio_buses: AudioBuses,
    note_input: bool,
    note_output: bool,
    parameters: Vec<Parameter>,
    configuration: Configuration,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Parameter {
    id: u32,
    name: String,
    unit: String,
    value: f64,
    default: f64,
    step_count: i32,
    can_automate: bool,
    read_only: bool,
}
pub(super) fn validate(value: &serde_json::Value, ready: &Ready, changed: bool) -> io::Result<()> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid VST3 captured state");
    let info: Info = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    let config = &info.configuration;
    crate::wire::processing_settings(
        ready.sample_rate,
        ready.block_size,
        120.,
        [4, 4],
        Some(config),
        &Default::default(),
    )
    .map_err(|_| invalid())?;
    let mut ids = BTreeSet::new();
    if info.protocol_version != 1
        || info.class_id != ready.class_id
        || info.sha256 != ready.sha256
        || !config.class_id.eq_ignore_ascii_case(&ready.class_id)
        || config.sha256 != ready.sha256
        || !info.audio_buses.valid()
        || info.input_channels != info.audio_buses.inputs.first().map_or(0, |b| b.channels)
        || info.output_channels != info.audio_buses.outputs.first().map_or(0, |b| b.channels)
        || (info.audio_buses.outputs.is_empty() && !info.note_output)
        || info.parameters.len() > 4096
        || info.parameters.iter().any(|p| {
            !ids.insert(p.id)
                || p.step_count < 0
                || crate::wire::normalized(p.value).is_err()
                || crate::wire::normalized(p.default).is_err()
        })
        || config.parameters.len() != info.parameters.iter().filter(|p| !p.read_only).count()
        || info
            .parameters
            .iter()
            .filter(|p| !p.read_only)
            .any(|p| config.parameters.get(&p.id.to_string()) != Some(&p.value))
    {
        return Err(invalid());
    }
    if !changed
        && (info.audio_buses != ready.audio_buses
            || info.note_input != ready.note_input
            || info.note_output != ready.note_output
            || info.category != ready.category
            || info.parameters.len() != ready.parameters.len()
            || info.parameters.iter().any(|p| {
                !ready.parameters.iter().any(|r| {
                    r.id == p.id && r.writable != p.read_only && r.automatable == p.can_automate
                })
            }))
    {
        return Err(invalid());
    }
    // Strings have been type checked; the enclosing 8 MiB frame bounds their total storage.
    let _ = (info.name, info.vendor, info.version);
    for parameter in info.parameters {
        let _ = (parameter.name, parameter.unit);
    }
    Ok(())
}
