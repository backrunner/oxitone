//! Read-only timeline. Clicking clips selects a pattern; ruler clicks seek.
use crate::ui::{alpha, color, label, Preview, BORDER, GOLD, MUTED, PANEL};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let project = this.project.as_ref().unwrap();
    let zoom = this.zoom;
    let width = (project.end() as f32 * zoom).max(900.);
    let cursor = project.beat(this.playback.audible) as f32 * zoom;
    let mut ruler = div().relative().h(px(28.)).w(px(width)).bg(rgb(PANEL));
    // One seek target per beat. Labels reflect the actual time-signature map.
    let step = ((20. / zoom).ceil() as usize).max(1);
    for beat in (0..project.end().ceil() as usize)
        .step_by(step)
        .take(10_000)
    {
        let (bar, within) = project
            .plan
            .time_signatures
            .beat_to_bar_beat(oxitone_core::Beat::new(beat as i64, 1).unwrap());
        ruler = ruler.child(
            div()
                .id(("beat", beat))
                .absolute()
                .left(px(beat as f32 * zoom))
                .w(px(zoom * step as f32))
                .h_full()
                .border_l_1()
                .border_color(rgb(BORDER))
                .px_1()
                .text_xs()
                .text_color(rgb(MUTED))
                .cursor_pointer()
                .child(if within == oxitone_core::Beat::ZERO {
                    format!("{bar}")
                } else {
                    "·".into()
                })
                .on_click(cx.listener(move |this, _, _, _| this.seek(beat as f64))),
        );
    }
    let mut rows = div().w(px(width + 184.)).flex().flex_col().child(
        div()
            .flex()
            .child(
                div()
                    .w(px(184.))
                    .h(px(28.))
                    .px_4()
                    .child(label("ARRANGEMENT")),
            )
            .child(ruler),
    );
    for (index, track) in project.snapshot.tracks.iter().enumerate() {
        let tint = color(index);
        let mut lane = div()
            .relative()
            .w(px(width))
            .h(px(57.))
            .bg(rgb(if index % 2 == 0 { 0x141c27 } else { 0x111822 }))
            .overflow_hidden();
        for clip in project
            .snapshot
            .pattern_clips
            .iter()
            .filter(|c| c.track_id == track.id)
        {
            let (start, end) = project.clip_bounds(clip);
            let id = clip.id.clone();
            let name = project
                .snapshot
                .patterns
                .iter()
                .find(|p| p.id == clip.pattern_id)
                .and_then(|p| p.name.clone())
                .unwrap_or_else(|| clip.pattern_id.clone());
            let selected = this.selected_clip.as_ref() == Some(&id);
            lane = lane.child(
                div()
                    .id(SharedString::from(id.clone()))
                    .absolute()
                    .left(px(start as f32 * zoom))
                    .top(px(6.))
                    .w(px(((end - start) as f32 * zoom - 2.).max(2.)))
                    .h(px(45.))
                    .overflow_hidden()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(if selected { 0xffffff } else { tint }))
                    .bg(alpha(
                        tint,
                        if clip.enabled == Some(false) {
                            0.08
                        } else {
                            0.23
                        },
                    ))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(tint))
                    .cursor_pointer()
                    .child(name)
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(format!("{:.1} beats", end - start)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_clip = Some(id.clone());
                        cx.notify();
                    })),
            );
        }
        for clip in project
            .snapshot
            .sample_clips
            .iter()
            .filter(|c| c.track_id == track.id)
        {
            if let Some((_, start, end)) = project.plan.samples.iter().find(|c| c.0 == clip.id) {
                let start = project.beat(*start);
                let end = project.beat(*end);
                lane = lane.child(
                    div()
                        .absolute()
                        .left(px(start as f32 * zoom))
                        .top(px(7.))
                        .w(px(((end - start) as f32 * zoom - 2.).max(2.)))
                        .h(px(43.))
                        .overflow_hidden()
                        .rounded_sm()
                        .bg(alpha(tint, 0.18))
                        .border_1()
                        .border_color(rgb(tint))
                        .px_2()
                        .text_xs()
                        .child(format!("Audio · {}", clip.sample_id)),
                );
            }
        }
        if this.loop_enabled {
            lane = lane.child(
                div()
                    .absolute()
                    .left(px(this.loop_start as f32 * zoom))
                    .top_0()
                    .w(px(((this.loop_end - this.loop_start) as f32 * zoom).max(0.)))
                    .h(px(2.))
                    .bg(rgb(GOLD)),
            );
        }
        lane = lane.child(
            div()
                .absolute()
                .left(px(cursor))
                .top_0()
                .w(px(1.))
                .h_full()
                .bg(rgb(GOLD)),
        );
        rows = rows.child(
            div()
                .flex()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .w(px(184.))
                        .flex_shrink_0()
                        .px_4()
                        .py_2()
                        .border_l_2()
                        .border_color(rgb(tint))
                        .bg(rgb(PANEL))
                        .child(
                            div()
                                .text_sm()
                                .child(track.name.clone().unwrap_or_else(|| track.id.clone())),
                        )
                        .child(div().text_xs().text_color(rgb(MUTED)).child(format!(
                            "{} channels{}",
                            track.channel_ids.len(),
                            if track.enabled == Some(false) {
                                " · disabled"
                            } else {
                                ""
                            }
                        ))),
                )
                .child(lane),
        );
    }
    div()
        .id("arrangement-scroll")
        .flex_1()
        .min_h(px(120.))
        .overflow_scroll()
        .child(rows)
}
