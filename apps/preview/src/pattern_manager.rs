//! Compact resource picker. Playlist and editors remain visible while dragging.
use crate::{
    playlist_edit::{Drag, ResourceKind},
    ui::Preview,
};
use gpui::{prelude::*, *};

pub fn view(this: &mut Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    this.document.browser.resource_bounds.borrow_mut().clear();
    let mut list = div()
        .id("pattern-manager")
        .min_w_0()
        .flex()
        .flex_col()
        .py_1();
    let Some(project) = this.project.as_ref().cloned() else {
        return list;
    };
    let selected = this.active_pattern().map(|pattern| &pattern.id);
    for (kind, label) in [
        (ResourceKind::Pattern, "Patterns"),
        (ResourceKind::Sample, "Samples"),
        (ResourceKind::Automation, "Automation"),
    ] {
        let mut entries: Vec<(String, String, f64)> = match kind {
            ResourceKind::Pattern => project
                .snapshot
                .patterns
                .iter()
                .filter(|p| {
                    !project.snapshot.patterns.iter().any(|root| {
                        root.parts
                            .as_ref()
                            .is_some_and(|parts| parts.iter().any(|part| part.pattern_id == p.id))
                    }) || project
                        .snapshot
                        .pattern_clips
                        .iter()
                        .any(|c| c.pattern_id == p.id)
                })
                .map(|p| {
                    (
                        p.id.clone(),
                        project.pattern_label(&p.id),
                        p.length_beats.to_f64(),
                    )
                })
                .collect(),
            ResourceKind::Sample => project
                .snapshot
                .samples
                .iter()
                .map(|s| {
                    (
                        s.id.clone(),
                        std::path::Path::new(&s.asset_uri)
                            .file_name()
                            .map_or_else(|| "Sample".into(), |n| n.to_string_lossy().into_owned()),
                        s.frames as f64 / f64::from(s.sample_rate),
                    )
                })
                .collect(),
            ResourceKind::Automation => project
                .snapshot
                .automation
                .iter()
                .filter(|lane| lane.target.entity_id != project.snapshot.id)
                .map(|lane| {
                    (
                        lane.id.clone(),
                        project.automation_label(lane),
                        lane.last_beat.map_or(4., |b| b.to_f64().max(0.25)),
                    )
                })
                .collect(),
        };
        if kind == ResourceKind::Pattern {
            entries.sort_by(|a, b| a.1.cmp(&b.1));
        }
        if entries.is_empty() {
            continue;
        }
        list = list.child(
            div()
                .px_3()
                .h(px(26.))
                .flex()
                .items_center()
                .text_size(px(10.))
                .text_color(rgb(t.muted))
                .child(label),
        );
        for (index, (id, name, length)) in entries.into_iter().enumerate() {
            let bounds = this.document.browser.resource_bounds.clone();
            let measured = format!("{kind:?}/{id}");
            let automation = (kind == ResourceKind::Automation)
                .then(|| {
                    project
                        .snapshot
                        .automation
                        .iter()
                        .find(|lane| lane.id == id)
                        .map(|lane| project.automation_target(lane))
                })
                .flatten();
            let active = kind == ResourceKind::Pattern && selected == Some(&id);
            let tint = match kind {
                ResourceKind::Pattern => t.accent,
                ResourceKind::Sample => t.gold,
                ResourceKind::Automation => t.track(3),
            };
            list = list.child(
                div()
                    .id(SharedString::from(format!("resource-{kind:?}-{index}")))
                    .relative()
                    .child(
                        canvas(
                            move |area, _, _| {
                                bounds.borrow_mut().insert(measured.clone(), area);
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .h(px(if automation.is_some() { 42. } else { 28. }))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .bg(rgb(if active { t.selected } else { t.bg }))
                    .hover(move |s| s.bg(rgb(t.button)))
                    .child(div().w(px(3.)).h(px(14.)).bg(rgb(tint)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.))
                            .child(
                                div().truncate().child(
                                    automation
                                        .as_ref()
                                        .map_or(name, |label| label.parameter.clone()),
                                ),
                            )
                            .when_some(automation, |d, label| {
                                d.child(
                                    div()
                                        .truncate()
                                        .text_size(px(10.))
                                        .text_color(rgb(t.muted))
                                        .child(label.context()),
                                )
                            }),
                    )
                    .child(div().text_size(px(9.)).text_color(rgb(t.muted)).child(
                        if kind == ResourceKind::Sample {
                            format!("{length:.2}s")
                        } else {
                            format!("{length}")
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.workspace_focus.focus(window);
                            if kind == ResourceKind::Pattern {
                                this.select_pattern(&id);
                                if event.click_count == 2 {
                                    this.selected_clip = this.project.as_ref().and_then(|p| {
                                        p.snapshot
                                            .pattern_clips
                                            .iter()
                                            .find(|c| c.pattern_id == id)
                                            .map(|c| c.id.clone())
                                    });
                                    this.float_editor(crate::workspace_layout::EditorMode::Piano);
                                    cx.notify();
                                    return;
                                }
                            } else if kind == ResourceKind::Automation && event.click_count == 2 {
                                this.document.automation.selected = Some(id.clone());
                                this.document.automation.open = true;
                                this.document
                                    .windows
                                    .focus(crate::window_manager::WindowId::Automation);
                                cx.notify();
                                return;
                            }
                            if this.document_ready() {
                                if kind == ResourceKind::Sample {
                                    if let Some(index) = this
                                        .document
                                        .view
                                        .as_ref()
                                        .and_then(|v| v.arrangement_order.as_ref())
                                        .and_then(|o| o.samples.iter().position(|s| s == &id))
                                    {
                                        this.document.browser.drag =
                                            Some(crate::sample_drop::SampleDrag::new(
                                                crate::sample_drop::SampleSource::Project { index },
                                                "Sample".into(),
                                                event.position,
                                            ));
                                    }
                                    cx.notify();
                                    return;
                                }
                                this.document.playlist.drag = Some(Drag {
                                    mode: Default::default(),
                                    kind,
                                    resource: id.clone(),
                                    clip: None,
                                    length,
                                    anchor: event.position,
                                    offset: 0.,
                                    target: None,
                                    moved: false,
                                });
                            }
                            cx.notify();
                        }),
                    ),
            );
        }
    }
    list
}
