//! Paths are built on the GPUI thread from bounded native analysis rings.
use crate::{theme::Theme, ui::Preview};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, height: f32) -> impl IntoElement {
    let theme = this.theme;
    let name = this
        .project
        .as_ref()
        .and_then(|p| {
            crate::mixer_model::strips(p)
                .into_iter()
                .find(|s| s.id == this.selected_scope)
        })
        .map_or_else(|| "Master".into(), |s| s.name);
    let node = this.analysis.get(&this.selected_scope);
    let wave = node.map(|n| n.wave.clone()).unwrap_or_default();
    let bins = crate::analysis::spectrum(&wave);
    let phase = wave.clone();
    let waveform = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            grid(window, bounds, theme);
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            for (channel, tint) in [(0, theme.accent), (1, theme.secondary)] {
                paint(
                    window,
                    bounds,
                    wave.iter().enumerate().map(|(i, s)| {
                        (
                            i as f32 / (wave.len().max(2) - 1) as f32 * w,
                            h * 0.5 - s[channel].clamp(-1., 1.) * h * 0.45,
                        )
                    }),
                    tint,
                );
            }
        },
    )
    .size_full();
    let spectrum = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            grid(window, bounds, theme);
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            paint(
                window,
                bounds,
                bins.iter().enumerate().map(|(i, v)| {
                    (
                        ((i + 1) as f32).ln() / (256_f32.ln()) * w,
                        h - (v.clamp(-90., 0.) + 90.) / 90. * h,
                    )
                }),
                theme.gold,
            );
        },
    )
    .size_full();
    let xy = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            grid(window, bounds, theme);
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            paint(
                window,
                bounds,
                phase.iter().step_by(2).map(|s| {
                    (
                        w * 0.5 + (s[0] - s[1]).clamp(-2., 2.) * w * 0.23,
                        h * 0.5 - (s[0] + s[1]).clamp(-2., 2.) * h * 0.23,
                    )
                }),
                theme.accent,
            );
        },
    )
    .size_full();
    div()
        .h(px(height))
        .flex_shrink_0()
        .border_t_1()
        .border_color(rgb(theme.border))
        .flex()
        .child(panel(
            theme,
            format!(
                "WAVEFORM · {name}{}",
                if this.playback.playing {
                    ""
                } else {
                    " · Paused"
                }
            ),
            waveform,
        ))
        .child(panel(theme, "SPECTRUM · −90 … 0 dB".into(), spectrum))
        .child(panel(theme, "STEREO · Mid ↑ / Side →".into(), xy))
}

fn panel(theme: Theme, title: String, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(rgb(theme.border))
        .child(
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme.muted))
                .truncate()
                .child(title),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .mt_2()
                .bg(rgb(theme.scope))
                .border_1()
                .border_color(rgb(theme.border))
                .child(content),
        )
}

fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    points: impl Iterator<Item = (f32, f32)>,
    tint: u32,
) {
    let mut path = PathBuilder::stroke(px(1.));
    for (i, (x, y)) in points.enumerate() {
        let point = point(bounds.origin.x + px(x), bounds.origin.y + px(y));
        if i == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, rgb(tint));
    }
}

fn grid(window: &mut Window, bounds: Bounds<Pixels>, theme: Theme) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    for i in 1..8 {
        let x = w * i as f32 / 8.;
        window.paint_quad(fill(
            Bounds::new(bounds.origin + point(px(x), px(0.)), size(px(1.), px(h))),
            crate::ui::alpha(theme.border, if i == 4 { 0.8 } else { 0.3 }),
        ));
    }
    for i in 1..4 {
        let y = h * i as f32 / 4.;
        window.paint_quad(fill(
            Bounds::new(bounds.origin + point(px(0.), px(y)), size(px(w), px(1.))),
            crate::ui::alpha(theme.border, if i == 2 { 0.8 } else { 0.3 }),
        ));
    }
}
