//! Submit a note gesture and restore its selection after the accepted source arrives.
use crate::{
    document_wire::{DocumentOperation, SourceNote, SourceOutput},
    note_gesture::{NoteChange, NoteGesture},
    note_transform::NoteAction,
    ui::Preview,
};
use std::collections::{BTreeSet, HashMap, VecDeque};

/// Musical values are revision-local lookup keys; equal duplicates consume distinct outputs.
pub fn selection_indices(outputs: &[SourceOutput], changes: &[NoteChange]) -> BTreeSet<usize> {
    let key = |note: &SourceNote| {
        let bits = |v: f64| if v == 0. { 0 } else { v.to_bits() };
        (
            note.pitch,
            bits(note.start),
            bits(note.duration),
            bits(note.velocity),
        )
    };
    let mut available: HashMap<_, VecDeque<_>> = HashMap::new();
    for (index, output) in outputs.iter().enumerate() {
        available
            .entry(key(&output.note))
            .or_default()
            .push_back(index);
    }
    changes
        .iter()
        .filter_map(|change| available.get_mut(&key(&change.after))?.pop_front())
        .collect()
}

impl Preview {
    pub fn finish_note(&mut self) {
        self.document.notes.marquee = None;
        let Some(gesture) = self.document.gesture.take() else {
            return;
        };
        let edits = gesture.edits();
        if edits.is_empty() {
            return;
        }
        if gesture.action != NoteAction::Velocity {
            if let Some(change) = gesture.changes.first() {
                self.piano.note_length = change.after.duration;
            }
        }
        let source = self
            .pattern_site()
            .map(|s| s.outputs.iter().map(|o| o.note.clone()).collect())
            .unwrap_or_default();
        let pattern = self.pattern_site().map(|s| s.pattern_id.clone());
        self.document_request(DocumentOperation::Notes {
            site: gesture.site.clone(),
            placement: gesture.placement.clone(),
            edits,
        });
        if self.document.pending.is_some() {
            self.document.pending_notes = Some(gesture);
            self.document.pending_note_source = source;
            self.document.pending_note_pattern = pattern;
        }
    }

    pub fn restore_note_selection(&mut self, gesture: &NoteGesture) {
        if !gesture.select_after {
            return;
        }
        if gesture.action == NoteAction::Erase {
            self.document.notes.clear();
            return;
        }
        if gesture.placement.is_some() && gesture.placement != self.selected_clip {
            return;
        }
        let Some(site) = self.pattern_site() else {
            return;
        };
        // Select acknowledged values; source handles and output order may change.
        let indices = selection_indices(&site.outputs, &gesture.changes);
        let handle = site.handle.clone();
        self.document.notes.select(handle, indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restored_selection_tracks_reordering_duplicates_and_signed_zero() {
        let note = SourceNote {
            pitch: 60,
            start: 0.,
            duration: 1.,
            velocity: 0.5,
        };
        let mut second = note.clone();
        second.pitch = 64;
        let outputs: Vec<_> = [second.clone(), note.clone(), note.clone()]
            .into_iter()
            .map(|note| SourceOutput {
                note,
                select: serde_json::Value::Null,
            })
            .collect();
        let mut changed = note.clone();
        changed.start = -0.;
        let changes: Vec<_> = [changed, second, note]
            .into_iter()
            .map(|after| NoteChange {
                index: None,
                before: after.clone(),
                after,
                selector: serde_json::Value::Null,
            })
            .collect();
        assert_eq!(
            selection_indices(&outputs, &changes),
            BTreeSet::from([0, 1, 2])
        );
    }
}
