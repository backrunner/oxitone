//! Metadata presentation is accepted only after comparison with the isolated processor handshake.
use super::invalid;
use oxitone_core::{
    wire::{ParameterMapping, ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit},
    OxitoneError,
};
use oxitone_graph::{
    ChannelLayout, PluginCapabilities, PluginDescriptor, PluginKind, StateSchemaId,
};
use oxitone_vst3_host::{
    stream_wire::Ready,
    wire::{Configuration, Source},
};
use serde::Deserialize;
#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Metadata {
    protocol_version: u32,
    class_id: String,
    pub name: String,
    vendor: String,
    version: String,
    category: String,
    pub sha256: String,
    pub input_channels: usize,
    output_channels: usize,
    pub audio_buses: oxitone_vst3_host::bus_wire::AudioBuses,
    pub note_input: bool,
    pub note_output: bool,
    parameters: Vec<Parameter>,
    configuration: Option<Configuration>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Parameter {
    id: u32,
    name: String,
    unit: String,
    value: f64,
    default: f64,
    step_count: u32,
    can_automate: bool,
    read_only: bool,
}

impl Metadata {
    pub fn validate(&self, source: &Source) -> Result<(), OxitoneError> {
        if self.protocol_version != 1
            || !self.class_id.eq_ignore_ascii_case(&source.class_id)
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || source
                .expected_hash
                .as_ref()
                .is_some_and(|h| !h.eq_ignore_ascii_case(&self.sha256))
            || self.parameters.len() > 4096
            || !self.audio_buses.valid()
            || self.input_channels != self.audio_buses.inputs.first().map_or(0, |b| b.channels)
            || self.output_channels != self.audio_buses.outputs.first().map_or(0, |b| b.channels)
            || (self.audio_buses.outputs.is_empty() && !self.note_output)
        {
            return Err(invalid(
                "VST3 inspection identity does not match registration",
            ));
        }
        for (i, p) in self.parameters.iter().enumerate() {
            if ![p.value, p.default]
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                || self.parameters[..i].iter().any(|other| other.id == p.id)
            {
                return Err(invalid("invalid VST3 parameter metadata"));
            }
        }
        // These are display-only inspection fields; the processor controls runtime capabilities.
        let _ = (
            &self.name,
            &self.vendor,
            &self.version,
            &self.category,
            &self.configuration,
        );
        Ok(())
    }
    pub fn check_ready(&self, ready: &Ready) -> Result<(), OxitoneError> {
        if ready.input_channels != self.input_channels
            || ready.output_channels != self.output_channels
            || ready.note_input != self.note_input
            || ready.note_output != self.note_output
            || ready.category != self.category
            || ready.audio_buses.inputs.iter().map(|b| b.channels).ne(self
                .audio_buses
                .inputs
                .iter()
                .map(|b| b.channels))
            || ready.audio_buses.outputs.iter().map(|b| b.channels).ne(self
                .audio_buses
                .outputs
                .iter()
                .map(|b| b.channels))
            || ready.parameters.len() != self.parameters.len()
            || !ready.class_id.eq_ignore_ascii_case(&self.class_id)
            || !ready.sha256.eq_ignore_ascii_case(&self.sha256)
            || self.parameters.iter().any(|p| {
                !ready.parameters.iter().any(|r| {
                    r.id == p.id && r.writable == !p.read_only && r.automatable == p.can_automate
                })
            })
        {
            return Err(invalid(
                "VST3 processor capabilities changed since inspection",
            ));
        }
        Ok(())
    }
    pub fn descriptor(&self) -> Result<PluginDescriptor, OxitoneError> {
        let instrument = self.is_instrument();
        if instrument && !self.note_input && !self.is_midi_only() {
            return Err(invalid("VST3 instrument must accept notes"));
        }
        if !instrument && self.input_channels == 0 {
            return Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "VST3 effect must have a main audio input",
            ));
        }
        let descriptor = PluginDescriptor {
            plugin_id: format!("vst3.{}", self.class_id.to_ascii_lowercase()),
            plugin_version: format!("0.0.0+{}", self.sha256),
            kind: if instrument {
                PluginKind::Instrument
            } else {
                PluginKind::Effect
            },
            input_layout: if instrument {
                ChannelLayout::None
            } else {
                ChannelLayout::Stereo
            },
            output_layout: ChannelLayout::Stereo,
            parameters: self
                .parameters
                .iter()
                .filter(|p| !p.read_only)
                .map(|p| {
                    let _ = (&p.unit, p.step_count);
                    ParameterSpec {
                        id: p.id.to_string(),
                        label: p.name.clone(),
                        unit: ParameterUnit::Normalized,
                        min: 0.0,
                        max: 1.0,
                        default: p.default,
                        smoothing: ParameterSmoothing::None,
                        rate: ParameterRate::Control,
                        automation: Some(p.can_automate),
                        mapping: Some(ParameterMapping::Linear),
                    }
                })
                .collect(),
            capabilities: PluginCapabilities {
                sidechain_input: !instrument && self.audio_buses.inputs.len() >= 2,
                reports_tail: true,
            },
            state_schema: Some(StateSchemaId("oxitone.vst3.configuration@1")),
            max_polyphony: instrument.then_some(128),
        };
        descriptor.validate()?;
        Ok(descriptor)
    }
    pub fn is_instrument(&self) -> bool {
        self.is_midi_only()
            || self
                .category
                .split('|')
                .any(|part| part.trim().eq_ignore_ascii_case("Instrument"))
            || (self.input_channels == 0
                && self.note_input
                && !self
                    .category
                    .split('|')
                    .any(|part| part.trim().eq_ignore_ascii_case("Fx")))
    }
    fn is_midi_only(&self) -> bool {
        self.audio_buses.inputs.is_empty()
            && self.audio_buses.outputs.is_empty()
            && self.note_output
    }
}
