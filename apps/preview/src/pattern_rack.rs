//! Pattern-focused channel rack; the asset Browser remains a separate view.
use crate::{
    ui::Preview,
    ui_icons::{icon, Icon},
    workspace_layout::EditorMode,
};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let mut rack = div().size_full().flex().flex_col();
    let Some(project) = this.project.clone() else {
        return rack;
    };
    let Some(pattern) = this.active_pattern() else {
        return rack.child(
            div()
                .p_4()
                .text_sm()
                .text_color(rgb(t.muted))
                .child("No patterns in this project."),
        );
    };
    rack = rack.child(
        div()
            .h(px(48.))
            .px_3()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .child(icon(Icon::Arrange, t.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(project.pattern_label(&pattern.id)),
            )
            .child(t.label(format!(
                "{} {}",
                pattern.length_beats.to_f64(),
                if pattern.length_beats.to_f64() == 1. {
                    "beat"
                } else {
                    "beats"
                }
            )))
            .child(
                t.icon_button("rack-piano", Icon::Piano, "Open piano roll")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.float_editor(EditorMode::Piano);
                        cx.notify();
                    })),
            ),
    );
    let mut voices: Vec<(Option<String>, String)> = if let Some(parts) = &pattern.parts {
        parts
            .iter()
            .map(|part| (Some(part.channel_id.clone()), part.pattern_id.clone()))
            .collect()
    } else {
        project
            .snapshot
            .pattern_clips
            .iter()
            .filter(|c| c.pattern_id == pattern.id)
            .filter_map(|c| project.snapshot.tracks.iter().find(|t| t.id == c.track_id))
            .flat_map(|track| {
                track
                    .channel_ids
                    .iter()
                    .map(|id| (Some(id.clone()), pattern.id.clone()))
            })
            .collect()
    };
    voices.sort();
    voices.dedup();
    if voices.is_empty() {
        voices.push((None, pattern.id.clone()));
    }
    let mut rows = div()
        .id("pattern-rack-rows")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .px_2()
        .gap_1();
    for (index, (channel_id, pattern_id)) in voices.into_iter().enumerate() {
        let channel = channel_id
            .as_ref()
            .and_then(|id| project.snapshot.channels.iter().find(|c| &c.id == id));
        let label = channel.and_then(|c| c.name.clone()).unwrap_or_else(|| {
            if channel.is_some() {
                format!("Channel {}", index + 1)
            } else {
                "Unassigned pattern".into()
            }
        });
        let plugin = channel
            .map(|c| c.instrument.plugin_id.clone())
            .unwrap_or_else(|| "No channel placement".into());
        let source = project.clone();
        let tint = t.track(index);
        let notes = project
            .snapshot
            .patterns
            .iter()
            .find(|p| p.id == pattern_id)
            .map_or(0, |p| p.notes.len());
        let mut row = div()
            .h(px(62.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .rounded_md()
            .bg(rgb(t.raised))
            .child(div().w(px(3.)).h(px(32.)).rounded_full().bg(rgb(tint)))
            .child(
                div()
                    .w(px(140.))
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(12.)).truncate().child(label))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(t.muted))
                            .truncate()
                            .child(plugin),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(38.))
                    .min_w_0()
                    .bg(rgb(t.scope))
                    .rounded_md()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |area, _, window, _| {
                                let Some(p) =
                                    source.snapshot.patterns.iter().find(|p| p.id == pattern_id)
                                else {
                                    return;
                                };
                                let low = p.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
                                let high = p.notes.iter().map(|n| n.pitch).max().unwrap_or(72);
                                let w = f32::from(area.size.width);
                                let h = f32::from(area.size.height);
                                window.with_content_mask(
                                    Some(ContentMask { bounds: area }),
                                    |window| {
                                        for note in &p.notes {
                                            let x = note.start.to_f64()
                                                / p.length_beats.to_f64().max(0.25)
                                                * f64::from(w);
                                            let width = note.duration.to_f64()
                                                / p.length_beats.to_f64().max(0.25)
                                                * f64::from(w);
                                            let y = 5.
                                                + f32::from(high - note.pitch)
                                                    / f32::from((high - low).max(12))
                                                    * (h - 12.).max(1.);
                                            window.paint_quad(fill(
                                                Bounds::new(
                                                    area.origin + point(px(x as f32), px(y)),
                                                    size(px((width as f32).max(2.)), px(3.)),
                                                ),
                                                rgb(tint),
                                            ));
                                        }
                                    },
                                );
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .text_size(px(9.))
                    .text_color(rgb(t.muted))
                    .child(format!("{notes}")),
            );
        if let Some(id) = channel_id.clone() {
            row = row.child(
                t.icon_button(
                    format!("rack-instrument-{index}"),
                    Icon::Wave,
                    "Open instrument",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let _ = this.open_plugin(
                        crate::plugin_details::DetailTarget::Instrument(id.clone()),
                        cx,
                    );
                    cx.notify();
                })),
            );
        }
        rows = rows.child(
            row.child(
                t.icon_button(format!("rack-notes-{index}"), Icon::Piano, "Edit notes")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        crate::pointer_capture::cancel(this);
                        this.piano.channel = channel_id.clone();
                        this.piano.fit();
                        this.document.notes.clear();
                        this.float_editor(EditorMode::Piano);
                        cx.notify();
                    })),
            ),
        );
    }
    rack.child(rows)
}
