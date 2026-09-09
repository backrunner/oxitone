//! Shared selection, duplication, deletion and edge gestures for Playlist placements.
use crate::{
    document_wire::DocumentOperation,
    playlist_edit::{ArrangementEdit, Drag, DragMode, ResourceKind},
    ui::Preview,
};
use gpui::{prelude::*, *};

#[derive(Clone)]
pub struct Clip {
    pub kind: ResourceKind,
    pub id: String,
    pub resource: String,
    pub track: String,
    pub start: f64,
    pub length: f64,
    pub enabled: bool,
}

impl Preview {
    pub fn begin_clip(
        &mut self,
        clip: &Clip,
        mode: DragMode,
        event: &MouseDownEvent,
        window: &mut Window,
    ) {
        self.workspace_focus.focus(window);
        self.document.playlist.focused = true;
        self.document.playlist.selected = Some((clip.kind, clip.id.clone()));
        if clip.kind == ResourceKind::Pattern {
            self.selected_clip = Some(clip.id.clone());
        }
        if !self.document_ready() {
            return;
        }
        self.document.playlist.drag = Some(Drag {
            mode,
            kind: clip.kind,
            resource: clip.resource.clone(),
            clip: Some((clip.id.clone(), 0)),
            length: clip.length,
            anchor: event.position,
            offset: self.playlist_offset(&clip.track, event.position.x, clip.start),
            target: Some((clip.track.clone(), clip.start)),
            moved: false,
        });
    }
    fn selected_placement(&self) -> Option<Clip> {
        let (kind, id) = self.document.playlist.selected.as_ref()?;
        let p = self.project.as_ref()?;
        let (resource, track, start, length, enabled) = match kind {
            ResourceKind::Pattern => {
                let c = p.snapshot.pattern_clips.iter().find(|c| &c.id == id)?;
                let (start, end) = p.clip_bounds(c);
                (
                    c.pattern_id.clone(),
                    c.track_id.clone(),
                    start,
                    end - start,
                    c.enabled != Some(false),
                )
            }
            ResourceKind::Sample => {
                let c = p.snapshot.sample_clips.iter().find(|c| &c.id == id)?;
                let (_, start, end) = p.plan.samples.iter().find(|c| &c.0 == id)?;
                (
                    c.sample_id.clone(),
                    c.track_id.clone(),
                    p.beat(*start),
                    p.beat(*end) - p.beat(*start),
                    c.enabled != Some(false),
                )
            }
            ResourceKind::Automation => {
                let c = p
                    .snapshot
                    .automation_clips
                    .as_ref()?
                    .iter()
                    .find(|c| &c.id == id)?;
                (
                    c.lane_id.clone(),
                    c.track_id.clone(),
                    c.start_beat.to_f64(),
                    c.duration_beats?.to_f64(),
                    c.enabled != Some(false),
                )
            }
        };
        Some(Clip {
            kind: *kind,
            id: id.clone(),
            resource,
            track,
            start,
            length,
            enabled,
        })
    }
    pub fn playlist_key(&mut self, event: &KeyDownEvent) -> bool {
        if !self.document.playlist.focused {
            return false;
        }
        let k = &event.keystroke;
        let command = k.modifiers.platform || k.modifiers.control;
        let action = match k.key.as_str() {
            "d" if command && !k.modifiers.alt && !k.modifiers.shift => "copy",
            "backspace" | "delete" if !command && !k.modifiers.alt && !k.modifiers.shift => {
                "remove"
            }
            "m" if !command && !k.modifiers.alt && !k.modifiers.shift => "enable",
            _ => return false,
        };
        if self.document.windows.front().is_some() {
            return false;
        }
        let Some(c) = self.selected_placement() else {
            return false;
        };
        if event.is_held || !self.document_ready() {
            return true;
        }
        let Some(o) = self
            .document
            .view
            .as_ref()
            .and_then(|v| v.arrangement_order.as_ref())
        else {
            return true;
        };
        let clips = match c.kind {
            ResourceKind::Pattern => &o.pattern_clips,
            ResourceKind::Sample => &o.sample_clips,
            ResourceKind::Automation => &o.automation_clips,
        };
        let (Some(clip), Some(resource), Some(track)) = (
            clips.iter().position(|id| id == &c.id),
            o.resource(c.kind, &c.resource),
            o.tracks.iter().position(|id| id == &c.track),
        ) else {
            return true;
        };
        let edit = match action {
            "copy" => ArrangementEdit::Duplicate {
                kind: c.kind,
                resource,
                clip,
                track,
                start_beat: c.start + c.length,
            },
            "enable" => ArrangementEdit::Enable {
                kind: c.kind,
                resource,
                clip,
                enabled: !c.enabled,
            },
            _ => ArrangementEdit::Remove {
                kind: c.kind,
                resource,
                clip,
            },
        };
        self.document_request(DocumentOperation::Arrangement { edit });
        true
    }
}
pub fn edge(this: &Preview, clip: Clip, cx: &Context<Preview>) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("clip-edge-{}", clip.id)))
        .absolute()
        .right_0()
        .top_0()
        .w(px(6.))
        .h_full()
        .when(this.document_ready(), |d| {
            d.cursor(CursorStyle::ResizeLeftRight)
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event, window, cx| {
                this.begin_clip(&clip, DragMode::Resize, event, window);
                cx.stop_propagation();
                cx.notify();
            }),
        )
}
