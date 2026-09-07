//! Native vector readouts. These elements intentionally have no musical input handlers.
use crate::{theme::Theme, ui::alpha};
use gpui::{prelude::*, *};

pub fn dial(fraction: f32, bipolar: bool, theme: Theme) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |at, _, window, _| {
            let center = at.origin + point(at.size.width / 2., px(29.));
            let arc = |from: f32, to: f32, radius: f32, width: f32, color, window: &mut Window| {
                let mut path = PathBuilder::stroke(px(width));
                for i in 0..=48 {
                    let angle = (-225. + 270. * (from + (to - from) * i as f32 / 48.)).to_radians();
                    let point = center + point(px(radius * angle.cos()), px(radius * angle.sin()));
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
            arc(0., 1., 24., 3., rgb(theme.border), window);
            arc(
                if bipolar { 0.5 } else { 0. },
                fraction,
                24.,
                3.,
                rgb(theme.accent),
                window,
            );
            window.paint_quad(
                fill(
                    Bounds::new(center - point(px(19.), px(17.)), size(px(38.), px(38.))),
                    alpha(theme.bg, 0.8),
                )
                .corner_radii(px(19.)),
            );
            window.paint_quad(
                fill(
                    Bounds::new(center - point(px(19.), px(19.)), size(px(38.), px(38.))),
                    rgb(theme.button),
                )
                .corner_radii(px(19.)),
            );
            arc(0.05, 0.65, 17., 1., alpha(theme.muted, 0.3), window);
            let angle = (-225. + fraction * 270.).to_radians();
            let mut line = PathBuilder::stroke(px(2.));
            line.move_to(center + point(px(angle.cos() * 8.), px(angle.sin() * 8.)));
            line.line_to(center + point(px(angle.cos() * 16.), px(angle.sin() * 16.)));
            if let Ok(path) = line.build() {
                window.paint_path(path, rgb(theme.text));
            }
        },
    )
    .w_full()
    .h(px(58.))
}

pub fn envelope(values: [f64; 4], theme: Theme) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |at, _, window, _| {
            let [a, d, s, r] = values;
            // Source ADSR schematic; fixed sustain hold, no invented effective/live trace.
            let hold = ((a + d + r) * 0.3).max(0.05);
            let total = (a + d + hold + r).max(0.001);
            let w = f32::from(at.size.width) - 20.;
            let h = f32::from(at.size.height) - 16.;
            for i in 0..=4 {
                window.paint_quad(fill(
                    Bounds::new(
                        at.origin + point(px(10. + w * i as f32 / 4.), px(8.)),
                        size(px(1.), px(h)),
                    ),
                    alpha(theme.border, 0.5),
                ));
            }
            let mut path = PathBuilder::stroke(px(2.));
            for (i, (x, y)) in [
                (0., 0.),
                (a, 1.),
                (a + d, s),
                (a + d + hold, s),
                (total, 0.),
            ]
            .into_iter()
            .enumerate()
            {
                let p = at.origin
                    + point(
                        px(10. + w * (x / total) as f32),
                        px(8. + h * (1. - y as f32)),
                    );
                if i == 0 {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(theme.accent));
            }
        },
    )
    .w_full()
    .h(px(76.))
}
