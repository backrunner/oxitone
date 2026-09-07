//! Paths are built on the GPUI thread from bounded native analysis rings.
use crate::ui::{alpha, label, Preview, ACCENT, BORDER, GOLD, MUTED};
use gpui::{prelude::*, *};

pub fn view(this: &Preview) -> impl IntoElement {
    let node = this.analysis.get(&this.selected_scope);
    let wave = node.map(|n| n.wave.clone()).unwrap_or_default();
    let bins = crate::analysis::spectrum(&wave);
    let phase = wave.clone();
    let waveform = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            for (channel, tint) in [(0, ACCENT), (1, 0x658fc2)] {
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
                GOLD,
            );
        },
    )
    .size_full();
    let xy = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
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
                ACCENT,
            );
        },
    )
    .size_full();
    div()
        .h(px(144.))
        .flex_shrink_0()
        .border_t_1()
        .border_color(rgb(BORDER))
        .flex()
        .child(panel(
            format!("WAVEFORM · {}", this.selected_scope),
            waveform,
        ))
        .child(panel("SPECTRUM · Hann 512 · −90 … 0 dB".into(), spectrum))
        .child(panel("STEREO · Mid ↑ / Side →".into(), xy))
}

fn panel(title: String, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(rgb(BORDER))
        .child(label(title))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .mt_2()
                .bg(rgb(0x0a1017))
                .border_1()
                .border_color(alpha(MUTED, 0.15))
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
