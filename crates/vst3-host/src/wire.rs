use crate::{invalid, Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;

pub const MAX_REQUEST: usize = 8 * 1024 * 1024;
pub const MAX_STATE: usize = 4 * 1024 * 1024;
/// Canonical MIDI 1.0 channel messages; two-byte messages have a zero third byte.
pub fn valid_midi([status, first, second]: [u8; 3]) -> bool {
    (0x80..=0xef).contains(&status)
        && first < 128
        && second < 128
        && (!matches!(status & 0xf0, 0xc0 | 0xd0) || second == 0)
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub bundle_path: String,
    pub class_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    pub allow_plugins: Policy,
}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    SignedOnly,
    Any,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub format_version: u32,
    pub class_id: String,
    pub sha256: String,
    pub state_base64: String,
    pub parameters: BTreeMap<String, f64>,
}
pub use crate::event_wire::{Event, OwnedEvent};
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderOptions {
    pub path: String,
    pub sample_rate: u32,
    pub block_size: usize,
    pub frames: u64,
    pub tail_frames: u64,
    pub input_path: Option<String>,
    pub configuration: Option<Configuration>,
    pub parameters: BTreeMap<String, f64>,
    pub events: Vec<OwnedEvent>,
    pub tempo: f64,
    pub time_signature: [i32; 2],
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub operation: Operation,
    pub source: Source,
    pub options: Option<RenderOptions>,
}
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Inspect,
    Render,
}

pub fn normalized(value: f64) -> Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(invalid("parameter/velocity must be in 0..1"));
    }
    Ok(())
}
pub fn parameters(parameters: &BTreeMap<String, f64>) -> Result<()> {
    if parameters.len() > 4096 {
        return Err(invalid("too many parameters"));
    }
    for (id, value) in parameters {
        let parsed = id.parse::<u32>().map_err(|_| invalid("invalid ParamID"))?;
        if parsed.to_string() != *id {
            return Err(invalid("ParamID must be canonical decimal"));
        }
        normalized(*value)?;
    }
    Ok(())
}
pub fn local_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\0')
        || !std::path::Path::new(path).is_absolute()
    {
        return Err(invalid("paths must be nonempty absolute local paths"));
    }
    Ok(())
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != 1 {
            return Err(Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 protocol",
            ));
        }
        self.source.validate()?;
        match (&self.operation, &self.options) {
            (Operation::Inspect, None) => Ok(()),
            (Operation::Render, Some(options)) => options.validate(),
            _ => Err(invalid("operation/options mismatch")),
        }
    }
}
impl Source {
    pub fn validate(&self) -> Result<()> {
        local_path(&self.bundle_path)?;
        if self.class_id.len() != 32 || !self.class_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("class ID must contain 32 hex characters"));
        }
        if let Some(hash) = &self.expected_hash {
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid("invalid expectedHash"));
            }
        }
        Ok(())
    }
}
impl RenderOptions {
    pub fn validate(&self) -> Result<()> {
        local_path(&self.path)?;
        if let Some(path) = &self.input_path {
            local_path(path)?;
        }
        processing_settings(
            self.sample_rate,
            self.block_size,
            self.tempo,
            self.time_signature,
            self.configuration.as_ref(),
            &self.parameters,
        )?;
        if self.frames == 0
            || self.tail_frames > 192000 * 60
            || self
                .frames
                .checked_add(self.tail_frames)
                .is_none_or(|v| v > 536_000_000)
        {
            return Err(Error::new(
                "WavTooLarge",
                "render exceeds bounded RIFF duration",
            ));
        }
        if self.events.len() > 100_000 {
            return Err(invalid("event count exceeds 100000"));
        }
        let mut budgets: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
        for event in &self.events {
            if event.frame() >= self.frames {
                return Err(invalid("event outside content interval"));
            }
            let budget = budgets
                .entry(event.frame() / self.block_size as u64)
                .or_default();
            budget.0 += 1;
            if let OwnedEvent::SysEx { data, .. } = event {
                budget.1 += data.len();
            }
            if budget.0 > crate::stream_wire::MAX_EVENTS
                || budget.1 > oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES
            {
                return Err(Error::new(
                    "BudgetExceeded",
                    "VST3 block event or SysEx budget exceeded",
                ));
            }
            match event {
                OwnedEvent::SysEx { data, .. } => {
                    if !oxitone_core::midi_bytes::valid_sysex(data) {
                        return Err(invalid("invalid SysEx message"));
                    }
                }
                OwnedEvent::Midi { message, .. } => {
                    if !valid_midi(*message) {
                        return Err(invalid("invalid MIDI channel message"));
                    }
                }
                OwnedEvent::Parameter { value, .. } => normalized(*value)?,
                OwnedEvent::NoteOn {
                    channel,
                    pitch,
                    velocity,
                    ..
                }
                | OwnedEvent::NoteOff {
                    channel,
                    pitch,
                    velocity,
                    ..
                } => {
                    if *channel > 15 || *pitch > 127 {
                        return Err(invalid("invalid MIDI channel/pitch"));
                    }
                    normalized(*velocity)?;
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn processing_settings(
    sample_rate: u32,
    block_size: usize,
    tempo: f64,
    time_signature: [i32; 2],
    configuration: Option<&Configuration>,
    values: &BTreeMap<String, f64>,
) -> Result<()> {
    if !(8000..=192000).contains(&sample_rate) || !(16..=4096).contains(&block_size) {
        return Err(invalid("unsupported sample rate or block size"));
    }
    if !tempo.is_finite()
        || !(20.0..=999.0).contains(&tempo)
        || !(1..=32).contains(&time_signature[0])
        || ![1, 2, 4, 8, 16].contains(&time_signature[1])
    {
        return Err(invalid("invalid tempo/time signature"));
    }
    parameters(values)?;
    if let Some(state) = configuration {
        if state.format_version != 1 {
            return Err(Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 configuration; preserve original data",
            ));
        }
        if state.class_id.len() != 32 || !state.class_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid(
                "configuration class ID must contain 32 hex characters",
            ));
        }
        if state.sha256.len() != 64 || !state.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid(
                "configuration sha256 must contain 64 hex characters",
            ));
        }
        if state.state_base64.len() > 4 * MAX_STATE.div_ceil(3) {
            return Err(invalid("state exceeds 4 MiB"));
        }
        let decoded = STANDARD
            .decode(&state.state_base64)
            .map_err(|_| invalid("stateBase64 is not valid padded base64"))?;
        if decoded.len() > MAX_STATE {
            return Err(Error::new("BudgetExceeded", "state exceeds 4 MiB"));
        }
        parameters(&state.parameters)?;
    }
    Ok(())
}
