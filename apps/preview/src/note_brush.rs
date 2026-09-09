//! Paint and erase collect a stroke locally; mouse-up submits a single undoable edit.
use crate::{
    document_wire::SourceNote,
    note_edit::NoteChange,
    note_transform::NoteAction,
    piano_layout::{PianoLayout, KEY_WIDTH, RULER},
    ui::Preview,
};
use gpui::*;
use std::collections::BTreeSet;

/// Traverse grid boundaries instead of sampling mouse events; fast vertical strokes keep all pitches.
pub fn stroke_cells(a: (f64, f64), b: (f64, f64)) -> BTreeSet<(i64, i64)> {
    let (mut x, mut y) = (a.0.floor() as i64, a.1.floor() as i64);
    let end = (b.0.floor() as i64, b.1.floor() as i64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (sx, sy) = (if dx > 0. { 1 } else { -1 }, if dy > 0. { 1 } else { -1 });
    let boundary = |v: f64, cell: i64, d: f64, step: i64| {
        if d.abs() < 1e-10 {
            f64::INFINITY
        } else {
            (cell as f64 + if step > 0 { 1. } else { 0. } - v) / d
        }
    };
    let (mut tx, mut ty) = (boundary(a.0, x, dx, sx), boundary(a.1, y, dy, sy));
    let mut cells = BTreeSet::new();
    for _ in 0..4096 {
        cells.insert((x, y));
        if (x, y) == end {
            break;
        }
        if (tx - ty).abs() < 1e-10 {
            x += sx;
            y += sy;
            tx += 1. / dx.abs();
            ty += 1. / dy.abs();
        } else if tx < ty {
            x += sx;
            tx += 1. / dx.abs();
        } else {
            y += sy;
            ty += 1. / dy.abs();
        }
    }
    cells
}

fn intersects(a: (f32, f32), b: (f32, f32), rect: (f32, f32, f32, f32)) -> bool {
    let mut low = 0_f32;
    let mut high = 1_f32;
    for (origin, delta, min, max) in [
        (a.0, b.0 - a.0, rect.0, rect.2),
        (a.1, b.1 - a.1, rect.1, rect.3),
    ] {
        if delta.abs() < 0.0001 {
            if origin < min || origin > max {
                return false;
            }
        } else {
            let first = (min - origin) / delta;
            let last = (max - origin) / delta;
            low = low.max(first.min(last));
            high = high.min(first.max(last));
            if low > high {
                return false;
            }
        }
    }
    true
}
impl Preview {
    pub fn brush_notes(&mut self, position: Point<Pixels>, layout: PianoLayout) {
        let Some(site) = self.pattern_site().cloned() else {
            return;
        };
        let Some(gesture) = &mut self.document.gesture else {
            return;
        };
        let origin = self.piano.origin.get();
        let a = gesture.pointer - origin;
        let b = position - origin;
        let a = (f32::from(a.x), f32::from(a.y));
        let b = (f32::from(b.x), f32::from(b.y));
        if b.0 < KEY_WIDTH || b.1 < RULER || b.1 >= RULER + layout.grid_height {
            return;
        }
        if gesture.action == NoteAction::Erase {
            for (index, output) in site.outputs.iter().enumerate() {
                let note = &output.note;
                if !gesture.changes.iter().any(|c| c.index == Some(index))
                    && intersects(
                        a,
                        b,
                        (
                            layout.x(note.start),
                            layout.y(note.pitch),
                            layout
                                .x(note.start + note.duration)
                                .max(layout.x(note.start) + 7.),
                            layout.y(note.pitch) + layout.key_height,
                        ),
                    )
                {
                    gesture.changes.push(NoteChange {
                        index: Some(index),
                        before: note.clone(),
                        after: note.clone(),
                        selector: output.select.clone(),
                    });
                }
            }
        } else {
            let step = self
                .piano
                .effective_snap(false)
                .step()
                .max(self.piano.note_length);
            let musical = |p: (f32, f32)| {
                (
                    layout.beat(p.0) / step,
                    f64::from((p.1 - RULER + layout.scroll_y) / layout.key_height),
                )
            };
            let mut occupied: BTreeSet<_> = site
                .outputs
                .iter()
                .map(|o| ((o.note.start * 960.).round() as i64, o.note.pitch))
                .chain(
                    gesture
                        .changes
                        .iter()
                        .map(|c| ((c.after.start * 960.).round() as i64, c.after.pitch)),
                )
                .collect();
            for (index, row) in stroke_cells(musical(a), musical(b)) {
                if gesture.changes.len() >= 4096 {
                    break;
                }
                let start = index as f64 * step;
                if start < 0. || !(0..128).contains(&row) {
                    continue;
                }
                let pitch = (127 - row) as u8;
                if !occupied.insert(((start * 960.).round() as i64, pitch)) {
                    continue;
                }
                let note = SourceNote {
                    pitch,
                    start,
                    duration: self.piano.note_length,
                    velocity: 0.7,
                };
                gesture.changes.push(NoteChange {
                    index: None,
                    before: note.clone(),
                    after: note,
                    selector: serde_json::Value::Null,
                });
            }
        }
        gesture.pointer = position;
    }
}

#[cfg(test)]
mod tests {
    use super::{intersects, stroke_cells};
    #[test]
    fn brush_fills_fast_vertical_and_diagonal_strokes_in_both_directions() {
        let vertical = stroke_cells((2.5, 20.5), (2.5, 30.5));
        assert_eq!(vertical.len(), 11);
        for row in 20..=30 {
            assert!(vertical.contains(&(2, row)));
        }
        let diagonal = stroke_cells((0.5, 0.5), (10.5, 10.5));
        assert_eq!(diagonal.len(), 11);
        assert_eq!(diagonal, stroke_cells((10.5, 10.5), (0.5, 0.5)));
        let mut segmented = stroke_cells((0.5, 0.5), (5.5, 5.5));
        segmented.extend(stroke_cells((5.5, 5.5), (10.5, 10.5)));
        assert_eq!(segmented, diagonal);
        assert_eq!(stroke_cells((0.5, 0.5), (1e6, 0.5)).len(), 4096);
    }
    #[test]
    fn fast_eraser_strokes_hit_crossed_notes_but_not_the_whole_bounding_box() {
        assert!(intersects((0., 0.), (100., 100.), (49., 49., 51., 51.)));
        assert!(!intersects((0., 0.), (100., 100.), (49., 10., 51., 12.)));
        assert!(intersects((0., 5.), (100., 5.), (49., 4., 51., 6.)));
        assert!(!intersects((0., 5.), (100., 5.), (49., 6., 51., 9.)));
    }
}
