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
        (0..6).map(|i| cycle(i as f32 / 5.)).collect()
    } else {
        Vec::new()
    };
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
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
                        for layer in (0..6).rev() {
                            trace(
                                at,
                                (0..=128).map(|i| {
                                    let x = i as f32 / 128.;
                                    (
                                        0.025 + x * 0.82 + layer as f32 * 0.025,
                                        0.55 - layers[layer][i * 2] * 0.24 - layer as f32 * 0.055,
                                    )
                                }),
                                alpha(theme.secondary, 0.22 + layer as f32 * 0.05).into(),
                                1.,
                                window,
                            );
                        }
                    }
                    trace(
                        at,
                        (0..=256).map(|i| {
                            let x = i as f32 / 256.;
                            (
                                if stacked {
                                    0.025 + x * 0.82 + position as f32 * 0.125
                                } else {
                                    0.025 + x * 0.95
                                },
                                if stacked {
                                    0.55 - selected[i] * 0.24 - position as f32 * 0.275
                                } else {
                                    0.5 - selected[i] * 0.4
                                },
                            )
                        }),
                        rgb(theme.accent).into(),
                        2.,
                        window,
                    );
                },
            )
            .w_full()
            .h(px(148.)),
        )
}

pub fn sub(values: &[f64], theme: Theme) -> Div {
    let wave = (values[0] as usize).min(5);
    let samples = preview_sub_cycle(wave);
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .mb_2()
        .overflow_hidden()
        .child(caption(
            ["SINE", "TRIANGLE", "SAW", "SQUARE", "PULSE 25%", "ROUNDED"][wave].into(),
            String::new(),
            theme,
        ))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    grid(at, theme, window);
                    trace(
                        at,
                        samples
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (0.025 + i as f32 / 256. * 0.95, 0.5 - v * 0.3)),
                        rgb(theme.accent).into(),
                        2.,
                        window,
                    );
                },
            )
            .w_full()
            .h(px(72.)),
        )
}
