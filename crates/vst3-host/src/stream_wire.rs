//! Control handshake and bounded block records, independent of any VST3 runtime dependency.
use crate::{
    invalid,
    wire::{Configuration, Event, Source},
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_EVENTS: usize = 256;
pub const STREAM_VERSION: u32 = 11;
#[cfg(test)]
#[path = "stream_wire_tests.rs"]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub stream_protocol_version: u32,
    pub source: Source,
    pub options: Options,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    pub midi_output: bool,
    #[serde(default)]
    pub processing_mode: ProcessingMode,
    pub sample_rate: u32,
    pub block_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Configuration>,
    pub parameters: BTreeMap<String, f64>,
    pub tempo: f64,
    pub time_signature: [i32; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<crate::transport_wire::Transport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus_activation: Option<crate::bus_wire::BusActivation>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessingMode {
    #[default]
    Realtime,
    Offline,
}
impl Start {
    pub fn validate(&self) -> Result<()> {
        if self.stream_protocol_version != STREAM_VERSION {
            return Err(crate::Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 stream protocol",
            ));
        }
        self.source.validate()?;
        let o = &self.options;
        if o.bus_activation.as_ref().is_some_and(|b| {
            b.inputs.len() > crate::bus_wire::MAX_BUSES
                || b.outputs.len() > crate::bus_wire::MAX_BUSES
        }) {
            return Err(invalid("invalid VST3 bus activation count"));
        }
        if o.transport.is_some_and(|transport| !transport.valid()) {
            return Err(invalid("invalid initial VST3 transport"));
        }
        crate::wire::processing_settings(
            o.sample_rate,
            o.block_size,
            o.tempo,
            o.time_signature,
            o.configuration.as_ref(),
            &o.parameters,
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parameter {
    pub id: u32,
    pub writable: bool,
    pub automatable: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ready {
    pub stream_protocol_version: u32,
    pub sample_rate: u32,
    pub block_size: usize,
    pub class_id: String,
    pub sha256: String,
    pub input_channels: usize,
    pub output_channels: usize,
    pub audio_buses: crate::bus_wire::AudioBuses,
    pub note_input: bool,
    pub note_output: bool,
    pub category: String,
    pub latency_frames: u32,
    pub tail_frames: u32,
    pub helper_time_constraint: bool,
    pub parameters: Vec<Parameter>,
}
impl Ready {
    pub fn validate(&mut self, start: &Start) -> Result<()> {
        if start.options.midi_output && !self.note_output {
            return Err(crate::unsupported("VST3 has no MIDI output"));
        }
        if self.stream_protocol_version != STREAM_VERSION
            || self.sample_rate != start.options.sample_rate
            || self.block_size != start.options.block_size
            || !self.class_id.eq_ignore_ascii_case(&start.source.class_id)
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || start
                .source
                .expected_hash
                .as_ref()
                .is_some_and(|v| !v.eq_ignore_ascii_case(&self.sha256))
            || self.input_channels > 2
            || self.output_channels > 2
            || !self.audio_buses.valid()
            || (self.audio_buses.outputs.is_empty() && !self.note_output)
            || self.input_channels != self.audio_buses.inputs.first().map_or(0, |b| b.channels)
            || self.output_channels != self.audio_buses.outputs.first().map_or(0, |b| b.channels)
            || start.options.bus_activation.as_ref().is_some_and(|a| {
                a.inputs.len() != self.audio_buses.inputs.len()
                    || a.outputs.len() != self.audio_buses.outputs.len()
                    || a.inputs
                        .iter()
                        .zip(&self.audio_buses.inputs)
                        .chain(a.outputs.iter().zip(&self.audio_buses.outputs))
                        .any(|(active, b)| *active != b.active)
            })
            || self.parameters.len() > 4096
            || self.latency_frames > start.options.sample_rate * 10
            || start.options.configuration.as_ref().is_some_and(|c| {
                !c.sha256.eq_ignore_ascii_case(&self.sha256)
                    || !c.class_id.eq_ignore_ascii_case(&self.class_id)
            })
        {
            return Err(invalid(
                "invalid VST3 stream handshake identity/capabilities",
            ));
        }
        self.parameters.sort_by_key(|p| p.id);
        if self.parameters.windows(2).any(|p| p[0].id == p[1].id) {
            return Err(invalid("duplicate VST3 stream parameter"));
        }
        Ok(())
    }
    /// Scalar checks only; suitable for use before entering a realtime queue.
    pub fn valid_event(&self, event: &Event, frames: usize) -> bool {
        if event.frame() >= frames as u64 {
            return false;
        }
        match *event {
            Event::SysEx { data, .. } => {
                self.note_input
                    && (2..=oxitone_core::midi_bytes::MAX_SYSEX_BYTES as u32).contains(&data.length)
            }
            Event::Midi { message, .. } => self.note_input && crate::wire::valid_midi(message),
            Event::Parameter {
                parameter_id,
                value,
                ..
            } => {
                value.is_finite()
                    && (0.0..=1.0).contains(&value)
                    && self
                        .parameters
                        .binary_search_by_key(&parameter_id, |p| p.id)
                        .ok()
                        .is_some_and(|i| {
                            self.parameters[i].writable && self.parameters[i].automatable
                        })
            }
            Event::NoteOn {
                channel,
                pitch,
                velocity,
                ..
            }
            | Event::NoteOff {
                channel,
                pitch,
                velocity,
                ..
            } => {
                self.note_input
                    && channel < 16
                    && pitch < 128
                    && velocity.is_finite()
                    && (0.0..=1.0).contains(&velocity)
            }
        }
    }
}

#[cfg(all(feature = "stream", unix))]
pub(crate) struct Block {
    pub reset: bool,
    pub restart_required: bool,
    pub transport: Option<crate::transport_wire::Transport>,
    pub sequence: u64,
    pub processing_micros: u32,
    pub frames: usize,
    pub bus_count: usize,
    pub max_frames: usize,
    pub audio: Box<[f32]>,
    pub events: [Event; MAX_EVENTS],
    pub event_count: usize,
    pub payload: Vec<u8>,
}
#[cfg(all(feature = "stream", unix))]
impl Block {
    pub fn new(max_frames: usize) -> Self {
        Self::with_buses(max_frames, 1)
    }
    pub fn with_buses(max_frames: usize, buses: usize) -> Self {
        Self {
            reset: false,
            restart_required: false,
            transport: None,
            sequence: 0,
            processing_micros: 0,
            frames: 0,
            bus_count: 1,
            max_frames,
            audio: vec![0.; 2 * max_frames * buses].into_boxed_slice(),
            events: [Event::Parameter {
                frame: 0,
                parameter_id: 0,
                value: 0.,
            }; MAX_EVENTS],
            event_count: 0,
            payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
        }
    }
}
