use crate::{
    document_wire::DocumentOperation,
    note_transform::{transform, NoteAction},
    piano_layout::{NoteTool, PianoLayout, RULER},
    ui::Preview,
};
use gpui::*;
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct NoteSelection {
    pub site: Option<String>,
    pub indices: BTreeSet<usize>,
    pub marquee: Option<Marquee>,
}
#[derive(Clone)]
pub struct Marquee {
    pub start: (f64, f32),
    pub end: (f64, f32),
    initial: BTreeSet<usize>,
}
impl NoteSelection {
    pub fn selected(&self, site: &str, index: usize) -> bool {
        self.site.as_deref() == Some(site) && self.indices.contains(&index)
    }
    pub fn clear(&mut self) {
        self.site = None;
        self.indices.clear();
        self.marquee = None;
    }
    pub fn select(&mut self, site: String, indices: impl IntoIterator<Item = usize>) {
        self.site = Some(site);
        self.indices = indices.into_iter().collect();
    }
    pub fn toggle(&mut self, site: String, index: usize) {
        if self.site.as_ref() != Some(&site) {
            self.clear();
            self.site = Some(site);
        }
        if !self.indices.remove(&index) {
            self.indices.insert(index);
        }
    }
}
fn musical(position: Point<Pixels>, origin: Point<Pixels>, layout: PianoLayout) -> (f64, f32) {
    let at = position - origin;
    (
        layout.beat(f32::from(at.x)),
        ((f32::from(at.y) - RULER + layout.scroll_y) / layout.key_height).clamp(0., 128.),
    )
}
impl Preview {
    pub fn start_note_marquee(
        &mut self,
        site: String,
        event: &MouseDownEvent,
        layout: PianoLayout,
    ) {
        let at = musical(event.position, self.piano.origin.get(), layout);
        if !event.modifiers.shift || self.document.notes.site.as_ref() != Some(&site) {
            self.document.notes.clear();
        }
        self.document.notes.site = Some(site);
        self.document.notes.marquee = Some(Marquee {
            start: at,
            end: at,
            initial: self.document.notes.indices.clone(),
        });
    }
    pub fn move_note_marquee(&mut self, position: Point<Pixels>, layout: PianoLayout) {
        let at = musical(position, self.piano.origin.get(), layout);
        let Some(marquee) = self.document.notes.marquee.as_mut() else {
            return;
        };
        marquee.end = at;
        let (left, right) = (marquee.start.0.min(at.0), marquee.start.0.max(at.0));
        let (top, bottom) = (marquee.start.1.min(at.1), marquee.start.1.max(at.1));
        let mut selected = marquee.initial.clone();
        if let Some(site) = self.pattern_site() {
            for (index, output) in site.outputs.iter().enumerate() {
                let note = &output.note;
                let row = f32::from(127 - note.pitch);
                if note.start < right
                    && note.start + note.duration > left
                    && row < bottom
                    && row + 1. > top
                {
                    selected.insert(index);
                }
            }
        }
        self.document.notes.indices = selected;
    }
    pub fn edit_note_key(&mut self, key: &Keystroke) -> bool {
        if self.document.view.is_none() {
            return false;
        }
        let command = key.modifiers.platform || key.modifiers.control;
        if key.key == "escape" {
            if self.piano.snap_menu.take().is_some() {
                return true;
            }
            self.document.gesture = None;
            self.document.notes.clear();
            return true;
        }
        if !command && !key.modifiers.alt {
            match key.key.as_str() {
                "p" => {
                    self.piano.tool = NoteTool::Draw;
                    return true;
                }
                "b" => {
                    self.piano.tool = NoteTool::Paint;
                    return true;
                }
                "e" => {
                    self.piano.tool = NoteTool::Select;
                    return true;
                }
                _ => {}
            }
        }
        if key.modifiers.alt {
            return false;
        }
        let Some(site) = self.pattern_site().cloned() else {
            return false;
        };
        if command && key.key == "a" {
            self.document
                .notes
                .select(site.handle, 0..site.outputs.len());
            return true;
        }
        if self.document.notes.site.as_ref() != Some(&site.handle)
            || self.document.notes.indices.is_empty()
        {
            return false;
        }
        let supported = if command {
            key.key == "d"
        } else {
            matches!(
                key.key.as_str(),
                "delete" | "backspace" | "up" | "down" | "left" | "right" | "q"
            )
        };
        if !supported {
            return false;
        }
        if !self.document_ready() || self.document.gesture.is_some() {
            return true;
        }
        let outputs: Vec<_> = self
            .document
            .notes
            .indices
            .iter()
            .filter_map(|&i| site.outputs.get(i))
            .collect();
        let notes: Vec<_> = outputs.iter().map(|o| o.note.clone()).collect();
        let mut edits = Vec::new();
        if matches!(key.key.as_str(), "delete" | "backspace") {
            edits = outputs
                .iter()
                .map(|o| json!({ "select": o.select, "expect": o.note, "remove": true }))
                .collect();
        } else {
            let mut next = notes.clone();
            if key.key == "q" {
                for note in &mut next {
                    let snap = self.piano.effective_snap(false);
                    note.start = snap.round(note.start).max(0.);
                }
            } else if command && key.key == "d" {
                let start = notes.iter().map(|n| n.start).fold(f64::INFINITY, f64::min);
                let end = notes
                    .iter()
                    .map(|n| n.start + n.duration)
                    .fold(0., f64::max);
                let step = self.piano.effective_snap(false).step();
                next = transform(
                    &notes,
                    NoteAction::Duplicate,
                    ((end - start) / step).ceil() * step,
                    0,
                    self.piano.effective_snap(false),
                );
            } else {
                let horizontal = matches!(key.key.as_str(), "left" | "right");
                let direction = if matches!(key.key.as_str(), "left" | "down") {
                    -1
                } else {
                    1
                };
                next = transform(
                    &notes,
                    if horizontal && key.modifiers.shift {
                        NoteAction::Resize
                    } else {
                        NoteAction::Move
                    },
                    if horizontal && !key.modifiers.shift {
                        crate::note_transform::clamp_start_delta(
                            &notes,
                            self.piano.effective_snap(false).step() * f64::from(direction),
                            self.piano.effective_snap(false),
                        )
                    } else if horizontal {
                        self.piano.effective_snap(false).step() * f64::from(direction)
                    } else {
                        0.
                    },
                    if horizontal {
                        0
                    } else {
                        direction * if key.modifiers.shift { 12 } else { 1 }
                    },
                    self.piano.effective_snap(false),
                );
            }
            for (output, after) in outputs.iter().zip(&next) {
                if command {
                    edits.push(json!({ "insert": after }));
                } else if output.note != *after {
                    edits.push(
                        json!({ "select": output.select, "expect": output.note, "set": after }),
                    );
                }
            }
            if !edits.is_empty() {
                self.document.pending_notes = Some(crate::note_edit::NoteGesture {
                    select_after: true,
                    site: site.handle.clone(),
                    placement: self.edit_placement(),
                    action: if command {
                        NoteAction::Duplicate
                    } else {
                        NoteAction::Move
                    },
                    pointer: point(px(0.), px(0.)),
                    scroll: (0., 0.),
                    changes: self
                        .document
                        .notes
                        .indices
                        .iter()
                        .zip(outputs.iter().zip(next))
                        .map(|(&index, (output, after))| crate::note_edit::NoteChange {
                            index: Some(index),
                            before: output.note.clone(),
                            after,
                            selector: output.select.clone(),
                        })
                        .collect(),
                });
            }
        }
        if !edits.is_empty() {
            self.document.pending_note_source =
                site.outputs.iter().map(|o| o.note.clone()).collect();
            self.document.pending_note_pattern = Some(site.pattern_id.clone());
            self.document_request(DocumentOperation::Notes {
                site: site.handle,
                placement: self.edit_placement(),
                edits,
            });
        }
        if self.document.pending.is_none() {
            self.document.pending_notes = None;
        }
        true
    }
}
