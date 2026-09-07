use crate::{
    plugin_visuals::{caption, grid, trace},
    theme::Theme,
    ui::alpha,
};
use gpui::{prelude::*, *};
use oxitone_instruments::wavetable::cycle_value;

pub fn oscillator(values: &[f64], theme: Theme, stacked: bool) -> Div {
    let (source, target, position, phase) =
        (values[0] as usize, values[1] as usize, values[2], values[3]);
    let names = ["SINE", "SAW", "SQUARE", "TRIANGLE", "ORGAN", "GLASS"];
    let title = format!("{} → {}", names[source.min(5)], names[target.min(5)]);
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
                    let sample = |t: f64, blend: f64| {
                        let a = cycle_value(source, t + phase);
                        a + (cycle_value(target, t + phase) - a) * blend
                    };
                    if stacked {
                        for layer in (0..6).rev() {
                            let blend = layer as f64 / 5.;
                            trace(
                                at,
                                (0..=128).map(|i| {
                                    let x = i as f32 / 128.;
                                    (
                                        0.025 + x * 0.82 + layer as f32 * 0.025,
                                        0.55 - sample(x as f64, blend) as f32 * 0.24
                                            - layer as f32 * 0.055,
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
                        (0..=192).map(|i| {
                            let x = i as f32 / 192.;
                            (
                                0.025 + x * 0.95,
                                0.55 - sample(x as f64, position) as f32
                                    * if stacked { 0.3 } else { 0.4 },
                            )
                        }),
                        rgb(theme.accent).into(),
                        2.,
                        window,
                    );
                },
            )
            .w_full()
            .h(px(92.)),
        )
        .child(caption(
            format!("{} voices · {:.0} ct", values[4] as usize, values[5]),
            format!("Width {:.0}%", values[6] * 100.),
            theme,
        ))
}
