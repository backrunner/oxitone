//! Selection and edge gestures for Playlist placements.
use crate::{
    playlist_edit::{Drag, DragMode, ResourceKind},
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
    pub fn selected_placements(&self) -> Vec<Clip> {
        self.project.as_ref().map_or_else(Vec::new, |p| {
            crate::playlist_projection::placements(p)
                .into_iter()
                .filter(|c| {
                    self.document.playlist.selection.contains(&c.id)
                        || (self.document.playlist.selection.is_empty()
                            && self
                                .document
                                .playlist
                                .selected
                                .as_ref()
                                .is_some_and(|s| s.1 == c.id))
                })
                .collect()
        })
    }
    pub fn begin_clip(
        &mut self,
        clip: &Clip,
        mode: DragMode,
        event: &MouseDownEvent,
        window: &mut Window,
    ) {
        self.workspace_focus.focus(window);
        self.document.playlist.focused = true;
        let playlist = &mut self.document.playlist;
        if event.modifiers.platform || event.modifiers.control {
            if !playlist.selection.remove(&clip.id) {
                playlist.selection.insert(clip.id.clone());
            }
            playlist.selected = playlist
                .selection
                .contains(&clip.id)
                .then(|| (clip.kind, clip.id.clone()));
            return;
        }
        if !playlist.selection.contains(&clip.id) {
            playlist.selection.clear();
            playlist.selection.insert(clip.id.clone());
        }
        playlist.selected = Some((clip.kind, clip.id.clone()));
        playlist.brush_clip = Some(clip.clone());
        playlist.cursor = clip.start;
        playlist.cursor_track = Some(clip.track.clone());
        if clip.kind == ResourceKind::Pattern {
            self.selected_clip = Some(clip.id.clone());
        }
        if !self.document_ready() {
            return;
        }
        self.document.playlist.drag_group = self.selected_placements();
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
    pub fn restore_playlist_selection(&mut self) {
        let expected = std::mem::take(&mut self.document.playlist.pending_selection);
        if expected.is_empty() {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let mut available = crate::playlist_projection::placements(project);
        let mut ids = std::collections::BTreeSet::new();
        for clip in expected {
            if let Some(index) = available.iter().rposition(|c| {
                c.kind == clip.kind
                    && c.resource == clip.resource
                    && c.track == clip.track
                    && (c.start - clip.start).abs() < 1e-7
                    && (c.length - clip.length).abs() < 1e-7
            }) {
                let c = available.remove(index);
                self.document.playlist.selected = Some((c.kind, c.id.clone()));
                if c.kind == ResourceKind::Pattern {
                    self.selected_clip = Some(c.id.clone());
                }
                ids.insert(c.id);
            }
        }
        self.document.playlist.selection = ids;
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
