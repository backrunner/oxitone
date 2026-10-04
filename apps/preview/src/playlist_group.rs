//! A group gesture keeps its relative timing/lanes and commits one batch.
use crate::{
    playlist_actions::Clip,
    playlist_edit::{ArrangementEdit, Drag, DragMode, ResourceKind},
    ui::Preview,
};

pub fn project(this: &Preview, mut clips: Vec<Clip>, drag: &Drag) -> Option<Vec<Clip>> {
    let projected = targets(this, drag)?;
    if drag.mode != DragMode::Copy {
        clips.retain(|c| {
            !this
                .document
                .playlist
                .drag_group
                .iter()
                .any(|s| s.id == c.id)
        });
    }
    for mut c in projected {
        if drag.mode == DragMode::Copy {
            c.id = format!("playlist-copy-{}", c.id);
        }
        clips.push(c);
    }
    Some(clips)
}
fn targets(this: &Preview, drag: &Drag) -> Option<Vec<Clip>> {
    let group = &this.document.playlist.drag_group;
    let id = &drag.clip.as_ref()?.0;
    let original = group.iter().find(|c| &c.id == id)?;
    let (target, start) = drag.target.as_ref()?;
    let tracks = &this.project.as_ref()?.snapshot.tracks;
    let row = |id: &str| tracks.iter().position(|t| t.id == id);
    let row_delta = row(target)? as isize - row(&original.track)? as isize;
    let first = group.iter().map(|c| c.start).fold(f64::INFINITY, f64::min);
    let delta = (start - original.start).max(-first);
    let shortest = group.iter().map(|c| c.length).fold(f64::INFINITY, f64::min);
    let resize_delta = (drag.length - original.length).max(1. / 960. - shortest);
    group
        .iter()
        .map(|c| {
            let mut next = c.clone();
            if drag.mode == DragMode::Resize {
                next.length = (c.length + resize_delta).max(1. / 960.);
            } else {
                next.start += delta;
                next.track = tracks
                    .get(row(&c.track)?.checked_add_signed(row_delta)?)?
                    .id
                    .clone();
            }
            Some(next)
        })
        .collect()
}
impl Preview {
    pub fn finish_playlist_group(&mut self, drag: &Drag) -> bool {
        if drag.clip.is_none() || self.document.playlist.drag_group.is_empty() {
            return false;
        }
        let Some(next) = targets(self, drag) else {
            return true;
        };
        let mut edits = Vec::new();
        for c in &next {
            let Some(original) = self
                .document
                .playlist
                .drag_group
                .iter()
                .find(|s| s.id == c.id)
            else {
                return true;
            };
            let Some((clip, resource, _)) = self.placement_address(original) else {
                return true;
            };
            let o = self
                .document
                .view
                .as_ref()
                .unwrap()
                .arrangement_order
                .as_ref()
                .unwrap();
            let Some(track) = o.tracks.iter().position(|id| id == &c.track) else {
                return true;
            };
            edits.push(match drag.mode {
                DragMode::Copy => ArrangementEdit::Duplicate {
                    kind: c.kind,
                    clip,
                    resource,
                    track,
                    start_beat: c.start,
                },
                DragMode::Move => ArrangementEdit::Move {
                    kind: c.kind,
                    clip,
                    resource,
                    track,
                    start_beat: c.start,
                },
                DragMode::Resize if c.kind == ResourceKind::Sample => ArrangementEdit::FitSample {
                    kind: c.kind,
                    clip,
                    resource,
                    duration_beats: c.length,
                },
                DragMode::Resize => ArrangementEdit::Resize {
                    kind: c.kind,
                    clip,
                    resource,
                    duration_beats: c.length,
                },
            });
        }
        self.submit_placements(edits, next);
        if self.document.pending.is_some() {
            self.document.playlist.pending = Some(drag.clone());
        }
        true
    }
}
