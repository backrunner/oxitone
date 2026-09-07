use crate::{
    mixer_model::{self, Strip},
    ui::Preview,
    workspace::Axis,
};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, strip: &Strip, cx: &mut Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    let mut content = div()
        .w_full()
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(strip.name.clone()),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme.muted))
                .child(strip.kind.clone()),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(9.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme.muted))
                .child("EFFECT SLOTS"),
        );
    for (i, (effect, bypass)) in strip.effects.iter().enumerate() {
        content = content.child(
            div()
                .px_2()
                .py_2()
                .rounded_sm()
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(rgb(theme.border))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(if *bypass { theme.muted } else { theme.accent }))
                        .child(format!("{:02}  {effect}", i + 1)),
                )
                .when(*bypass, |d| {
                    d.child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(theme.muted))
                            .child("Bypassed in source"),
                    )
                }),
        );
    }
    if strip.effects.is_empty() {
        content = content.child(
            div()
                .p_2()
                .rounded_sm()
                .border_1()
                .border_color(rgb(theme.border))
                .text_xs()
                .text_color(rgb(theme.muted))
                .child("No inserts"),
        );
    }
    content = content.child(
        div()
            .mt_2()
            .text_size(px(9.))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(theme.muted))
            .child("ROUTING"),
    );
    for route in &strip.routes {
        content = content.child(
            div()
                .text_xs()
                .text_color(rgb(theme.muted))
                .child(route.clone()),
        );
    }
    if strip.id == "mix_master" {
        let true_peak = this.analysis.get(&strip.id).map_or(0., |a| a.true_peak);
        content = content
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme.muted))
                    .child("Stereo output"),
            )
            .child(
                div()
                    .mt_2()
                    .text_xs()
                    .text_color(rgb(theme.gold))
                    .child(format!("{} dBTP · scope", mixer_model::db(true_peak))),
            );
    }
    if let Some(a) = this.analysis.get(&strip.id).filter(|a| a.dropped > 0) {
        content = content.child(
            div()
                .text_xs()
                .text_color(rgb(theme.gold))
                .child(format!("{} analysis drops", a.dropped)),
        );
    }
    div()
        .w(px(172.))
        .flex_shrink_0()
        .h_full()
        .border_l_1()
        .border_color(rgb(theme.border))
        .flex()
        .child(
            div()
                .id("mixer-inspector-scroll")
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_y_scroll()
                .track_scroll(&this.workspace.inspector)
                .child(content),
        )
        .child(crate::scrollbar::view(
            "inspector-scrollbar",
            Axis::Vertical,
            &this.workspace.inspector,
            this,
            cx,
        ))
}
