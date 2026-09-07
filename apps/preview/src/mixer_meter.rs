//! Meter/fader readouts: native canvas painting, no musical gestures or setters.
use crate::{theme::Theme, ui::alpha};
use gpui::{prelude::*, *};

pub fn meter(level: f32, peak: f32, rms: f32, tint: u32, theme: Theme) -> impl IntoElement {
    let scale = |db: f32| (1. - (db + 60.) / 66.).clamp(0., 1.) * 130.;
    let mut root = div()
        .relative()
        .w_full()
        .h(px(160.))
        .flex_shrink_0()
        .rounded_md()
        .bg(rgb(theme.scope))
        .border_1()
        .border_color(alpha(theme.border, 0.55));
    for v in [0., -12., -24., -48.] {
        root = root.child(
            div()
                .absolute()
                .left(px(4.))
                .top(px(14. + scale(v)))
                .text_size(px(8.))
                .text_color(rgb(theme.muted))
                .child(format!("{v:.0}")),
        );
    }
    root.child(
        div()
            .absolute()
            .top(px(2.))
            .right(px(5.))
            .text_size(px(7.))
            .text_color(rgb(theme.muted))
            .child("P  RMS"),
    )
    .child(
        canvas(
            |_, _, _| {},
            move |at, _, window, _| {
                let draw = |window: &mut Window, x, y, w, h, color, round| {
                    window.paint_quad(
                        fill(
                            Bounds::new(at.origin + point(px(x), px(y)), size(px(w), px(h))),
                            color,
                        )
                        .corner_radii(px(round)),
                    );
                };
                draw(window, 32., 17., 3., 132., rgb(theme.border), 1.5);
                for v in [6., 0., -12., -24., -36., -48., -60.] {
                    draw(
                        window,
                        27.,
                        17. + scale(v),
                        13.,
                        1.,
                        alpha(theme.muted, 0.45),
                        0.,
                    );
                }
                for (x, value, color) in [(51., peak, theme.accent), (61., rms, theme.secondary)] {
                    draw(window, x - 1., 16., 8., 133., rgb(theme.meter), 2.);
                    let db = 20. * value.max(1e-6).log10();
                    for i in 0..33 {
                        let tick = -60. + i as f32 * 2.;
                        let tone = if tick >= 0. {
                            theme.danger
                        } else if tick >= -6. {
                            theme.gold
                        } else {
                            color
                        };
                        draw(
                            window,
                            x,
                            145. - i as f32 * 4.,
                            6.,
                            3.,
                            alpha(tone, if db >= tick { 1. } else { 0.12 }),
                            0.6,
                        );
                    }
                }
                let y = 12. + scale(20. * level.max(1e-6).log10());
                draw(window, 23., y + 2., 23., 13., alpha(theme.bg, 0.9), 3.);
                draw(window, 22., y, 23., 13., rgb(theme.muted), 3.);
                draw(window, 23., y + 1., 21., 10., rgb(theme.button_hover), 2.);
                draw(window, 25., y + 6., 17., 1., rgb(tint), 0.);
            },
        )
        .size_full(),
    )
}

pub fn pan(value: f32, tint: u32, theme: Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .h(px(31.))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    let center = at.origin + point(px(12.), px(12.));
                    let mut arc = PathBuilder::stroke(px(2.));
                    for i in 0..=32 {
                        let angle = (-225. + i as f32 * 270. / 32.).to_radians();
                        let p = center + point(px(angle.cos() * 9.), px(angle.sin() * 9.));
                        if i == 0 {
                            arc.move_to(p);
                        } else {
                            arc.line_to(p);
                        }
                    }
                    if let Ok(path) = arc.build() {
                        window.paint_path(path, rgb(theme.border));
                    }
                    let angle = (-90. + value * 135.).to_radians();
                    let mut needle = PathBuilder::stroke(px(2.));
                    needle.move_to(center + point(px(angle.cos() * 3.), px(angle.sin() * 3.)));
                    needle.line_to(center + point(px(angle.cos() * 9.), px(angle.sin() * 9.)));
                    if let Ok(path) = needle.build() {
                        window.paint_path(path, rgb(tint));
                    }
                },
            )
            .size(px(24.)),
        )
        .child(
            div()
                .w(px(28.))
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .child(if value.abs() < 0.005 {
                    "C".into()
                } else {
                    format!(
                        "{} {:.0}",
                        if value < 0. { "L" } else { "R" },
                        value.abs() * 100.
                    )
                }),
        )
}
