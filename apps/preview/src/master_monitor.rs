//! A bounded display of the existing post-limiter Master telemetry; no second ring reader.
use crate::ui::Preview;
use gpui::{prelude::*, *};

pub fn view(this: &Preview) -> impl IntoElement {
    let t = this.theme;
    let node = this.analysis.get("mix_master");
    let peak = node.map_or(0., |n| n.peak);
    let rms = node.map_or(0., |n| n.rms);
    let wave = if this.playback.playing {
        node.map(|n| n.wave.clone()).unwrap_or_default()
    } else {
        vec![]
    };
    let db = if peak < 0.000_001 {
        "−∞".into()
    } else {
        format!("{:.1}", 20. * peak.log10())
    };
    div()
        .w(px(208.))
        .h(px(34.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .rounded_md()
        .bg(rgb(t.scope))
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(82.))
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(8.))
                        .text_color(rgb(t.muted))
                        .child("MASTER")
                        .child(format!("{db} dB")),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        move |area, _, window, _| {
                            let w = f32::from(area.size.width);
                            let h = f32::from(area.size.height);
                            window.paint_quad(fill(
                                Bounds::new(
                                    area.origin + point(px(0.), px(h * 0.5)),
                                    size(px(w), px(1.)),
                                ),
                                rgb(t.border),
                            ));
                            for (channel, tint) in [(0, t.accent), (1, t.secondary)] {
                                let bucket = wave.len().div_ceil(80).max(1);
                                for (i, values) in wave.chunks(bucket).enumerate() {
                                    let (low, high) =
                                        values.iter().fold((1_f32, -1_f32), |(low, high), s| {
                                            (low.min(s[channel]), high.max(s[channel]))
                                        });
                                    let top = h * 0.5 - high.clamp(-1., 1.) * h * 0.48;
                                    let bottom = h * 0.5 - low.clamp(-1., 1.) * h * 0.48;
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            area.origin + point(px(i as f32 / 80. * w), px(top)),
                                            size(px(1.), px((bottom - top).max(0.6))),
                                        ),
                                        rgb(tint),
                                    ));
                                }
                            }
                        },
                    )
                    .w_full()
                    .h(px(17.)),
                ),
        )
        .child(
            div().flex_1().flex().flex_col().gap(px(3.)).children(
                [("PK", peak), ("RMS", rms)]
                    .into_iter()
                    .map(move |(label, value)| {
                        let level = ((20. * value.max(1e-6).log10() + 60.) / 60.).clamp(0., 1.);
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(22.))
                                    .text_size(px(7.))
                                    .text_color(rgb(t.muted))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(5.))
                                    .rounded_sm()
                                    .bg(rgb(t.meter))
                                    .child(div().w(relative(level)).h_full().rounded_sm().bg(rgb(
                                        if value >= 1. {
                                            t.danger
                                        } else if value >= 0.5 {
                                            t.gold
                                        } else {
                                            t.accent
                                        },
                                    ))),
                            )
                    }),
            ),
        )
}
