//! Command/event envelopes and engine/render option wire types
//! (04-api-contracts.md §Facade 与 commands, §Wire messages).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::serde_util::{opt_u64_string, u64_string};
use super::snapshot::ProjectSnapshot;
use super::EntityId;
use crate::beat::Beat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransportCommandKind {
    Play,
    Pause,
    Stop,
    Seek,
}

/// Versioned command envelope sent from TypeScript to the native engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NativeCommand {
    Compile {
        #[serde(with = "u64_string")]
        revision: u64,
        snapshot: Box<ProjectSnapshot>,
    },
    Transport {
        command: TransportCommandKind,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "opt_u64_string"
        )]
        frame: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        beat: Option<Beat>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seconds: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        loop_region: Option<LoopRegion>,
    },
    SetParameter {
        entity_id: EntityId,
        parameter_id: String,
        value: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "opt_u64_string"
        )]
        at_frame: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRegion {
    #[serde(with = "u64_string")]
    pub start_frame: u64,
    #[serde(with = "u64_string")]
    pub end_frame: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeEventType {
    Compiled,
    Transport,
    Meter,
    Diagnostic,
    Fault,
}

/// Event envelope emitted by the native engine to TypeScript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeEvent {
    pub protocol_version: String,
    #[serde(with = "u64_string")]
    pub revision: u64,
    #[serde(rename = "type")]
    pub event_type: NativeEventType,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LatencyMode {
    Buffered,
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AllowPlugins {
    #[serde(rename = "signed-only", alias = "signedOnly")]
    SignedOnly,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceRatePolicy {
    #[serde(rename = "adapt-device", alias = "adaptDevice")]
    AdaptDevice,
    Resample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceChangePolicy {
    #[serde(rename = "follow-default", alias = "followDefault")]
    FollowDefault,
    Pause,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetronomeOptions {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
}

/// Explicit headless processing uses the real renderer and a simulated sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioBackend {
    Device,
    Simulated,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_ahead_blocks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_mode: Option<LatencyMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_plugins: Option<AllowPlugins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_backend: Option<AudioBackend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_rate_policy: Option<DeviceRatePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_change_policy: Option<DeviceChangePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metronome: Option<MetronomeOptions>,
}

/// Wall-clock seconds or integer sample frames (authoritative on the DSP
/// timeline). Frames serialize as decimal strings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum Timecode {
    Seconds {
        seconds: f64,
    },
    Frames {
        #[serde(with = "u64_string")]
        frames: u64,
    },
}

/// Render range boundary: exactly one of bar / beat / timecode / marker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum RenderPosition {
    Bar { bar: u32 },
    Beat { beat: Beat },
    Timecode(Timecode),
    Marker { marker: EntityId },
}

/// WAV bit depth: 16/24-bit PCM or 32-bit float (`"float32"` on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    Pcm16,
    Pcm24,
    Float32,
}

impl Serialize for BitDepth {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            BitDepth::Pcm16 => serializer.serialize_u8(16),
            BitDepth::Pcm24 => serializer.serialize_u8(24),
            BitDepth::Float32 => serializer.serialize_str("float32"),
        }
    }
}

impl<'de> Deserialize<'de> for BitDepth {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        match value.as_u64() {
            Some(16) => return Ok(BitDepth::Pcm16),
            Some(24) => return Ok(BitDepth::Pcm24),
            _ => {}
        }
        if value.as_str() == Some("float32") {
            return Ok(BitDepth::Float32);
        }
        Err(serde::de::Error::custom(
            "bitDepth must be 16, 24, or \"float32\"",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dither {
    Tpdf,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StemMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "mixer-channels")]
    MixerChannels,
    #[serde(rename = "tracks")]
    Tracks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<RenderPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<RenderPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respect_solo: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<BitDepth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dither: Option<Dither>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stems: Option<StemMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_metronome: Option<bool>,
    /// Base directory for resolving relative `SampleRef.assetUri` values
    /// during the render (absolute URIs are used as-is).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_base_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFileReport {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stem: Option<EntityId>,
    pub duration_seconds: f64,
    pub peak_dbfs: f64,
    pub true_peak_dbfs: f64,
    pub integrated_lufs: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderReport {
    pub files: Vec<RenderFileReport>,
    #[serde(with = "u64_string")]
    pub graph_latency_frames: u64,
}
