//! Versioned, bounded control messages. Never used by the realtime audio port.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

pub const CONTROL_VERSION: u32 = 1;
pub const MAGIC: &[u8; 4] = b"OXVC";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Command {
    StartEdits {},
    StartRecording {
        mode: crate::edit_wire::RecordingMode,
        #[serde(rename = "parameterIds")]
        parameter_ids: Vec<u32>,
    },
    ReadEdits {
        #[serde(rename = "captureId")]
        capture_id: String,
        #[serde(rename = "fromSequence")]
        from_sequence: u64,
    },
    StopEdits {
        #[serde(rename = "captureId")]
        capture_id: String,
        #[serde(rename = "fromSequence")]
        from_sequence: u64,
    },
    DiscardEdits {
        #[serde(rename = "captureId")]
        capture_id: String,
    },
    OpenEditor {},
    CloseEditor {},
    Poll {},
    Capture {},
    SetParameter {
        #[serde(rename = "parameterId")]
        parameter_id: u32,
        value: f64,
    },
}
impl Command {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::StartRecording { parameter_ids, .. }
                if !crate::edit_wire::valid_selection(parameter_ids) =>
            {
                return Err(crate::invalid("invalid VST3 recording parameter selection"))
            }
            Self::ReadEdits {
                capture_id,
                from_sequence,
            }
            | Self::StopEdits {
                capture_id,
                from_sequence,
            } => {
                if !crate::edit_wire::valid_id(capture_id)
                    || *from_sequence > crate::edit_wire::MAX_SEQUENCE
                {
                    return Err(crate::invalid("invalid VST3 gesture cursor"));
                }
            }
            Self::DiscardEdits { capture_id } if !crate::edit_wire::valid_id(capture_id) => {
                return Err(crate::invalid("invalid VST3 gesture capture id"))
            }
            _ => {}
        }
        if let Self::SetParameter { value, .. } = self {
            crate::wire::normalized(*value)?;
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub control_protocol_version: u32,
    pub command: Command,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if self.control_protocol_version != CONTROL_VERSION {
            return Err(Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 control protocol",
            ));
        }
        self.command.validate()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub editor_open: bool,
    pub next_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edits: Option<crate::edit_wire::Page>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<Restart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Restart {
    pub reasons: Vec<RestartReason>,
    pub latency_frames: u32,
    pub tail_frames: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestartReason {
    Io,
    Latency,
    Parameters,
    Reload,
}
impl Restart {
    pub fn valid(&self) -> bool {
        !self.reasons.is_empty()
            && self.reasons.len() <= 4
            && self
                .reasons
                .iter()
                .enumerate()
                .all(|(i, reason)| !self.reasons[..i].contains(reason))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_versions_unknown_fields_and_invalid_parameters() {
        for command in [
            json!({"kind":"setParameter","parameterId":-1,"value":0.5}),
            json!({"kind":"poll","pcm":[]}),
            json!({"kind":"restore"}),
        ] {
            assert!(serde_json::from_value::<Command>(command).is_err());
        }
        for value in [-1., 1.1, f64::NAN] {
            assert!(Command::SetParameter {
                parameter_id: 0,
                value
            }
            .validate()
            .is_err());
        }
        assert_eq!(
            Request {
                control_protocol_version: 2,
                command: Command::Poll {}
            }
            .validate()
            .unwrap_err()
            .code,
            "ProtocolVersionUnsupported"
        );
    }
}
