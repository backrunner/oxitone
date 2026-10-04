//! Dial paint only; shared gesture wrappers own input and source transactions.
use crate::{theme::Theme, ui::alpha};
use gpui::{prelude::*, *};

pub fn dial(fraction: f32, bipolar: bool, theme: Theme) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |at, _, window, _| {
            let center = at.origin + point(at.size.width / 2., px(29.));
            let position = |fraction: f32, radius: f32| {
                let angle = (-225. + 270. * fraction).to_radians();
                center + point(px(radius * angle.cos()), px(radius * angle.sin()))
            };
            for i in 0..=10 {
                let at = position(i as f32 / 10., 27.);
                window.paint_quad(fill(
                    Bounds::new(at - point(px(0.7), px(0.7)), size(px(1.4), px(1.4))),
                    alpha(theme.muted, if i == 5 && bipolar { 0.8 } else { 0.35 }),
                ));
            }
            let arc = |from: f32, to: f32, width: f32, color: Hsla, window: &mut Window| {
                let mut path = PathBuilder::stroke(px(width));
                for i in 0..=48 {
                    let point = position(from + (to - from) * i as f32 / 48., 23.);
                    if i == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            };
            arc(0., 1., 2.5, rgb(theme.border).into(), window);
            let start = if bipolar { 0.5 } else { 0. };
            arc(
                start,
                fraction,
                6.,
                alpha(theme.accent, 0.10).into(),
                window,
            );
            arc(start, fraction, 2.5, rgb(theme.accent).into(), window);
            window.paint_quad(quad(
                Bounds::new(center - point(px(18.), px(18.)), size(px(36.), px(36.))),
                px(18.),
                rgb(theme.raised),
                px(1.),
                alpha(theme.muted, 0.15),
                BorderStyle::default(),
            ));
            let mut line = PathBuilder::stroke(px(2.));
            line.move_to(position(fraction, 9.));
            line.line_to(position(fraction, 15.));
            if let Ok(path) = line.build() {
                window.paint_path(path, rgb(theme.accent));
            }
        },
    )
    .w_full()
    .h(px(58.))
}
