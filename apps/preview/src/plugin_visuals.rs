//! Source-parameter displays. Drawing and response analysis stay on the UI thread.
use crate::{plugin_details::PluginDetails, plugin_layout::Control, theme::Theme, ui::alpha};
use gpui::{prelude::*, *};

pub fn view(
    control: &Control,
    details: &PluginDetails,
    theme: Theme,
    sample_rate: f64,
    stacked: bool,
) -> Div {
    let parameter = |id: &str| {
        details
            .parameters
            .iter()
            .find(|p| !p.host && p.spec.id == id)
    };
    let values: Vec<_> = control
        .bindings()
        .iter()
        .map(|id| parameter(id).map_or(0., |p| p.value))
        .collect();
    let root = div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .mb_2()
        .overflow_hidden();
    match control {
        Control::Envelope { .. } => root.child(crate::plugin_dial::envelope(
            [values[0], values[1], values[2], values[3]],
            theme,
        )),
        Control::Oscillator { .. } => crate::plugin_wave_view::oscillator(&values, theme, stacked),
        Control::FilterResponse { .. } => {
            crate::plugin_response_view::filter(&values, theme, sample_rate)
        }
        Control::LfoCurve { .. } => crate::plugin_response_view::lfo(&values, theme),
        Control::Modulation { routes, .. } => {
            let mut rows = div().w_full().flex().flex_col().gap_1();
            for route in routes {
                let Some(p) = parameter(&route.amount) else {
                    continue;
                };
                let strength =
                    (p.value.abs() / p.spec.min.abs().max(p.spec.max.abs()).max(0.001)) as f32;
                let bipolar = p.spec.min < 0.;
                let width = strength * if bipolar { 0.5 } else { 1. };
                let start = if bipolar {
                    0.5 - if p.value < 0. { width } else { 0. }
                } else {
                    0.
                };
                rows = rows.child(
                    div()
                        .h(px(32.))
                        .px_2()
                        .rounded_md()
                        .bg(rgb(theme.scope))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size(px(5.)).rounded_full().bg(rgb(if strength > 0. {
                            theme.accent
                        } else {
                            theme.border
                        })))
                        .child(div().flex_1().text_size(px(11.)).child(route.label.clone()))
                        .child(
                            div()
                                .relative()
                                .w(px(58.))
                                .h(px(3.))
                                .bg(rgb(theme.border))
                                .child(
                                    div()
                                        .absolute()
                                        .left(relative(start))
                                        .w(relative(width))
                                        .h_full()
                                        .bg(rgb(theme.accent)),
                                )
                                .when(bipolar, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .left(relative(0.5))
                                            .top(px(-2.))
                                            .w(px(1.))
                                            .h(px(7.))
                                            .bg(rgb(theme.muted)),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .w(px(80.))
                                .text_right()
                                .text_size(px(11.))
                                .text_color(rgb(if strength > 0. {
                                    theme.accent
                                } else {
                                    theme.muted
                                }))
                                .child(crate::plugin_controls::value(p)),
                        ),
                );
            }
            rows
        }
        _ => root,
    }
}

pub fn caption(left: String, right: String, theme: Theme) -> Div {
    div()
        .px_2()
        .h(px(24.))
        .flex()
        .items_center()
        .justify_between()
        .text_size(px(10.))
        .child(div().text_color(rgb(theme.text)).child(left))
        .child(div().text_color(rgb(theme.muted)).child(right))
}
pub fn grid(at: Bounds<Pixels>, theme: Theme, window: &mut Window) {
    for i in 1..8 {
        window.paint_quad(fill(
            Bounds::new(
                at.origin + point(at.size.width * i as f32 / 8., px(0.)),
                size(px(1.), at.size.height),
            ),
            alpha(theme.border, 0.3),
        ));
    }
    for i in 1..4 {
        window.paint_quad(fill(
            Bounds::new(
                at.origin + point(px(0.), at.size.height * i as f32 / 4.),
                size(at.size.width, px(1.)),
            ),
            alpha(theme.border, 0.3),
        ));
    }
}
pub fn trace(
    at: Bounds<Pixels>,
    points: impl Iterator<Item = (f32, f32)>,
    color: Hsla,
    stroke: f32,
    window: &mut Window,
) {
    let mut path = PathBuilder::stroke(px(stroke));
    for (i, (x, y)) in points.enumerate() {
        let p = at.origin + point(at.size.width * x, at.size.height * y);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
