//! Swing-aware note dispatch (03-audio-runtime-spec.md §Channel swing). The
//! scheduler is compiled with swing 0; at dispatch time every note event's
//! beat is reconstructed from its frame, tested against the 1/16-beat grid,
//! and shifted by `swing × 0.125` beat using the channel's *current* swing
//! (static or the automation lane's value at the event's beat — control
//! rate, read per event). Note-offs reuse the shift recorded for their
//! note-on so swing preserves durations.
//!
//! Beat reconstruction snaps `frame_to_beat` (exact inverse up to frame
//! quantization, < 1e-4 beat) onto a 1/2048-beat grid: beats authored on
//! the 1/16 swing grid are recovered exactly, off-grid beats stay off-grid.
//!
//! Dispatch queries `lookback` frames before the block so events swung
//! forward into the block are caught; events swung out of the block are
//! picked up by a later block. Without any swing (static or automated) the
//! fast path dispatches the pre-sorted range directly.

use oxitone_graph::compile::RenderPlan;
use oxitone_graph::{NoteEvent, NoteEventKind};
use oxitone_transport::event::{EventPayload, ScheduledEvent};

use crate::channel::ChannelNode;

/// Beat-grid snap for reconstruction (power of two: exact in binary FP).
const SNAP: f64 = 2048.0;

/// Reconstruct the authored beat of an unswung event frame.
fn reconstruct_beat(plan: &RenderPlan, frame: u64) -> f64 {
    let beat = plan.tempo.frame_to_beat(frame).to_f64();
    (beat * SNAP).round() / SNAP
}

/// Whether a beat sits on an odd 1/16-grid position (the swingable beats).
fn on_weak_grid(beat: f64) -> bool {
    let grid = (beat * 4.0).round();
    (beat * 4.0 - grid).abs() < 1e-6 && (grid as i64) % 2 != 0
}

/// Swing shift in frames for one event.
fn shift_frames(plan: &RenderPlan, event: &ScheduledEvent, swing: f64) -> i64 {
    if swing <= 0.0 {
        return 0;
    }
    let beat = reconstruct_beat(plan, event.frame);
    if !on_weak_grid(beat) {
        return 0;
    }
    let shifted = plan.tempo.beat_to_frame(
        oxitone_core::Beat::from_f64(beat + swing * 0.125).unwrap_or(oxitone_core::Beat::ZERO),
    );
    shifted as i64 - event.frame as i64
}

/// Maximum forward shift in frames: `0.125` beat at the lowest BPM (20).
fn lookback_frames(sample_rate: u32) -> u64 {
    (0.125 * 60.0 / 20.0 * f64::from(sample_rate)).ceil() as u64
}

/// Per-graph dispatch state (preallocated scratch).
pub struct Dispatcher {
    lookback: u64,
    /// (absolute frame, event index into the ranged slice) pending sort.
    staged: Vec<(u64, usize)>,
}

impl Dispatcher {
    pub fn new(sample_rate: u32, event_capacity: usize) -> Self {
        Self {
            lookback: lookback_frames(sample_rate),
            staged: Vec::with_capacity(event_capacity),
        }
    }

    /// Fill each channel's `notes` staging with the block's events.
    /// `swing_at(channel_index, beat)` yields the effective swing.
    pub fn dispatch(
        &mut self,
        plan: &RenderPlan,
        channels: &mut [ChannelNode],
        channel_index: &std::collections::BTreeMap<String, usize>,
        frame_start: u64,
        frame_end: u64,
        mut swing_at: impl FnMut(usize, f64) -> f64,
    ) {
        if !plan.has_swing {
            for event in plan.scheduler.events_in_range(frame_start, frame_end) {
                Self::deliver(
                    channels,
                    channel_index,
                    event,
                    event.frame,
                    frame_start,
                    0,
                    false,
                );
            }
            return;
        }
        let query_start = frame_start.saturating_sub(self.lookback);
        let ranged = plan.scheduler.events_in_range(query_start, frame_end);
        self.staged.clear();
        for (idx, event) in ranged.iter().enumerate() {
            let Some(&ch) = channel_index.get(event.channel_id.as_str()) else {
                continue;
            };
            let beat = reconstruct_beat(plan, event.frame);
            let swing = swing_at(ch, beat).clamp(0.0, 1.0);
            let (absolute, _shift) = match event.payload {
                EventPayload::NoteOn { .. } => {
                    let shift = shift_frames(plan, event, swing);
                    (event.frame as i64 + shift, shift)
                }
                EventPayload::NoteOff { pitch, .. } => {
                    // Reuse the note-on shift so the duration is preserved.
                    let shift = if channels[ch].note_active[pitch as usize] {
                        channels[ch].note_shift[pitch as usize]
                    } else {
                        0
                    };
                    (event.frame as i64 + shift, shift)
                }
            };
            if absolute >= frame_start as i64 && (absolute as u64) < frame_end {
                self.staged.push((absolute as u64, idx));
            }
        }
        let events = ranged;
        self.staged.sort_unstable_by(|&(fa, ia), &(fb, ib)| {
            (fa, events[ia].priority, events[ia].sequence).cmp(&(
                fb,
                events[ib].priority,
                events[ib].sequence,
            ))
        });
        for &(absolute, idx) in &self.staged {
            let event = &events[idx];
            let shift = absolute as i64 - event.frame as i64;
            Self::deliver(
                channels,
                channel_index,
                event,
                absolute,
                frame_start,
                shift,
                true,
            );
        }
    }

    fn deliver(
        channels: &mut [ChannelNode],
        channel_index: &std::collections::BTreeMap<String, usize>,
        event: &ScheduledEvent,
        absolute: u64,
        frame_start: u64,
        shift: i64,
        record: bool,
    ) {
        let Some(&ch) = channel_index.get(event.channel_id.as_str()) else {
            return;
        };
        let offset = absolute.saturating_sub(frame_start) as u32;
        let node = &mut channels[ch];
        match event.payload {
            EventPayload::NoteOn { pitch, velocity } => {
                if record {
                    node.note_active[pitch as usize] = true;
                    node.note_shift[pitch as usize] = shift;
                }
                node.notes.push(NoteEvent {
                    frame_offset: offset,
                    kind: NoteEventKind::NoteOn,
                    pitch,
                    velocity: velocity as f32,
                });
            }
            EventPayload::NoteOff { pitch, velocity } => {
                if record {
                    node.note_active[pitch as usize] = false;
                    node.note_shift[pitch as usize] = 0;
                }
                node.notes.push(NoteEvent {
                    frame_offset: offset,
                    kind: NoteEventKind::NoteOff,
                    pitch,
                    velocity: velocity as f32,
                });
            }
        }
    }
}
