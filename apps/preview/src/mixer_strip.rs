use crate::{
    mixer_model::{db, Strip},
    ui::{alpha, Preview},
};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, strip: &Strip, cx: &mut Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    let master = strip.id == "mix_master";
    let tint = if master {
        theme.gold
    } else {
        theme.track(strip.color_index)
    };
    let selected = this.selected_scope == strip.id;
    let id = strip.id.clone();
    let analysis = this.analysis.get(&strip.id);
    let peak = analysis.map_or(0., |a| a.peak);
    let rms = analysis.map_or(0., |a| a.rms);
    let level = strip.level as f32;
    let pan = strip.pan as f32;
    let mut meter = div().relative().h(px(142.)).w_full().flex_shrink_0();
    // Source fader and meter scales are visual readouts, not parameter controls.
    let scale = |db: f32| (1. - (db + 60.) / 66.).clamp(0., 1.) * 128.;
    meter = meter.child(
        div()
            .absolute()
            .left(px(12.))
            .top(px(4.))
            .w(px(4.))
            .h(px(128.))
            .rounded_full()
            .bg(rgb(theme.meter)),
    );
    for value in [6., 0., -12., -24., -36., -48., -60.] {
        meter = meter.child(
            div()
                .absolute()
                .left(px(7.))
                .top(px(4. + scale(value)))
                .w(px(14.))
                .h(px(1.))
                .bg(rgb(theme.border)),
        );
    }
    for (x, value, color) in [(30., peak, theme.accent), (39., rms, theme.secondary)] {
        let mut bar = div()
            .absolute()
            .left(px(x))
            .top(px(4.))
            .w(px(6.))
            .h(px(128.))
            .rounded_sm()
            .bg(rgb(theme.meter));
        let db = 20. * value.max(0.000001).log10();
        for segment in 0..32 {
            let at_db = -60. + segment as f32 * 66. / 32.;
            let lit = db >= at_db;
            let tone = if at_db >= 0. {
                theme.danger
            } else if at_db >= -6. {
                theme.gold
            } else {
                color
            };
            bar = bar.child(
                div()
                    .absolute()
                    .bottom(px(segment as f32 * 4.))
                    .w_full()
                    .h(px(3.))
                    .bg(alpha(tone, if lit { 1. } else { 0.08 })),
            );
        }
        meter = meter.child(bar);
    }
    for value in [0., -12., -24., -48.] {
        meter = meter.child(
            div()
                .absolute()
                .left(px(49.))
                .w(px(22.))
                .top(px(scale(value) - 1.))
                .text_size(px(8.))
                .whitespace_nowrap()
                .text_color(rgb(theme.muted))
                .child(format!("{value:.0}")),
        );
    }
    let fader_y = scale(20. * level.max(1e-6).log10());
    meter = meter.child(
        div()
            .absolute()
            .left(px(4.))
            .top(px(fader_y))
            .w(px(20.))
            .h(px(12.))
            .rounded_sm()
            .bg(rgb(theme.button_hover))
            .border_1()
            .border_color(rgb(theme.muted))
            .child(div().mt(px(5.)).h(px(1.)).w_full().bg(rgb(tint))),
    );
    div()
        .id(SharedString::from(format!("strip-{}", strip.id)))
        .w(px(84.))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(rgb(theme.border))
        .bg(rgb(if selected {
            theme.selected
        } else {
            theme.panel
        }))
        .hover(move |s| s.bg(rgb(theme.button)))
        .cursor_pointer()
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.selected_scope = id.clone();
            this.workspace.inspector.set_offset(point(px(0.), px(0.)));
            if event.click_count() == 2 {
                this.open_plugin(
                    crate::plugin_details::DetailTarget::Instrument(id.clone()),
                    cx,
                );
            }
            cx.notify();
        }))
        .child(div().h(px(3.)).flex_shrink_0().bg(rgb(tint)))
        .child(
            div()
                .px_2()
                .pt_1()
                .h(px(47.))
                .flex_shrink_0()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(10.))
                        .text_color(rgb(tint))
                        .child(if master {
                            "M".into()
                        } else {
                            format!("{:02}", strip.index)
                        })
                        .child(if selected { "●" } else { "○" }),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(strip.name.clone()),
                ),
        )
        .child(
            div()
                .px_2()
                .h(px(28.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .relative()
                        .size(px(20.))
                        .rounded_full()
                        .bg(rgb(theme.button))
                        .border_1()
                        .border_color(rgb(theme.border))
                        .child(
                            div()
                                .absolute()
                                .top(px(3.))
                                .left(px(8. + pan * 5.))
                                .w(px(2.))
                                .h(px(8.))
                                .bg(rgb(tint)),
                        ),
                )
                .child(div().text_size(px(9.)).text_color(rgb(theme.muted)).child(
                    if pan.abs() < 0.005 {
                        "C".into()
                    } else {
                        format!(
                            "{} {:.0}",
                            if pan < 0. { "L" } else { "R" },
                            pan.abs() * 100.
                        )
                    },
                )),
        )
        .child(div().px_1().child(meter))
        .child(
            div()
                .px_2()
                .text_size(px(8.))
                .text_color(rgb(theme.muted))
                .child("LEVEL   P / RMS"),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_family("Menlo")
                .text_color(rgb(theme.text))
                .child(format!("{} dB", db(level))),
        )
        .child(
            div()
                .px_2()
                .flex()
                .gap_1()
                .child(badge("M", strip.mute, theme))
                .child(badge("S", strip.solo, theme))
                .child(
                    div()
                        .flex_1()
                        .text_right()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child(format!("{} FX", strip.effects.len())),
                ),
        )
        .child(div().flex_1())
        .child(
            div()
                .h(px(23.))
                .px_2()
                .border_t_1()
                .border_color(rgb(theme.border))
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .child(format!("{} peak", db(peak))),
        )
}
fn badge(label: &'static str, active: bool, theme: crate::theme::Theme) -> Div {
    div()
        .w(px(18.))
        .h(px(16.))
        .rounded_sm()
        .text_center()
        .text_size(px(9.))
        .bg(rgb(if active { theme.gold } else { theme.button }))
        .text_color(rgb(if active { theme.bg } else { theme.muted }))
        .child(label)
}
