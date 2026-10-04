use crate::{
    plugin_visuals::{caption, grid, trace},
    theme::Theme,
    ui::alpha,
};
use gpui::{prelude::*, *};
use oxitone_instruments::wavetable::{preview_cycle, preview_sub_cycle};

pub fn oscillator(values: &[f64], advanced: [f64; 4], theme: Theme, stacked: bool) -> Div {
    let (source, target, position, phase) =
        (values[0] as usize, values[1] as usize, values[2], values[3]);
    let names = ["SINE", "SAW", "SQUARE", "TRIANGLE", "ORGAN", "GLASS"];
    let [bank, warp_mode, warp, _octave] = advanced;
    let title = if bank == 0. {
        format!("{} → {}", names[source.min(5)], names[target.min(5)])
    } else {
        ["PAIR", "ANALOG", "DIGITAL", "VOWEL"][(bank as usize).min(3)].into()
    };
    let cycle = |blend| {
        preview_cycle(
            bank as usize,
            source,
            target,
            blend,
            phase,
            warp_mode as usize,
            warp as f32,
        )
    };
    let selected = cycle(position as f32);
    let layers: Vec<_> = if stacked {
        (0..12).map(|i| cycle(i as f32 / 11.)).collect()
    } else {
        Vec::new()
    };
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .border_1()
        .border_color(alpha(theme.border, 0.5))
        .mb_2()
        .overflow_hidden()
        .child(caption(
            title,
            format!("WT {:02.0}%", position * 100.),
            theme,
        ))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    grid(at, theme, window);
                    if stacked {
                        for layer in (0..12).rev() {
                            trace(
                                at,
                                (0..=128).map(|i| {
                                    let x = i as f32 / 128.;
                                    (
                                        0.04 + x * 0.72 + layer as f32 * 0.018,
                                        0.66 - layers[layer][i * 2] * 0.22 - layer as f32 * 0.033,
                                    )
                                }),
                                alpha(theme.accent, 0.10 + layer as f32 * 0.022).into(),
                                1.,
                                window,
                            );
                        }
                    }
                    let points: Vec<_> = (0..=256)
                        .map(|i| {
                            let x = i as f32 / 256.;
                            (
                                if stacked {
                                    0.04 + x * 0.72 + position as f32 * 0.198
                                } else {
                                    0.025 + x * 0.95
                                },
                                if stacked {
                                    0.66 - selected[i] * 0.22 - position as f32 * 0.363
                                } else {
                                    0.5 - selected[i] * 0.4
                                },
                            )
                        })
                        .collect();
                    let baseline = if stacked {
                        0.66 - position as f32 * 0.363
                    } else {
                        0.5
                    };
                    crate::plugin_graph_paint::area(
                        at,
                        &points,
                        baseline,
                        alpha(theme.accent, 0.16).into(),
                        window,
                    );
                    trace(
                        at,
                        points.iter().copied(),
                        alpha(theme.accent, 0.13).into(),
                        7.,
                        window,
                    );
                    trace(at, points.into_iter(), rgb(theme.accent).into(), 2., window);
                },
            )
            .w_full()
            .h(px(196.)),
        )
        .child(caption(
            if stacked {
                "12 TABLE SLICES".into()
            } else {
                "SINGLE CYCLE".into()
            },
            format!(
                "{} VOICE{} · PHASE {:.0}°",
                values[4] as usize,
                if values[4] == 1. { "" } else { "S" },
                phase * 360.
            ),
            theme,
        ))
}

pub fn sub(values: &[f64], theme: Theme) -> Div {
    let wave = (values[0] as usize).min(5);
    let level = values[2] as f32;
    let samples = preview_sub_cycle(wave);
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .mb_2()
        .overflow_hidden()
        .child(caption(
            ["SINE", "TRIANGLE", "SAW", "SQUARE", "PULSE 25%", "ROUNDED"][wave].into(),
            format!("{:+.0} OCT · {:.0}%", values[1], values[2] * 100.),
            theme,
        ))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    grid(at, theme, window);
                    let points: Vec<_> = samples
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (0.025 + i as f32 / 256. * 0.95, 0.5 - v * level * 0.4))
                        .collect();
                    crate::plugin_graph_paint::area(
                        at,
                        &points,
                        0.5,
                        alpha(theme.accent, 0.13).into(),
                        window,
                    );
                    trace(at, points.into_iter(), rgb(theme.accent).into(), 2., window);
                },
            )
            .w_full()
            .h(px(108.)),
        )
}
