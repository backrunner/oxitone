//! oxitone-transport — tempo map, time-signature map, and deterministic
//! note-event scheduling for the Oxitone engine. See
//! `.agents/docs/02-domain-spec.md` (Project 与时间轴、Track/Pattern/PatternClip)
//! and `.agents/docs/03-audio-runtime-spec.md` (时间和调度).

pub mod automation;
pub mod event;
pub mod scheduler;
pub mod tempo;
pub mod timesig;

use std::cmp::Ordering;

use oxitone_core::beat::Beat;

pub use automation::{
    bake_tempo_lane, bake_tempo_lane_spec, ensure_transport_invariant, find_tempo_lane,
    normalized_to_bpm, CompiledAutomation, EvalContext, MAX_DEPTH as AUTOMATION_MAX_DEPTH,
    MAX_NODES as AUTOMATION_MAX_NODES, TEMPO_BAKE_GRID_BEAT, TEMPO_BAKE_MAX_SEGMENTS,
};
pub use event::{EventPayload, EventPriority, ScheduledEvent};
pub use scheduler::{ClipSource, Scheduler};
pub use tempo::{CompiledTempoMap, TempoMap};
pub use timesig::{CompiledTimeSignatureMap, TimeSignatureMap};

pub(crate) fn beat_cmp(a: Beat, b: Beat) -> Ordering {
    (a.numerator() as i128 * b.denominator() as i128)
        .cmp(&(b.numerator() as i128 * a.denominator() as i128))
}
