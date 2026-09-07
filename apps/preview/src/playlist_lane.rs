use crate::{
    pattern_preview,
    ui::{alpha, Preview},
};
use gpui::{prelude::*, *};
const ROW: f32 = 52.;

pub fn rows(
    this: &Preview,
    width: f32,
    cursor: f32,
    grid_ticks: &[(f32, bool)],
    cx: &mut Context<Preview>,
) -> (Div, Div) {
    let theme = this.theme;
    let project = this.project.as_ref().unwrap();
    let zoom = this.zoom;
    let offset = this.workspace.arrangement.offset();
    let mut lanes = div().w(px(width)).flex().flex_col();
    let mut headers = div().absolute().top(offset.y).w_full().flex().flex_col();
    for (index, track) in project.snapshot.tracks.iter().enumerate() {
        let tint = theme.track(index);
        let mut lane = div()
            .relative()
            .w(px(width))
            .h(px(ROW))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(rgb(theme.lanes[index % 2]))
            .border_b_1()
            .border_color(alpha(theme.border, 0.6));
        for (x, downbeat) in grid_ticks {
            lane = lane.child(
                div()
                    .absolute()
                    .left(px(*x))
                    .w(px(1.))
                    .h_full()
                    .bg(alpha(theme.border, if *downbeat { 0.8 } else { 0.25 })),
            );
        }
        for clip in project
            .snapshot
            .pattern_clips
            .iter()
            .filter(|c| c.track_id == track.id)
        {
            let (start, end) = project.clip_bounds(clip);
            let id = clip.id.clone();
            let selected = this.selected_clip.as_ref() == Some(&id);
            let enabled = clip.enabled != Some(false);
            lane = lane.child(
                div()
                    .id(SharedString::from(id.clone()))
                    .absolute()
                    .left(px(start as f32 * zoom))
                    .top(px(4.))
                    .w(px(((end - start) as f32 * zoom - 2.).max(2.)))
                    .h(px(43.))
                    .rounded_sm()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(if selected { theme.text } else { tint }))
                    .bg(alpha(
                        tint,
                        if enabled {
                            theme.clip_opacity
                        } else {
                            theme.clip_opacity * 0.35
                        },
                    ))
                    .hover(move |style| style.border_color(rgb(theme.text)))
                    .cursor_pointer()
                    .child(
                        div()
                            .px_1()
                            .h(px(17.))
                            .overflow_hidden()
                            .bg(alpha(tint, 0.08))
                            .text_size(px(9.))
                            .text_color(rgb(tint))
                            .child(
                                div()
                                    .truncate()
                                    .child(project.pattern_label(&clip.pattern_id)),
                            ),
                    )
                    .child(div().h(px(24.)).px_1().child(pattern_preview::thumbnail(
                        project.clone(),
                        clip.clone(),
                        tint,
                    )))
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
                let name = project
                    .snapshot
                    .samples
                    .iter()
                    .find(|s| s.id == clip.sample_id)
                    .and_then(|s| {
                        std::path::Path::new(&s.asset_uri)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| "Audio clip".into());
                lane = lane.child(
                    div()
                        .absolute()
                        .left(px(project.beat(*start) as f32 * zoom))
                        .top(px(4.))
                        .w(px(((project.beat(*end) - project.beat(*start)) as f32
                            * zoom
                            - 2.)
                            .max(2.)))
                        .h(px(43.))
                        .rounded_sm()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(tint))
                        .bg(alpha(tint, theme.clip_opacity))
                        .px_2()
                        .child(div().text_xs().truncate().child(name))
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(rgb(theme.muted))
                                .child("AUDIO"),
                        ),
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
                    .bg(rgb(theme.gold)),
            );
        }
        lane = lane.child(
            div()
                .absolute()
                .left(px(cursor))
                .top_0()
                .w(px(1.))
                .h_full()
                .bg(rgb(theme.gold)),
        );
        lanes = lanes.child(lane);
        headers = headers.child(
            div()
                .h(px(ROW))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .border_b_1()
                .border_color(rgb(theme.border))
                .child(div().w(px(3.)).h(px(32.)).rounded_full().bg(rgb(tint)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(
                                    track
                                        .name
                                        .clone()
                                        .unwrap_or_else(|| format!("Track {}", index + 1)),
                                ),
                        )
                        .child(div().text_size(px(9.)).text_color(rgb(theme.muted)).child(
                            format!(
                                "{:02}  ·  {} channel{}{}",
                                index + 1,
                                track.channel_ids.len(),
                                if track.channel_ids.len() == 1 {
                                    ""
                                } else {
                                    "s"
                                },
                                if track.enabled == Some(false) {
                                    " · off"
                                } else {
                                    ""
                                }
                            ),
                        )),
                ),
        );
    }
    (lanes, headers)
}
