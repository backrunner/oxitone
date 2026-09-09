//! One gesture produces one semantic document transaction, with a local preview.
use crate::{
    document_wire::SourceNote,
    note_transform::{transform, NoteAction},
    piano_layout::{NoteTool, KEY_WIDTH, RULER, SCROLLBAR},
    ui::Preview,
};
use gpui::*;
use serde_json::Value;

pub use crate::note_gesture::{NoteChange, NoteGesture};
impl Preview {
    pub fn press_note(&mut self, event: &MouseDownEvent) -> bool {
        if self.document.view.is_none() {
            return false;
        }
        let Some(layout) = self.piano_layout() else {
            return false;
        };
        let at = event.position - self.piano.origin.get();
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        if x < KEY_WIDTH
            || x >= layout.width - SCROLLBAR
            || y < RULER
            || y >= layout.height - SCROLLBAR
        {
            return false;
        }
        // Busy source edits must never fall through into transport actions.
        if !self.document_ready() {
            return true;
        }
        let Some(site) = self.pattern_site().cloned() else {
            return true;
        };
        let velocity = y >= RULER + layout.grid_height;
        if velocity {
            if event.button == MouseButton::Left {
                self.document.gesture = Some(NoteGesture {
                    select_after: self.document.notes.site.as_ref() == Some(&site.handle)
                        && !self.document.notes.indices.is_empty(),
                    site: site.handle,
                    placement: self.edit_placement(),
                    changes: vec![],
                    action: NoteAction::Velocity,
                    pointer: event.position,
                    scroll: (layout.scroll_x, layout.scroll_y),
                });
                self.draw_velocity(event.position, layout);
            }
            return true;
        }
        let hit = site.outputs.iter().enumerate().rev().find(|(_, output)| {
            let note = &output.note;
            x >= layout.x(note.start)
                && x <= layout
                    .x(note.start + note.duration)
                    .max(layout.x(note.start) + 7.)
                && y >= layout.y(note.pitch)
                && y < layout.y(note.pitch) + layout.key_height
        });
        if event.button == MouseButton::Right
            || (self.piano.tool == NoteTool::Paint
                && !event.modifiers.platform
                && !event.modifiers.control)
        {
            self.document.gesture = Some(NoteGesture {
                select_after: true,
                site: site.handle.clone(),
                placement: self.edit_placement(),
                changes: vec![],
                action: if event.button == MouseButton::Right {
                    NoteAction::Erase
                } else {
                    NoteAction::Paint
                },
                pointer: event.position,
                scroll: (layout.scroll_x, layout.scroll_y),
            });
            self.brush_notes(event.position, layout);
            return true;
        }
        let command = event.modifiers.platform || event.modifiers.control;
        if let Some((index, output)) = hit {
            let selected = self.document.notes.selected(&site.handle, index);
            if command {
                self.document.notes.toggle(site.handle.clone(), index);
                return true;
            }
            if !selected {
                self.document.notes.select(site.handle.clone(), [index]);
            }
            let action = if event.modifiers.shift {
                NoteAction::Duplicate
            } else if x > layout.x(output.note.start + output.note.duration)
                - (layout.beat_width * output.note.duration as f32 * 0.3).clamp(2., 8.)
            {
                NoteAction::Resize
            } else {
                NoteAction::Move
            };
            self.piano.note_length = output.note.duration;
            let changes = self
                .document
                .notes
                .indices
                .iter()
                .filter_map(|&index| {
                    site.outputs.get(index).map(|output| NoteChange {
                        index: Some(index),
                        before: output.note.clone(),
                        after: output.note.clone(),
                        selector: output.select.clone(),
                    })
                })
                .collect();
            self.document.gesture = Some(NoteGesture {
                select_after: true,
                site: site.handle,
                placement: self.edit_placement(),
                changes,
                action,
                pointer: event.position,
                scroll: (layout.scroll_x, layout.scroll_y),
            });
        } else if command || self.piano.tool == NoteTool::Select {
            self.start_note_marquee(site.handle, event, layout);
        } else {
            self.document.notes.clear();
            let pitch = (127. - ((y - RULER + layout.scroll_y) / layout.key_height).floor())
                .clamp(0., 127.) as u8;
            let step = self.piano.effective_snap(event.modifiers.alt).step();
            let start = (layout.beat(x) / step).floor() * step;
            let note = SourceNote {
                pitch,
                start,
                duration: self.piano.note_length,
                velocity: 0.7,
            };
            self.document.gesture = Some(NoteGesture {
                select_after: true,
                site: site.handle,
                placement: self.edit_placement(),
                changes: vec![NoteChange {
                    index: None,
                    before: note.clone(),
                    after: note,
                    selector: Value::Null,
                }],
                action: if event.modifiers.shift {
                    NoteAction::InsertResize
                } else {
                    NoteAction::Insert
                },
                pointer: event.position,
                scroll: (layout.scroll_x, layout.scroll_y),
            });
        }
        true
    }

    pub fn move_note(&mut self, event: &MouseMoveEvent) {
        let Some(mut layout) = self.piano_layout() else {
            return;
        };
        if self.document.notes.marquee.is_some() {
            self.move_note_marquee(event.position, layout);
            return;
        }
        let Some(gesture) = &self.document.gesture else {
            return;
        };
        let button = if gesture.action == NoteAction::Erase {
            MouseButton::Right
        } else {
            MouseButton::Left
        };
        if event.pressed_button != Some(button) {
            return;
        }
        if gesture.action == NoteAction::Velocity {
            self.draw_velocity(event.position, layout);
            return;
        }
        if matches!(gesture.action, NoteAction::Paint | NoteAction::Erase) {
            self.brush_notes(event.position, layout);
            return;
        }
        let at = event.position - self.piano.origin.get();
        if !matches!(gesture.action, NoteAction::Velocity) {
            let edge = |position: f32, low: f32, high: f32| {
                if position < low {
                    (position - low).max(-18.)
                } else if position > high {
                    (position - high).min(18.)
                } else {
                    0.
                }
            };
            self.piano.set_offset((
                (layout.scroll_x + edge(f32::from(at.x), KEY_WIDTH, layout.width - SCROLLBAR))
                    .clamp(0., layout.max_x),
                (layout.scroll_y + edge(f32::from(at.y), RULER, RULER + layout.grid_height))
                    .clamp(0., layout.max_y),
            ));
            layout = self.piano_layout().unwrap();
        }
        let gesture = self.document.gesture.as_mut().unwrap();
        let delta = event.position - gesture.pointer;
        let dx = f32::from(delta.x) + layout.scroll_x - gesture.scroll.0;
        let dy = f32::from(delta.y) + layout.scroll_y - gesture.scroll.1;
        let snap = self.piano.effective_snap(event.modifiers.alt);
        let before: Vec<_> = gesture.changes.iter().map(|c| c.before.clone()).collect();
        let beats = if matches!(
            gesture.action,
            NoteAction::Move | NoteAction::Duplicate | NoteAction::Insert
        ) {
            crate::note_transform::clamp_start_delta(
                &before,
                f64::from(dx / layout.beat_width),
                snap,
            )
        } else {
            f64::from(dx / layout.beat_width)
        };
        let next = transform(
            &before,
            gesture.action,
            beats,
            -(dy / layout.key_height).round() as i32,
            snap,
        );
        for (change, after) in gesture.changes.iter_mut().zip(next) {
            change.after = after;
        }
    }
}
