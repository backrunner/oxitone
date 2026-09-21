//! Bounded cursor pages for plugin-reported gestures. No audio callback users.
use crate::{transport_wire::Transport, Result};
use serde::{Deserialize, Serialize};

pub const CAPACITY: usize = 4096;
pub const PAGE_SIZE: usize = 256;
pub const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Position {
    pub audio_sequence: u64,
    pub transport: Transport,
    pub reset: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Begin,
    Value,
    End,
    Sample,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordingMode {
    Touch,
    Write,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recording {
    pub mode: RecordingMode,
    pub parameter_ids: Vec<u32>,
    pub sample_rate: u32,
}
pub fn valid_selection(ids: &[u32]) -> bool {
    !ids.is_empty() && ids.len() <= 32 && ids.windows(2).all(|pair| pair[0] < pair[1])
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    pub sequence: u64,
    pub parameter_id: u32,
    pub kind: Kind,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub value: Option<f64>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub frames: Option<usize>,
    pub position: Position,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Recording,
    Stopping,
    Stopped,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Page {
    pub capture_id: String,
    pub status: Status,
    pub first_sequence: u64,
    pub next_sequence: u64,
    pub pending_events: usize,
    pub events: Vec<Event>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub recording: Option<Recording>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub end_position: Option<Position>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub error: Option<Failure>,
}
// An omitted optional field is valid; explicit JSON null is not in the TS contract.
fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
pub fn valid_id(value: &str) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|id| id > 0 && id.to_string() == value)
}
impl Page {
    pub fn validate(&self, ready: &crate::stream_wire::Ready) -> Result<()> {
        let valid_position = |p: Position| {
            p.audio_sequence <= MAX_SEQUENCE && p.transport.valid() && p.transport.playing
        };
        if !valid_id(&self.capture_id)
            || self.recording.as_ref().is_some_and(|recording| {
                !valid_selection(&recording.parameter_ids)
                    || recording.sample_rate != ready.sample_rate
                    || recording.parameter_ids.iter().any(|id| {
                        !ready
                            .parameters
                            .iter()
                            .any(|p| p.id == *id && p.writable && p.automatable)
                    })
            })
            || self.next_sequence > MAX_SEQUENCE
            || self.first_sequence > self.next_sequence
            || self.events.len() > PAGE_SIZE
            || self.pending_events > CAPACITY
            || self.first_sequence.saturating_add(self.events.len() as u64) > self.next_sequence
            || (self.status == Status::Failed) != self.error.is_some()
            || (self.status == Status::Stopped) != self.end_position.is_some()
            || (self.status == Status::Failed && !self.events.is_empty())
            || (self.error.is_none()
                && self.events.len()
                    != self
                        .next_sequence
                        .saturating_sub(self.first_sequence)
                        .min(PAGE_SIZE as u64) as usize)
            || self
                .events
                .windows(2)
                .any(|events| events[0].position.audio_sequence > events[1].position.audio_sequence)
            || self.end_position.is_some_and(|end| {
                self.events
                    .iter()
                    .any(|event| event.position.audio_sequence > end.audio_sequence)
            })
            || (matches!(self.status, Status::Failed | Status::Stopped) && self.pending_events != 0)
            || self.end_position.is_some_and(|p| !valid_position(p))
            || self.error.as_ref().is_some_and(|e| {
                !matches!(
                    e.code.as_str(),
                    "BudgetExceeded" | "PluginConfigInvalid" | "PluginRestartRequired"
                ) || e.message.is_empty()
            })
            || self.events.iter().enumerate().any(|(index, event)| {
                event.sequence != self.first_sequence + index as u64
                    || !valid_position(event.position)
                    || match event.kind {
                        Kind::Sample => {
                            event
                                .frames
                                .is_none_or(|frames| frames == 0 || frames > ready.block_size)
                                || self
                                    .recording
                                    .as_ref()
                                    .is_none_or(|r| !r.parameter_ids.contains(&event.parameter_id))
                                || self.end_position.is_some_and(|end| {
                                    event.position.audio_sequence >= end.audio_sequence
                                })
                        }
                        _ => event.frames.is_some(),
                    }
                    || !ready
                        .parameters
                        .iter()
                        .any(|p| p.id == event.parameter_id && p.writable && p.automatable)
                    || match event.kind {
                        Kind::Value | Kind::Sample => event
                            .value
                            .is_none_or(|v| !v.is_finite() || !(0.0..=1.0).contains(&v)),
                        _ => event.value.is_some(),
                    }
            })
        {
            return Err(crate::invalid("invalid VST3 gesture page"));
        }
        Ok(())
    }
}
