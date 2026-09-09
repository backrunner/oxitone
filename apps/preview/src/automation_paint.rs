use crate::{
    automation_state::AutomationGesture, document_wire::AutomationPoint, theme::Theme, ui::alpha,
};
use gpui::*;
use oxitone_transport::{CompiledAutomation, EvalContext};

pub fn paint(
    area: Bounds<Pixels>,
    theme: Theme,
    beats: f64,
    offset: f64,
    source: Option<&CompiledAutomation>,
    handles: &[AutomationPoint],
    gesture: Option<&AutomationGesture>,
    window: &mut Window,
    _cx: &mut App,
) {
    let width = f32::from(area.size.width);
    let height = f32::from(area.size.height);
    if width <= 0. || height <= 0. {
        return;
    }
    window.with_content_mask(Some(ContentMask { bounds: area }), |window| {
        window.paint_quad(fill(area, rgb(theme.scope)));
        let step = (beats / (width / 70.).max(1.) as f64)
            .log2()
            .ceil()
            .exp2()
            .max(0.125);
        let first = (offset / step).floor() as usize;
        let last = ((offset + beats) / step).ceil() as usize;
        for i in first..=last {
            let x = ((i as f64 * step - offset) / beats) as f32 * width;
            window.paint_quad(fill(
                Bounds::new(
                    area.origin + point(px(x), px(0.)),
                    size(px(1.), area.size.height),
                ),
                alpha(
                    theme.border,
                    if (i as f64 * step).rem_euclid(4.) == 0. {
                        0.9
                    } else {
                        0.45
                    },
                ),
            ));
        }
        for i in 0..=8 {
            window.paint_quad(fill(
                Bounds::new(
                    area.origin + point(px(0.), px(height * i as f32 / 8.)),
                    size(area.size.width, px(1.)),
                ),
                alpha(theme.border, if i % 4 == 0 { 0.65 } else { 0.25 }),
            ));
        }
        if let Some(gesture) = gesture {
            let left = ((gesture.curve.start - offset) / beats) as f32 * width;
            let right = ((gesture.curve.end - offset) / beats) as f32 * width;
            window.paint_quad(fill(
                Bounds::new(
                    area.origin + point(px(left), px(0.)),
                    size(px(right - left), area.size.height),
                ),
                alpha(theme.gold, 0.07),
            ));
        }
        let mut line = PathBuilder::stroke(px(2.));
        let mut fill_path = PathBuilder::fill();
        fill_path.move_to(area.origin + point(px(0.), px(height)));
        let count = (width as usize).clamp(2, 2048);
        let mut any = false;
        for i in 0..=count {
            let x = i as f32 / count as f32 * width;
            let beat = offset + f64::from(x / width) * beats;
            let current = gesture
                .filter(|g| beat >= g.curve.start && beat < g.curve.end)
                .and_then(|g| g.preview.as_deref())
                .or(source);
            let Some(current) = current else {
                continue;
            };
            let value = current
                .value_at(beat, &EvalContext::default())
                .clamp(0., 1.);
            let at = area.origin + point(px(x), px(height * (1. - value as f32)));
            if !any {
                line.move_to(at);
            } else {
                line.line_to(at);
            }
            fill_path.line_to(at);
            any = true;
        }
        if any {
            fill_path.line_to(area.origin + point(px(width), px(height)));
            fill_path.close();
            if let Ok(path) = fill_path.build() {
                window.paint_path(path, alpha(theme.accent, 0.09));
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, rgb(theme.accent));
            }
        }
        let points = gesture.map_or(handles, |g| &g.curve.points);
        for p in points {
            let x = ((p.beat - offset) / beats) as f32 * width;
            let y = (1. - p.value) as f32 * height;
            if x < -6. || x > width + 6. {
                continue;
            }
            let at = area.origin + point(px(x - 4.), px(y - 4.));
            window.paint_quad(quad(
                Bounds::new(at, size(px(8.), px(8.))),
                px(4.),
                rgb(theme.panel),
                px(2.),
                rgb(if gesture.is_some() {
                    theme.gold
                } else {
                    theme.accent
                }),
                BorderStyle::default(),
            ));
        }
    });
}
