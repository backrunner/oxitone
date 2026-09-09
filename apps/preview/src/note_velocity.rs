//! Absolute velocity strokes, interpolated between pointer events and committed once.
use crate::{
    document_wire::SourceOutput,
    note_edit::NoteChange,
    piano_layout::{PianoLayout, KEY_WIDTH, RULER},
    ui::Preview,
};
use gpui::{Pixels, Point};
use std::collections::{BTreeSet, HashMap};

pub fn velocity_at(layout: PianoLayout, y: f32) -> f64 {
    let baseline = RULER + layout.grid_height + layout.velocity_height - 5.;
    f64::from(((baseline - y) / (layout.velocity_height - 12.)).clamp(0., 1.))
}

/// A stem has a usable horizontal target; the next onset limits overlap.
pub fn stroke(
    outputs: &[SourceOutput],
    selected: &BTreeSet<usize>,
    changes: &mut Vec<NoteChange>,
    a: (f32, f32),
    b: (f32, f32),
    layout: PianoLayout,
) {
    let mut starts: Vec<_> = outputs.iter().map(|o| o.note.start).collect();
    starts.sort_by(f64::total_cmp);
    starts.dedup();
    let mut changed: HashMap<_, _> = changes
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.index.map(|index| (index, i)))
        .collect();
    for (index, output) in outputs.iter().enumerate() {
        if !selected.is_empty() && !selected.contains(&index) {
            continue;
        }
        let x = layout.x(output.note.start);
        let next = starts.partition_point(|start| *start <= output.note.start);
        let right = starts
            .get(next)
            .map_or(x + 24., |beat| layout.x(*beat))
            .min(x + 24.);
        if b.0.max(a.0) < x - 5. || b.0.min(a.0) > right {
            continue;
        }
        let sample = if b.0 >= x - 5. && b.0 <= right {
            b.0
        } else {
            x.clamp(a.0.min(b.0), a.0.max(b.0))
        };
        let t = if (b.0 - a.0).abs() < 0.001 {
            1.
        } else {
            ((sample - a.0) / (b.0 - a.0)).clamp(0., 1.)
        };
        let velocity = velocity_at(layout, a.1 + (b.1 - a.1) * t);
        let change = *changed.entry(index).or_insert_with(|| {
            changes.push(NoteChange {
                index: Some(index),
                before: output.note.clone(),
                after: output.note.clone(),
                selector: output.select.clone(),
            });
            changes.len() - 1
        });
        changes[change].after.velocity = velocity;
    }
}

impl Preview {
    pub fn draw_velocity(&mut self, position: Point<Pixels>, layout: PianoLayout) {
        let Some(mut gesture) = self.document.gesture.take() else {
            return;
        };
        let Some(site) = self.pattern_site() else {
            self.document.gesture = Some(gesture);
            return;
        };
        let origin = self.piano.origin.get();
        let a = gesture.pointer - origin;
        let b = position - origin;
        let empty = BTreeSet::new();
        let selected = if self.document.notes.site.as_deref() == Some(&site.handle) {
            &self.document.notes.indices
        } else {
            &empty
        };
        // Include untouched selected notes so selection survives revision-local output reordering.
        if gesture.changes.is_empty() {
            gesture.changes.extend(selected.iter().filter_map(|&index| {
                site.outputs.get(index).map(|o| NoteChange {
                    index: Some(index),
                    before: o.note.clone(),
                    after: o.note.clone(),
                    selector: o.select.clone(),
                })
            }));
        }
        stroke(
            &site.outputs,
            selected,
            &mut gesture.changes,
            (f32::from(a.x).max(KEY_WIDTH), f32::from(a.y)),
            (
                f32::from(b.x).clamp(KEY_WIDTH, KEY_WIDTH + layout.grid_width),
                f32::from(b.y),
            ),
            layout,
        );
        gesture.pointer = position;
        self.document.gesture = Some(gesture);
    }
}
