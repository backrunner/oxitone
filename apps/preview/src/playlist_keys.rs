//! Keyboard edits share the source transaction path with pointer gestures.
use crate::{
    document_wire::DocumentOperation,
    piano_state::NoteTool,
    playlist_actions::Clip,
    playlist_edit::{ArrangementEdit, ResourceKind},
    ui::Preview,
};
use gpui::*;

impl Preview {
    pub fn placement_address(&self, c: &Clip) -> Option<(usize, usize, usize)> {
        let o = self.document.view.as_ref()?.arrangement_order.as_ref()?;
        let clips = match c.kind {
            ResourceKind::Pattern => &o.pattern_clips,
            ResourceKind::Sample => &o.sample_clips,
            ResourceKind::Automation => &o.automation_clips,
        };
        Some((
            clips.iter().position(|id| id == &c.id)?,
            o.resource(c.kind, &c.resource)?,
            o.tracks.iter().position(|id| id == &c.track)?,
        ))
    }
    pub fn submit_placements(&mut self, edits: Vec<ArrangementEdit>, selected: Vec<Clip>) {
        if edits.is_empty() {
            return;
        }
        self.document_request(DocumentOperation::Arrangement {
            edit: ArrangementEdit::Batch { edits },
        });
        if self.document.pending.is_some() {
            self.document.playlist.pending_selection = selected;
        }
    }
    pub fn playlist_key(&mut self, event: &KeyDownEvent) -> bool {
        if !self.document.playlist.focused {
            return false;
        }
        let k = &event.keystroke;
        let command = k.modifiers.platform || k.modifiers.control;
        if k.key == "escape" && !command {
            self.document.playlist.selection.clear();
            self.document.playlist.selected = None;
            return true;
        }
        if !command && !k.modifiers.alt {
            let tool = match k.key.as_str() {
                "p" => Some(NoteTool::Draw),
                "b" => Some(NoteTool::Paint),
                "e" => Some(NoteTool::Select),
                _ => None,
            };
            if let Some(tool) = tool {
                self.document.playlist.tool = tool;
                return true;
            }
        }
        if command && k.key == "a" {
            self.document.playlist.selection =
                self.project.as_ref().map_or_else(Default::default, |p| {
                    crate::playlist_projection::placements(p)
                        .into_iter()
                        .map(|c| c.id)
                        .collect()
                });
            return true;
        }
        let supported = if command {
            matches!(k.key.as_str(), "c" | "x" | "v" | "d")
        } else {
            matches!(
                k.key.as_str(),
                "delete" | "backspace" | "m" | "left" | "right" | "up" | "down"
            )
        };
        if !supported {
            return false;
        }
        if event.is_held || !self.document_ready() {
            return true;
        }
        let clips = self.selected_placements();
        if command && matches!(k.key.as_str(), "c" | "x") {
            self.document.playlist.clipboard = self.copy_placements(&clips);
            if k.key == "c" {
                return true;
            }
        }
        if command && k.key == "v" {
            self.paste_playlist(k.modifiers.shift);
            return true;
        }
        if clips.is_empty() {
            return true;
        }
        let first = clips.iter().map(|c| c.start).fold(f64::INFINITY, f64::min);
        let end = clips.iter().map(|c| c.start + c.length).fold(0., f64::max);
        let step: f64 = if k.modifiers.alt { 1. / 960. } else { 0.25 };
        let shortest = clips.iter().map(|c| c.length).fold(f64::INFINITY, f64::min);
        let resize_delta = if k.key == "left" {
            -step.min((shortest - 1. / 960.).max(0.))
        } else {
            step
        };
        let delta = match k.key.as_str() {
            "d" => ((end - first) / step).ceil() * step,
            "left" => -step.min(first),
            "right" => step,
            _ => 0.,
        };
        let mut edits = Vec::new();
        let mut expected = Vec::new();
        for mut c in clips {
            let Some((clip, resource, track)) = self.placement_address(&c) else {
                return true;
            };
            let edit =
                if (command && k.key == "x") || matches!(k.key.as_str(), "delete" | "backspace") {
                    ArrangementEdit::Remove {
                        kind: c.kind,
                        resource,
                        clip,
                    }
                } else if k.key == "m" {
                    c.enabled = !c.enabled;
                    ArrangementEdit::Enable {
                        kind: c.kind,
                        resource,
                        clip,
                        enabled: c.enabled,
                    }
                } else if command && k.key == "d" {
                    c.start += delta;
                    ArrangementEdit::Duplicate {
                        kind: c.kind,
                        resource,
                        clip,
                        track,
                        start_beat: c.start,
                    }
                } else if k.modifiers.shift && matches!(k.key.as_str(), "left" | "right") {
                    c.length = (c.length + resize_delta).max(1. / 960.);
                    if c.kind == ResourceKind::Sample {
                        ArrangementEdit::FitSample {
                            kind: c.kind,
                            resource,
                            clip,
                            duration_beats: c.length,
                        }
                    } else {
                        ArrangementEdit::Resize {
                            kind: c.kind,
                            resource,
                            clip,
                            duration_beats: c.length,
                        }
                    }
                } else {
                    let tracks: Vec<_> = self
                        .project
                        .as_ref()
                        .unwrap()
                        .snapshot
                        .tracks
                        .iter()
                        .map(|t| t.id.clone())
                        .collect();
                    let row = tracks.iter().position(|id| id == &c.track).unwrap_or(0);
                    let next = if k.key == "up" {
                        row.checked_sub(1)
                    } else if k.key == "down" {
                        (row + 1 < tracks.len()).then_some(row + 1)
                    } else {
                        Some(row)
                    };
                    let Some(next) = next else {
                        return true;
                    };
                    c.track = tracks[next].clone();
                    c.start += delta;
                    let o = self
                        .document
                        .view
                        .as_ref()
                        .unwrap()
                        .arrangement_order
                        .as_ref()
                        .unwrap();
                    ArrangementEdit::Move {
                        kind: c.kind,
                        resource,
                        clip,
                        track: o.tracks.iter().position(|id| id == &c.track).unwrap(),
                        start_beat: c.start,
                    }
                };
            if !matches!(edit, ArrangementEdit::Remove { .. }) {
                expected.push(c);
            }
            edits.push(edit);
        }
        self.submit_placements(edits, expected);
        true
    }
    fn paste_playlist(&mut self, original: bool) {
        let copied = self.document.playlist.clipboard.clone();
        if copied.is_empty() {
            return;
        }
        let first = copied
            .iter()
            .map(|c| c.clip.start)
            .fold(f64::INFINITY, f64::min);
        let delta = if original {
            0.
        } else {
            self.document.playlist.cursor - first
        };
        let tracks: Vec<_> = self
            .project
            .as_ref()
            .unwrap()
            .snapshot
            .tracks
            .iter()
            .map(|t| t.id.clone())
            .collect();
        let first_row = copied
            .iter()
            .filter_map(|c| tracks.iter().position(|id| id == &c.clip.track))
            .min()
            .unwrap_or(0);
        let row_delta = if original {
            0
        } else {
            self.document
                .playlist
                .cursor_track
                .as_ref()
                .and_then(|id| tracks.iter().position(|t| t == id))
                .unwrap_or(first_row) as isize
                - first_row as isize
        };
        let mut edits = Vec::new();
        let mut expected = Vec::new();
        for c in &copied {
            let Some(row) = tracks.iter().position(|t| t == &c.clip.track) else {
                return;
            };
            let Some(track) = row
                .checked_add_signed(row_delta)
                .and_then(|i| tracks.get(i))
            else {
                return;
            };
            let Some(edit) = self.paste_placement(c, track, c.clip.start + delta) else {
                self.status = "Clipboard resource is no longer in this project".into();
                return;
            };
            let mut next = c.clip.clone();
            next.start += delta;
            next.track = track.clone();
            expected.push(next);
            edits.push(edit);
        }
        self.submit_placements(edits, expected);
    }
}
