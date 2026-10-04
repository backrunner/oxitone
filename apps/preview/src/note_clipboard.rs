//! In-app note clipboard survives source revisions and switching patterns.
use crate::{
    note_gesture::{NoteChange, NoteGesture},
    note_transform::NoteAction,
    ui::Preview,
};
use gpui::*;
use serde_json::Value;

impl Preview {
    pub fn note_clipboard_key(&mut self, key: &Keystroke) -> bool {
        if !(key.modifiers.platform || key.modifiers.control)
            || key.modifiers.alt
            || !matches!(key.key.as_str(), "c" | "x" | "v")
        {
            return false;
        }
        let Some(site) = self.pattern_site().cloned() else {
            return false;
        };
        if !self.document_ready() {
            return true;
        }
        if key.key == "v" {
            let notes = &self.document.notes.clipboard;
            if notes.is_empty() {
                return true;
            }
            let first = notes.iter().map(|n| n.start).fold(f64::INFINITY, f64::min);
            let delta = if key.modifiers.shift {
                0.
            } else {
                self.document.notes.cursor - first
            };
            self.document.gesture = Some(NoteGesture {
                select_after: true,
                site: site.handle,
                placement: self.edit_placement(),
                action: NoteAction::Insert,
                pointer: point(px(0.), px(0.)),
                scroll: (0., 0.),
                changes: notes
                    .iter()
                    .map(|note| {
                        let mut after = note.clone();
                        after.start += delta;
                        NoteChange {
                            index: None,
                            before: note.clone(),
                            after,
                            selector: Value::Null,
                        }
                    })
                    .collect(),
            });
            self.finish_note();
        } else {
            if self.document.notes.site.as_ref() != Some(&site.handle) {
                return true;
            }
            let outputs: Vec<_> = self
                .document
                .notes
                .indices
                .iter()
                .filter_map(|&i| site.outputs.get(i).map(|o| (i, o)))
                .collect();
            if outputs.is_empty() {
                return true;
            }
            self.document.notes.clipboard = outputs.iter().map(|(_, o)| o.note.clone()).collect();
            if key.key == "x" {
                self.document.gesture = Some(NoteGesture {
                    select_after: true,
                    site: site.handle.clone(),
                    placement: self.edit_placement(),
                    action: NoteAction::Erase,
                    pointer: point(px(0.), px(0.)),
                    scroll: (0., 0.),
                    changes: outputs
                        .iter()
                        .map(|(i, o)| NoteChange {
                            index: Some(*i),
                            before: o.note.clone(),
                            after: o.note.clone(),
                            selector: o.select.clone(),
                        })
                        .collect(),
                });
                self.finish_note();
            }
        }
        true
    }
}
