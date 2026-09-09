//! Local note preview and its single semantic transaction payload.
use crate::{document_wire::SourceNote, note_transform::NoteAction};
use gpui::*;
use serde_json::{json, Value};
#[derive(Clone)]
pub struct NoteChange {
    pub index: Option<usize>,
    pub before: SourceNote,
    pub after: SourceNote,
    pub selector: Value,
}
#[derive(Clone)]
pub struct NoteGesture {
    pub site: String,
    pub placement: Option<String>,
    pub changes: Vec<NoteChange>,
    pub action: NoteAction,
    pub select_after: bool,
    pub pointer: Point<Pixels>,
    pub scroll: (f32, f32),
}
impl NoteGesture {
    pub fn edits(&self) -> Vec<Value> {
        self.changes.iter().filter_map(|change| {
                if self.action == NoteAction::Erase {
                    Some(json!({ "select": change.selector, "expect": change.before, "remove": true }))
                } else if self.action == NoteAction::Duplicate && change.before == change.after {
                    None
                } else if matches!(self.action, NoteAction::Insert | NoteAction::InsertResize | NoteAction::Duplicate | NoteAction::Paint) {
                Some(json!({ "insert": change.after }))
            } else if change.before != change.after {
                Some(json!({ "select": change.selector, "expect": change.before, "set": change.after }))
            } else { None }
        }).collect()
    }
}
