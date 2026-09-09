//! Authoring wire types: notes, patterns, clips, channels, mixer and
//! automation lanes (04-api-contracts.md §Authoring interfaces).

use serde::{Deserialize, Serialize};

use super::automation::AutomationSourceSpec;
use super::basic::{EffectRef, InstrumentRef, LoopSpec};
use super::EntityId;
use crate::beat::Beat;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<EntityId>,
    pub pitch: u8,
    pub start: Beat,
    pub duration: Beat,
    pub velocity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_velocity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternSpec {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub length_beats: Beat,
    pub notes: Vec<NoteSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<PatternPartSpec>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternPartSpec {
    pub channel_id: EntityId,
    pub pattern_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternClipSpec {
    pub id: EntityId,
    pub pattern_id: EntityId,
    pub track_id: EntityId,
    pub start_beat: Beat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_beats: Option<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat: Option<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transpose: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TempoSync {
    Off,
    Stretch,
    Repitch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleClipSpec {
    pub id: EntityId,
    pub sample_id: EntityId,
    pub track_id: EntityId,
    pub start_beat: Beat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_beats: Option<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pan: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_spec: Option<LoopSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo_sync: Option<TempoSync>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretch_algorithm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSpec {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub instrument: InstrumentRef,
    pub effect_chain: Vec<EffectRef>,
    pub level: f64,
    pub pan: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swing: Option<f64>,
    pub mixer_channel_id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solo: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendSpec {
    pub destination_id: EntityId,
    pub ratio: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_fader: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidechain: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixerChannelSpec {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub level: f64,
    pub balance: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_send_ratio: Option<f64>,
    pub inserts: Vec<EffectRef>,
    pub sends: Vec<SendSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solo: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutomationCombine {
    Replace,
    Add,
    Multiply,
    Max,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationTarget {
    pub entity_id: EntityId,
    pub parameter_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ParameterScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterScope {
    Plugin,
    EffectHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutomationPlayback {
    Global,
    Playlist,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationLaneSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback: Option<AutomationPlayback>,
    pub id: EntityId,
    pub target: AutomationTarget,
    pub source: AutomationSourceSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combine: Option<AutomationCombine>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_spec: Option<LoopSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat: Option<Beat>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationClipSpec {
    pub id: EntityId,
    pub lane_id: EntityId,
    pub track_id: EntityId,
    pub start_beat: Beat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_beats: Option<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}
