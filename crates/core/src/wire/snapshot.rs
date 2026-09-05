//! Project snapshot wire type and its canonical decode/encode entry points.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::authoring::{
    AutomationLaneSpec, ChannelSpec, MixerChannelSpec, PatternClipSpec, PatternSpec, SampleClipSpec,
};
use super::basic::{MarkerSpec, SampleRef, TempoSegment, TimeSignatureSegment, TrackSpec};
use super::serde_util::u64_string;
use super::EntityId;
use crate::canonical::to_canonical_json;
use crate::error::{codes, OxitoneError};
use crate::version::check_protocol_version;

/// Immutable, fully validated project state exchanged with the engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub protocol_version: String,
    #[serde(with = "u64_string")]
    pub revision: u64,
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub sample_rate: u32,
    pub block_size: u32,
    pub seed: u64,
    pub tempo_map: Vec<TempoSegment>,
    pub time_signature_map: Vec<TimeSignatureSegment>,
    pub markers: Vec<MarkerSpec>,
    pub tracks: Vec<TrackSpec>,
    pub patterns: Vec<PatternSpec>,
    pub pattern_clips: Vec<PatternClipSpec>,
    pub sample_clips: Vec<SampleClipSpec>,
    pub samples: Vec<SampleRef>,
    pub channels: Vec<ChannelSpec>,
    pub mixer_channels: Vec<MixerChannelSpec>,
    pub automation: Vec<AutomationLaneSpec>,
}

/// Decode a project JSON document. Unknown fields are ignored; an unknown
/// major version (or newer minor) returns `ProtocolVersionUnsupported`.
pub fn decode_project_snapshot(json: &str) -> Result<ProjectSnapshot, OxitoneError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| OxitoneError::new(codes::INVALID_PROJECT, format!("invalid JSON: {e}")))?;
    let version = value
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| OxitoneError::new(codes::INVALID_PROJECT, "missing protocolVersion"))?;
    check_protocol_version(version)?;
    serde_json::from_value(value)
        .map_err(|e| OxitoneError::new(codes::INVALID_PROJECT, format!("invalid snapshot: {e}")))
}

/// Serialize a snapshot as canonical JSON (with trailing LF).
pub fn encode_project_snapshot(snapshot: &ProjectSnapshot) -> Result<String, OxitoneError> {
    to_canonical_json(snapshot)
}
