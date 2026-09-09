//! One content renderer for stationary, dragged and pending Playlist items.
use crate::{
    playlist_edit::{DragMode, ResourceKind},
    ui::{alpha, Preview},
};
use gpui::{prelude::*, *};
use oxitone_core::wire::TrackSpec;

pub fn view(
    this: &Preview,
    mut lane: Div,
    track: &TrackSpec,
    tint: u32,
    clips: &[crate::playlist_actions::Clip],
    cx: &mut Context<Preview>,
) -> Div {
    let project = this.project.as_ref().unwrap();
    let theme = this.theme;
    let drag = this.document.playlist.drag.as_ref().or_else(|| {
        this.document
            .playlist
            .pending
            .as_ref()
            .filter(|_| this.presentation_active())
    });
    for clip in clips.iter().filter(|c| c.track == track.id).cloned() {
        let selected = this
            .document
            .playlist
            .selected
            .as_ref()
            .is_some_and(|s| s.1 == clip.id)
            || this.selected_clip.as_ref() == Some(&clip.id)
            || drag.is_some_and(|d| {
                d.moved
                    && d.resource == clip.resource
                    && d.target
                        .as_ref()
                        .is_some_and(|(t, b)| t == &clip.track && *b == clip.start)
            });
        let color = if clip.kind == ResourceKind::Automation {
            theme.gold
        } else {
            tint
        };
        let mut item = div()
            .id(SharedString::from(clip.id.clone()))
            .absolute()
            .left(px(clip.start as f32 * this.zoom))
            .top(px(4.))
            .w(px((clip.length as f32 * this.zoom - 2.).max(2.)))
            .h(px(43.))
            .rounded_sm()
            .overflow_hidden()
            .border_1()
            .border_color(rgb(if selected { theme.text } else { color }))
            .bg(alpha(
                color,
                theme.clip_opacity * if clip.enabled { 1. } else { 0.35 },
            ))
            .hover(move |style| style.border_color(rgb(theme.text)))
            .cursor_pointer();
        let title = match clip.kind {
            ResourceKind::Pattern => project.pattern_label(&clip.resource),
            ResourceKind::Sample => project
                .snapshot
                .samples
                .iter()
                .find(|s| s.id == clip.resource)
                .and_then(|s| std::path::Path::new(&s.asset_uri).file_name())
                .map_or_else(|| "Audio clip".into(), |n| n.to_string_lossy().into_owned()),
            ResourceKind::Automation => project
                .snapshot
                .automation
                .iter()
                .find(|s| s.id == clip.resource)
                .map_or_else(|| "Automation".into(), |s| project.automation_label(s)),
        };
        item = item.child(
            div()
                .px_1()
                .h(px(17.))
                .overflow_hidden()
                .bg(alpha(color, 0.08))
                .text_size(px(9.))
                .text_color(rgb(color))
                .child(div().truncate().child(title)),
        );
        match clip.kind {
            ResourceKind::Pattern => {
                item = item.child(
                    div()
                        .h(px(24.))
                        .px_1()
                        .child(crate::pattern_preview::thumbnail(
                            project.clone(),
                            clip.clone(),
                            color,
                        )),
                )
            }
            ResourceKind::Automation => {
                item = item.child(
                    div()
                        .h(px(24.))
                        .px_1()
                        .child(crate::automation_thumbnail::view(
                            project,
                            &clip.resource,
                            clip.length,
                            color,
                        )),
                )
            }
            ResourceKind::Sample => {
                let source_id = clip.id.strip_prefix("playlist-copy-").unwrap_or(&clip.id);
                let sync = project
                    .snapshot
                    .sample_clips
                    .iter()
                    .find(|s| s.id == source_id)
                    .and_then(|s| s.tempo_sync);
                let label = match sync {
                    Some(oxitone_core::wire::TempoSync::Stretch) => "STRETCH",
                    Some(oxitone_core::wire::TempoSync::Repitch) => "REPITCH",
                    _ => "AUDIO",
                };
                item = item.child(
                    div()
                        .px_1()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child(label),
                );
            }
        }
        lane = lane.child(
            item.child(crate::playlist_actions::edge(this, clip.clone(), cx))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.begin_clip(
                            &clip,
                            if event.modifiers.shift {
                                DragMode::Copy
                            } else {
                                DragMode::Move
                            },
                            event,
                            window,
                        );
                        if clip.kind == ResourceKind::Automation {
                            this.document.automation.selected = Some(clip.resource.clone());
                            if event.click_count == 2 {
                                this.document.playlist.drag = None;
                                this.document.automation.open = true;
                                this.document
                                    .windows
                                    .focus(crate::window_manager::WindowId::Automation);
                            }
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
        );
    }
    lane
}
