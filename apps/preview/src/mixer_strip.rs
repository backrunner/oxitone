use crate::mixer_control::{control, toggle};
use crate::{
    mixer_model::{db, Strip, STRIP_WIDTH},
    ui::{alpha, Preview},
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn view(
    this: &Preview,
    strip: &Strip,
    related: Option<&'static str>,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
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
    let output = strip.output();
    let count = strip.sends().count();
    let level = this.mix_value(&strip.id, crate::mixer_edit::Control::Level, strip.level);
    let pan = this.mix_value(&strip.id, crate::mixer_edit::Control::Pan, strip.pan);
    div()
        .id(SharedString::from(format!("strip-{}", strip.id)))
        .w(px(STRIP_WIDTH))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(alpha(theme.border, 0.5))
        .bg(rgb(if selected {
            theme.selected
        } else {
            theme.panel
        }))
        .cursor_pointer()
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.select_mixer(&id);
            if event.click_count() == 2 {
                this.open_plugin(
                    crate::plugin_details::DetailTarget::Instrument(id.clone()),
                    cx,
                );
            }
            cx.notify();
        }))
        .child(
            div()
                .h(px(3.))
                .flex_shrink_0()
                .bg(alpha(tint, if selected { 1. } else { 0.55 })),
        )
        .child(
            div()
                .px_2()
                .pt_2()
                .h(px(61.))
                .flex_shrink_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(8.))
                        .text_color(rgb(tint))
                        .child(if master {
                            "M".into()
                        } else {
                            format!("{:02}", strip.index)
                        })
                        .child(related.unwrap_or(if master {
                            "OUT"
                        } else if strip.instrument {
                            "INST"
                        } else {
                            "BUS"
                        })),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(strip.name.clone()),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .truncate()
                        .child(strip.kind.clone()),
                ),
        )
        .child(
            control(this, strip, crate::mixer_edit::Control::Pan, pan, cx)
                .child(crate::mixer_meter::pan(pan as f32, tint, theme)),
        )
        .child(
            control(this, strip, crate::mixer_edit::Control::Level, level, cx)
                .px_2()
                .child(crate::mixer_meter::meter(
                    level as f32,
                    peak,
                    rms,
                    tint,
                    theme,
                )),
        )
        .child(
            div()
                .h(px(29.))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .items_center()
                .text_size(px(12.))
                .font_family("Menlo")
                .font_weight(FontWeight::MEDIUM)
                .child(format!("{} ", db(level as f32)))
                .child(
                    div()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child("dB"),
                ),
        )
        .child(
            div()
                .px_2()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(toggle(this, strip, false, cx))
                        .child(toggle(this, strip, true, cx)),
                )
                .when(!strip.effects.is_empty(), |row| {
                    row.child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(theme.muted))
                            .child(format!("{} FX", strip.effects.len())),
                    )
                }),
        )
        .child(div().flex_1().min_h(px(6.)))
        .child(
            div()
                .px_2()
                .py_2()
                .h(px(44.))
                .flex_shrink_0()
                .border_t_1()
                .border_color(alpha(theme.border, 0.6))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(9.))
                        .text_color(rgb(if related.is_some() { tint } else { theme.muted }))
                        .child(icon(
                            Icon::Output,
                            if related.is_some() { tint } else { theme.muted },
                        ))
                        .child(
                            div().min_w_0().truncate().child(
                                output
                                    .map_or("Device", |r| r.destination_name.as_str())
                                    .to_owned(),
                            ),
                        ),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(8.))
                        .text_color(rgb(theme.muted))
                        .truncate()
                        .child(if analysis.is_some_and(|a| a.dropped > 0) {
                            format!("{} analysis drops", analysis.unwrap().dropped)
                        } else if count > 0 {
                            format!("{count} send{}", if count == 1 { "" } else { "s" })
                        } else if master {
                            format!("{} inputs", strip.inputs.len())
                        } else if peak > 0. {
                            format!("{} peak", db(peak))
                        } else {
                            String::new()
                        }),
                ),
        )
}
