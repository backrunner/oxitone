//! Scheduled event types for the transport scheduler
//! (`.agents/docs/03-audio-runtime-spec.md` §时间和调度).

use oxitone_core::wire::EntityId;

/// Dispatch priority within one sample frame. Ordering is fixed by the
/// runtime spec: stop < note-off < param < note-on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Stop,
    NoteOff,
    Param,
    NoteOn,
}

/// Payload carried by a scheduled event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventPayload {
    NoteOn { pitch: u8, velocity: f64 },
    NoteOff { pitch: u8, velocity: f64 },
}

/// One event on the sample-frame timeline. Events sort by
/// `(frame, priority, sequence)`; `sequence` is the deterministic creation
/// ordinal assigned during clip expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledEvent {
    pub frame: u64,
    pub priority: EventPriority,
    pub sequence: u64,
    pub track_id: EntityId,
    pub channel_id: EntityId,
    pub payload: EventPayload,
}
