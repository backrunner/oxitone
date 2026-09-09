//! Musical gesture math is independent of GPUI and the document transport.
use crate::{document_wire::SourceNote, piano_layout::Snap};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteAction {
    Move,
    Resize,
    Velocity,
    Insert,
    InsertResize,
    Duplicate,
    Paint,
    Erase,
}

pub fn transform(
    notes: &[SourceNote],
    action: NoteAction,
    beats: f64,
    pitches: i32,
    snap: Snap,
) -> Vec<SourceNote> {
    let mut shift = snap.round(beats);
    let mut transpose = pitches;
    if matches!(
        action,
        NoteAction::Move | NoteAction::Duplicate | NoteAction::Insert
    ) {
        let first = notes.iter().map(|n| n.start).fold(f64::INFINITY, f64::min);
        shift = shift.max(-first);
        let low = notes.iter().map(|n| n.pitch).min().unwrap_or(0) as i32;
        let high = notes.iter().map(|n| n.pitch).max().unwrap_or(127) as i32;
        transpose = transpose.clamp(-low, 127 - high);
    }
    if action == NoteAction::Resize {
        let shortest = notes
            .iter()
            .map(|n| n.duration)
            .fold(f64::INFINITY, f64::min);
        shift = shift.max(snap.step().min(0.0625) - shortest);
    }
    notes
        .iter()
        .map(|note| {
            let mut next = note.clone();
            match action {
                NoteAction::Move | NoteAction::Duplicate | NoteAction::Insert => {
                    next.start += shift;
                    next.pitch = (i32::from(next.pitch) + transpose) as u8;
                }
                NoteAction::Resize | NoteAction::InsertResize => {
                    next.duration = (next.duration + shift).max(snap.step().min(0.0625))
                }
                NoteAction::Velocity | NoteAction::Paint | NoteAction::Erase => {}
            }
            next
        })
        .collect()
}

pub fn clamp_start_delta(notes: &[SourceNote], beats: f64, snap: Snap) -> f64 {
    let first = notes.iter().map(|n| n.start).fold(f64::INFINITY, f64::min);
    snap.round(beats).max(-first)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(start: f64, pitch: u8, duration: f64) -> SourceNote {
        SourceNote {
            start,
            pitch,
            duration,
            velocity: 0.7,
        }
    }
    #[test]
    fn draw_places_before_release_while_shift_draw_stretches() {
        let notes = [note(1., 60, 0.5)];
        let placed = transform(&notes, NoteAction::Insert, 0.5, 4, Snap::Quarter);
        assert_eq!(
            (placed[0].start, placed[0].pitch, placed[0].duration),
            (1.5, 64, 0.5)
        );
        let stretched = transform(&notes, NoteAction::InsertResize, 0.5, 4, Snap::Quarter);
        assert_eq!(
            (
                stretched[0].start,
                stretched[0].pitch,
                stretched[0].duration
            ),
            (1., 60, 1.)
        );
    }
    #[test]
    fn group_move_preserves_intervals_at_time_and_pitch_boundaries() {
        let notes = [note(0.25, 4, 0.5), note(1.25, 124, 1.)];
        let moved = transform(&notes, NoteAction::Move, -3., 24, Snap::Quarter);
        assert_eq!((moved[0].start, moved[1].start), (0., 1.));
        assert_eq!((moved[0].pitch, moved[1].pitch), (7, 127));
        let down = transform(&notes, NoteAction::Move, 0.37, -24, Snap::Quarter);
        assert_eq!((down[0].start, down[0].pitch, down[1].pitch), (0.5, 0, 120));
    }
    #[test]
    fn resize_preserves_relative_lengths_and_fine_snap_is_available() {
        let notes = [note(0., 60, 0.25), note(1., 64, 1.)];
        let resized = transform(&notes, NoteAction::Resize, -2., 0, Snap::Quarter);
        assert_eq!((resized[0].duration, resized[1].duration), (0.0625, 0.8125));
        let fine = transform(&notes, NoteAction::Move, 0.1, 0, Snap::Free);
        assert!((fine[0].start - 0.1).abs() < 0.001);
    }
    #[test]
    fn group_start_is_open_ended_and_clamps_only_at_zero() {
        let notes = [note(1., 60, 1.), note(3., 64, 1.)];
        assert_eq!(super::clamp_start_delta(&notes, 4., Snap::Quarter), 4.);
        assert_eq!(super::clamp_start_delta(&notes, -4., Snap::Quarter), -1.);
    }
}
