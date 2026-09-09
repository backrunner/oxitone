use crate::{
    document_wire::SourceNote, note_edit::NoteGesture, note_transform::NoteAction, piano_layout::*,
    piano_paint::Painter, theme::Theme, ui::alpha,
};
use gpui::*;
use std::collections::BTreeSet;

pub struct PaintedNote {
    pub note: SourceNote,
    pub selected: bool,
}
pub fn visible_notes(
    source: &[SourceNote],
    selected: &BTreeSet<usize>,
    gesture: Option<&NoteGesture>,
) -> Vec<PaintedNote> {
    let replaced: BTreeSet<_> = gesture
        .into_iter()
        .filter(|g| {
            !matches!(
                g.action,
                NoteAction::Insert
                    | NoteAction::InsertResize
                    | NoteAction::Duplicate
                    | NoteAction::Paint
            )
        })
        .flat_map(|g| g.changes.iter().filter_map(|c| c.index))
        .collect();
    let mut notes: Vec<_> = source
        .iter()
        .enumerate()
        .filter_map(|(index, note)| {
            if replaced.contains(&index) {
                return None;
            }
            Some(PaintedNote {
                note: note.clone(),
                selected: selected.contains(&index),
            })
        })
        .collect();
    if let Some(gesture) = gesture.filter(|g| g.action != NoteAction::Erase) {
        notes.extend(gesture.changes.iter().map(|change| PaintedNote {
            note: change.after.clone(),
            selected: gesture.select_after,
        }));
    }
    notes
}
pub fn paint_notes(
    p: &mut Painter<'_>,
    l: PianoLayout,
    theme: Theme,
    notes: &[PaintedNote],
    sounding: &[bool; 128],
    phase: Option<f64>,
) {
    for item in notes {
        let note = &item.note;
        let x = l.x(note.start);
        // Match grid and hit-test endpoints without adding a gap at the note tail.
        let width = (l.x(note.start + note.duration) - x).max(7.);
        let y = l.y(note.pitch);
        if x > KEY_WIDTH + l.grid_width
            || x + width < KEY_WIDTH
            || y > RULER + l.grid_height
            || y + l.key_height < RULER
        {
            continue;
        }
        let active = sounding[usize::from(note.pitch)]
            && phase.is_some_and(|beat| beat >= note.start && beat < note.start + note.duration);
        let color = if item.selected {
            theme.gold
        } else {
            theme.accent
        };
        let area = Bounds::new(
            p.origin + point(px(x), px(y + 1.5)),
            size(px(width), px((l.key_height - 3.).max(4.))),
        );
        p.window.paint_quad(quad(
            area,
            px(2.),
            alpha(color, 0.22 + note.velocity as f32 * 0.73),
            px(if item.selected { 1. } else { 0.5 }),
            alpha(if item.selected { theme.text } else { color }, 0.8),
            BorderStyle::default(),
        ));
        if active {
            p.rect(x, y + 1.5, width, 2., rgb(theme.text));
        }
        if width >= 38. && l.key_height >= 15. {
            p.text(
                x + 5.,
                y,
                note_name(note.pitch),
                if note.velocity < 0.45 {
                    theme.text
                } else {
                    theme.on_accent
                },
                l.key_height,
            );
        }
        if item.selected && width > 18. {
            p.rect(
                x + width - 4.,
                y + 4.,
                1.,
                (l.key_height - 8.).max(2.),
                alpha(theme.on_accent, 0.55),
            );
        }
    }
}
