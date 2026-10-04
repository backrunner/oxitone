//! Chart paint primitives; bounded source vectors, with no audio access.
use crate::{plugin_plot::Plot, theme::Theme, ui::alpha};
use gpui::*;

pub fn plot(plot: &Plot, bounds: Bounds<Pixels>, theme: Theme, window: &mut Window, cx: &mut App) {
    let at = Bounds::new(
        bounds.origin + point(px(40.), px(18.)),
        size(
            (bounds.size.width - px(58.)).max(px(1.)),
            (bounds.size.height - px(48.)).max(px(1.)),
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
                position + point(px(0.), at.size.height + px(10.)),
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
                alpha(theme.border, 0.4),
            ));
            text(
                label,
                position - point(px(33.), px(6.)),
                theme.muted,
                None,
                window,
                cx,
            );
        }
        window.with_content_mask(Some(ContentMask { bounds: at }), |window| {
            for region in &plot.regions {
                let [x, y, w, h] = region.rect;
                let area = Bounds::new(
                    at.origin + point(at.size.width * x, at.size.height * y),
                    size(at.size.width * w, at.size.height * h),
                );
                window.paint_quad(quad(
                    area,
                    px(2.),
                    alpha(theme.track(region.color), 0.13),
                    px(1.),
                    alpha(theme.track(region.color), 0.6),
                    BorderStyle::default(),
                ));
                if area.size.width > px(32.) && area.size.height > px(17.) {
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
                let color = theme.track(trace.color);
                if trace.points.len() > 8 && trace.points.windows(2).all(|p| p[1].0 >= p[0].0) {
                    area(
                        at,
                        &trace.points,
                        plot.baseline,
                        alpha(color, if trace.color == 0 { 0.12 } else { 0.06 }).into(),
                        window,
                    );
                }
                let primary = trace.color == 0;
                if primary {
                    crate::plugin_visuals::trace(
                        at,
                        trace.points.iter().copied(),
                        alpha(color, 0.12).into(),
                        6.,
                        window,
                    );
                }
                crate::plugin_visuals::trace(
                    at,
                    trace.points.iter().copied(),
                    alpha(color, if primary { 1. } else { 0.75 }).into(),
                    if primary { 2.2 } else { 1.3 },
                    window,
                );
            }
        });
    });
}

pub fn area(
    at: Bounds<Pixels>,
    points: &[(f32, f32)],
    baseline: f32,
    color: Hsla,
    window: &mut Window,
) {
    let Some(first) = points.first() else {
        return;
    };
    let Some(last) = points.last() else {
        return;
    };
    let position = |x, y| at.origin + point(at.size.width * x, at.size.height * y);
    let mut path = PathBuilder::fill();
    path.move_to(position(first.0, baseline));
    for &(x, y) in points {
        path.line_to(position(x, y));
    }
    path.line_to(position(last.0, baseline));
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

pub fn text(
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
