//! Basic wire types: parameter specs, curves/points, timeline segments,
//! track and asset references (04-api-contracts.md §基础类型).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::serde_util::{opt_u64_string, u64_string};
use super::EntityId;
use crate::beat::Beat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterUnit {
    Normalized,
    Db,
    Hz,
    Semitones,
    Seconds,
    Beats,
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterSmoothing {
    None,
    Linear,
    #[serde(rename = "one-pole", alias = "onePole")]
    OnePole,
}

#[cfg(test)]
mod smoothing_tests {
    use super::ParameterSmoothing;
    #[test]
    fn one_pole_uses_public_wire_spelling_and_reads_legacy_spelling() {
        assert_eq!(
            serde_json::to_string(&ParameterSmoothing::OnePole).unwrap(),
            "\"one-pole\""
        );
        for wire in ["\"one-pole\"", "\"onePole\""] {
            assert_eq!(
                serde_json::from_str::<ParameterSmoothing>(wire).unwrap(),
                ParameterSmoothing::OnePole
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterRate {
    Control,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterMapping {
    Linear,
    Log,
    Bipolar,
    Enum,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterSpec {
    pub id: String,
    pub label: String,
    pub unit: ParameterUnit,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub smoothing: ParameterSmoothing,
    pub rate: ParameterRate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<ParameterMapping>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CurveKind {
    Step,
    Linear,
    Smooth,
    Exponential,
    Bezier,
}

/// Interpolation between two automation points. Simple kinds serialize as
/// `{"kind": "..."}` (empty struct variants) to match the wire object form.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Curve {
    Step {},
    Linear {},
    Smooth {},
    Exponential {},
    Bezier { out: [f64; 2], r#in: [f64; 2] },
}

impl Curve {
    pub fn kind(&self) -> CurveKind {
        match self {
            Curve::Step {} => CurveKind::Step,
            Curve::Linear {} => CurveKind::Linear,
            Curve::Smooth {} => CurveKind::Smooth,
            Curve::Exponential {} => CurveKind::Exponential,
            Curve::Bezier { .. } => CurveKind::Bezier,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationPoint {
    pub beat: Beat,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<Curve>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TempoCurve {
    Step,
    Linear,
    Exponential,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoSegment {
    pub start_beat: Beat,
    pub bpm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<TempoCurve>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSignatureSegment {
    pub start_bar: u32,
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSpec {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub channel_ids: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<f64>,
    pub pattern_clip_ids: Vec<EntityId>,
    pub sample_clip_ids: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_channel: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_beat: Option<Beat>,
    pub length_beats: Beat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat: Option<Beat>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerSpec {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub start_beat: Beat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FadeCurve {
    Linear,
    EqualPower,
    Exponential,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FadeSpec {
    #[serde(with = "u64_string")]
    pub length_frames: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<FadeCurve>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizeSpec {
    pub peak_db: f64,
}

/// Non-destructive sample edit descriptor (baked at prepare, not automatable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleEditSpec {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "opt_u64_string"
    )]
    pub start_frame: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "opt_u64_string"
    )]
    pub end_frame: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalize: Option<NormalizeSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<FadeSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<FadeSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossfade: Option<FadeSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SampleFormat {
    Wav,
    Aiff,
    Flac,
    Mp3,
    Mp4,
    M4a,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleRef {
    pub id: EntityId,
    pub asset_uri: String,
    pub sha256: String,
    pub format: SampleFormat,
    pub sample_rate: u32,
    pub channels: u8,
    #[serde(with = "u64_string")]
    pub frames: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<SampleEditSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub musical_length_beats: Option<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<super::SampleProvenance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentRef {
    pub plugin_id: String,
    pub plugin_version: String,
    pub parameters: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<BTreeMap<String, String>>,
    /// Plugin-declared structured state (e.g. Slicer slice table); fixed at
    /// compile time, immutable while playing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectRef {
    pub plugin_id: String,
    pub plugin_version: String,
    pub parameters: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix: Option<f64>,
}
