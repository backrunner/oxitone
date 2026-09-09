//! Playlist pointer capture and one semantic transaction per drop.
use crate::{document_wire::DocumentOperation, ui::Preview};
use gpui::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    #[default]
    Pattern,
    Sample,
    Automation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ArrangementEdit {
    Place {
        kind: ResourceKind,
        resource: usize,
        track: usize,
        start_beat: f64,
        duration_beats: f64,
    },
    Move {
        kind: ResourceKind,
        resource: usize,
        clip: usize,
        track: usize,
        start_beat: f64,
    },
    Remove {
        kind: ResourceKind,
        resource: usize,
        clip: usize,
    },
    Duplicate {
        kind: ResourceKind,
        resource: usize,
        clip: usize,
        track: usize,
        start_beat: f64,
    },
    Resize {
        kind: ResourceKind,
        resource: usize,
        clip: usize,
        duration_beats: f64,
    },
    FitSample {
        kind: ResourceKind,
        resource: usize,
        clip: usize,
        duration_beats: f64,
    },
    Enable {
        kind: ResourceKind,
        resource: usize,
        clip: usize,
        enabled: bool,
    },
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub channels: Vec<String>,
    pub mixer_channels: Vec<String>,
    pub patterns: Vec<String>,
    pub tracks: Vec<String>,
    pub samples: Vec<String>,
    pub automation: Vec<String>,
    pub pattern_clips: Vec<String>,
    pub sample_clips: Vec<String>,
    #[serde(default)]
    pub automation_clips: Vec<String>,
}
impl Order {
    pub fn resource(&self, kind: ResourceKind, id: &str) -> Option<usize> {
        match kind {
            ResourceKind::Pattern => &self.patterns,
            ResourceKind::Sample => &self.samples,
            ResourceKind::Automation => &self.automation,
        }
        .iter()
        .position(|v| v == id)
    }
}
#[derive(Clone)]
pub struct Drag {
    pub mode: DragMode,
    pub kind: ResourceKind,
    pub resource: String,
    pub clip: Option<(String, usize)>,
    pub length: f64,
    pub anchor: Point<Pixels>,
    pub offset: f64,
    pub target: Option<(String, f64)>,
    pub moved: bool,
}
#[derive(Clone, Copy, PartialEq, Default)]
pub enum DragMode {
    #[default]
    Move,
    Copy,
    Resize,
}
#[derive(Default)]
pub struct PlaylistUi {
    pub focused: bool,
    pub selected: Option<(ResourceKind, String)>,
    pub drag: Option<Drag>,
    pub pending: Option<Drag>,
    pub controls: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    pub rows: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    pub viewport: Rc<Cell<Bounds<Pixels>>>,
}
impl Preview {
    pub fn playlist_offset(&self, track: &str, x: Pixels, start: f64) -> f64 {
        self.document
            .playlist
            .rows
            .borrow()
            .get(track)
            .map_or(0., |bounds| {
                f64::from(f32::from(x - bounds.origin.x)) / f64::from(self.zoom) - start
            })
    }
    pub fn move_playlist(&mut self, event: &MouseMoveEvent) {
        let occluded = self.document.windows.hit(event.position).is_some();
        let Some(drag) = &mut self.document.playlist.drag else {
            return;
        };
        drag.moved |= (event.position - drag.anchor).magnitude() > f64::from(px(3.));
        if drag.mode == DragMode::Resize {
            if let Some((id, start)) = &drag.target {
                if let Some(bounds) = self.document.playlist.rows.borrow().get(id) {
                    let end = f64::from(f32::from(event.position.x - bounds.origin.x))
                        / f64::from(self.zoom);
                    let step = if event.modifiers.alt { 1. / 960. } else { 0.25 };
                    drag.length = (((end - start) / step).round() * step).max(step);
                }
            }
            return;
        }
        drag.target = None;
        if occluded
            || !self
                .document
                .playlist
                .viewport
                .get()
                .contains(&event.position)
        {
            return;
        }
        // Lane bounds are captured from the rendered layout. This keeps the
        // target correct when the editor is resized, scrolled, or detached.
        for (id, bounds) in self.document.playlist.rows.borrow().iter() {
            if bounds.contains(&event.position) {
                let beat = (f64::from(f32::from(event.position.x - bounds.origin.x))
                    / f64::from(self.zoom)
                    - drag.offset)
                    .max(0.);
                let step = if event.modifiers.alt { 1. / 960. } else { 0.25 };
                drag.target = Some((id.clone(), (beat / step).round() * step));
                break;
            }
        }
    }
    pub fn finish_playlist(&mut self) {
        let Some(drag) = self.document.playlist.drag.take() else {
            return;
        };
        if !drag.moved {
            return;
        }
        let Some((track, beat)) = &drag.target else {
            return;
        };
        let Some(order) = self
            .document
            .view
            .as_ref()
            .and_then(|v| v.arrangement_order.as_ref())
        else {
            return;
        };
        let Some(track) = order.tracks.iter().position(|id| id == track) else {
            return;
        };
        let Some(resource) = order.resource(drag.kind, &drag.resource) else {
            return;
        };
        let edit = if let Some((id, _)) = &drag.clip {
            let clip = match drag.kind {
                ResourceKind::Pattern => order.pattern_clips.iter().position(|v| v == id),
                ResourceKind::Sample => order.sample_clips.iter().position(|v| v == id),
                ResourceKind::Automation => order.automation_clips.iter().position(|v| v == id),
            };
            let Some(clip) = clip else {
                return;
            };
            if drag.mode == DragMode::Resize {
                if drag.kind == ResourceKind::Sample {
                    ArrangementEdit::FitSample {
                        kind: drag.kind,
                        resource,
                        clip,
                        duration_beats: drag.length,
                    }
                } else {
                    ArrangementEdit::Resize {
                        kind: drag.kind,
                        resource,
                        clip,
                        duration_beats: drag.length,
                    }
                }
            } else if drag.mode == DragMode::Copy {
                ArrangementEdit::Duplicate {
                    kind: drag.kind,
                    resource,
                    clip,
                    track,
                    start_beat: *beat,
                }
            } else {
                ArrangementEdit::Move {
                    kind: drag.kind,
                    resource,
                    clip,
                    track,
                    start_beat: *beat,
                }
            }
        } else {
            ArrangementEdit::Place {
                kind: drag.kind,
                resource,
                track,
                start_beat: *beat,
                duration_beats: drag.length,
            }
        };
        self.document_request(DocumentOperation::Arrangement { edit });
        if self.document.pending.is_some() {
            self.document.playlist.pending = Some(drag);
        }
    }
}
