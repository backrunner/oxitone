//! A shared chart surface: calibrated axes, understated fills and bounded cached vector paths.
use crate::{plugin_plot::Plot, theme::Theme, ui::alpha};
use gpui::{prelude::*, *};
use std::sync::Arc;

pub fn view(plots: Arc<Vec<Plot>>, theme: Theme, width: f32) -> Div {
    let columns = if plots.len() > 1 && width >= 640. {
        2
    } else {
        1
    };
    let mut row = div().w_full().flex().flex_wrap().gap_3();
    for index in 0..plots.len() {
        let plot = &plots[index];
        let data = plots.clone();
        let mut legend = div().flex().flex_wrap().gap_3().items_center();
        for trace in plot.traces.iter().filter(|t| !t.label.is_empty()) {
            legend = legend.child(
                div()
                    .flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .w(px(12.))
                            .h(px(2.))
                            .bg(rgb(color(theme, trace.color))),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme.muted))
                            .child(trace.label.clone()),
                    ),
            );
        }
        row = row.child(
            div()
                .flex_1()
                .min_w_0()
                .flex_basis(px((width - 36.) / columns as f32 - 12.))
                .rounded_lg()
                .bg(rgb(theme.scope))
                .overflow_hidden()
                .child(
                    div()
                        .px_3()
                        .pt_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(plot.title.clone()),
                        )
                        .child(legend),
                )
                .child(
                    div()
                        .px_3()
                        .pt_1()
                        .text_size(px(10.))
                        .text_color(rgb(theme.muted))
                        .child(plot.detail.clone()),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            let plot = &data[index];
                            let at = Bounds::new(
                                bounds.origin + point(px(40.), px(14.)),
                                size(
                                    (bounds.size.width - px(54.)).max(px(1.)),
                                    (bounds.size.height - px(42.)).max(px(1.)),
                                ),
                            );
                            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                                for (x, label) in &plot.x {
                                    let position = at.origin + point(at.size.width * *x, px(0.));
                                    window.paint_quad(fill(
                                        Bounds::new(position, size(px(1.), at.size.height)),
                                        alpha(theme.border, 0.4),
                                    ));
                                    text(
                                        label,
                                        position + point(px(0.), at.size.height + px(8.)),
                                        theme.muted,
                                        Some(bounds),
                                        window,
                                        cx,
                                    );
                                }
                                for (y, label) in &plot.y {
                                    let position = at.origin + point(px(0.), at.size.height * *y);
                                    window.paint_quad(fill(
                                        Bounds::new(position, size(at.size.width, px(1.))),
                                        alpha(theme.border, 0.45),
                                    ));
                                    text(
                                        label,
                                        position - point(px(35.), px(6.)),
                                        theme.muted,
                                        None,
                                        window,
                                        cx,
                                    );
                                }
                                window.with_content_mask(
                                    Some(ContentMask { bounds: at }),
                                    |window| {
                                        for region in &plot.regions {
                                            let [x, y, w, h] = region.rect;
                                            let area = Bounds::new(
                                                at.origin
                                                    + point(at.size.width * x, at.size.height * y),
                                                size(at.size.width * w, at.size.height * h),
                                            );
                                            window.paint_quad(quad(
                                                area,
                                                px(2.),
                                                alpha(color(theme, region.color), 0.15),
                                                px(1.),
                                                alpha(color(theme, region.color), 0.6),
                                                BorderStyle::default(),
                                            ));
                                            if area.size.width > px(32.)
                                                && area.size.height > px(17.)
                                            {
                                                text(
                                                    &region.label,
                                                    area.origin + point(px(5.), px(4.)),
                                                    theme.text,
                                                    None,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        }
                                        for trace in &plot.traces {
                                            crate::plugin_visuals::trace(
                                                at,
                                                trace.points.iter().copied(),
                                                rgb(color(theme, trace.color)).into(),
                                                1.8,
                                                window,
                                            );
                                        }
                                    },
                                );
                            });
                        },
                    )
                    .w_full()
                    .h(px(168.)),
                ),
        );
    }
    row
}
fn color(theme: Theme, index: usize) -> u32 {
    [theme.accent, theme.gold, theme.muted, theme.text][index % 4]
}
fn text(
    label: &str,
    mut at: Point<Pixels>,
    color: u32,
    centered: Option<Bounds<Pixels>>,
    window: &mut Window,
    cx: &mut App,
) {
    let run = TextRun {
        len: label.len(),
        font: font("Helvetica Neue"),
        color: rgb(color).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(label.to_owned().into(), px(9.), &[run], None);
    if let Some(bounds) = centered {
        at.x = (at.x - line.width / 2.).clamp(
            bounds.left() + px(4.),
            (bounds.right() - line.width - px(4.)).max(bounds.left() + px(4.)),
        );
    }
    let _ = line.paint(at, px(12.), window, cx);
}
