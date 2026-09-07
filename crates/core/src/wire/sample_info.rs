//! Versioned control-thread sample inspection. No audio buffers cross the ABI.
use serde::{Deserialize, Serialize};

use super::SampleFormat;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectSampleRequest {
    pub protocol_version: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SampleChannelLayoutAction {
    Kept,
    DownmixedToStereo,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleInfo {
    pub protocol_version: String,
    pub sha256: String,
    pub format: SampleFormat,
    pub sample_rate: u32,
    pub channels: u8,
    #[serde(with = "super::serde_util::u64_string")]
    pub frames: u64,
    pub source_channels: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_bit_depth: Option<u8>,
    pub decoder: String,
    pub channel_layout_action: SampleChannelLayoutAction,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum SampleCacheEncoding {
    #[serde(rename = "wav-f32-v1")]
    WavF32V1,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleProvenance {
    pub source_sha256: String,
    pub source_format: SampleFormat,
    pub source_sample_rate: u32,
    pub source_channels: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_bit_depth: Option<u8>,
    pub decoder: String,
    pub channel_layout_action: SampleChannelLayoutAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_encoding: Option<SampleCacheEncoding>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSampleRequest {
    pub protocol_version: String,
    pub path: String,
    pub cache_dir: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedSampleInfo {
    pub protocol_version: String,
    pub path: String,
    pub sha256: String,
    pub format: SampleFormat,
    pub sample_rate: u32,
    pub channels: u8,
    #[serde(with = "super::serde_util::u64_string")]
    pub frames: u64,
    pub provenance: SampleProvenance,
}
