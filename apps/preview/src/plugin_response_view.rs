use crate::{
    plugin_visuals::{caption, grid, trace},
    theme::Theme,
    ui::alpha,
};
use gpui::{prelude::*, *};
use oxitone_dsp::biquad::{design, BiquadCoeffs, BiquadKind};

pub fn response_db(c: BiquadCoeffs, frequency: f64, sample_rate: f64) -> f64 {
    let w = std::f64::consts::TAU * frequency / sample_rate;
    let numerator = (c.b0 + c.b1 * w.cos() + c.b2 * (2. * w).cos()).powi(2)
        + (c.b1 * w.sin() + c.b2 * (2. * w).sin()).powi(2);
    let denominator = (1. + c.a1 * w.cos() + c.a2 * (2. * w).cos()).powi(2)
        + (c.a1 * w.sin() + c.a2 * (2. * w).sin()).powi(2);
    10. * (numerator / denominator.max(1e-30)).max(1e-12).log10()
}
pub fn filter(values: &[f64], theme: Theme, sample_rate: f64) -> Div {
    let mode = values[0] as usize;
    let kind = [
        BiquadKind::Lowpass,
        BiquadKind::Highpass,
        BiquadKind::Bandpass,
    ][mode.min(2)];
    let cutoff = values[1].clamp(10., sample_rate / 2. - 1.);
    let coeffs = design(kind, sample_rate, cutoff, 0.5 + 9.5 * values[2], 0.);
    let maximum = 20_000f64.min(sample_rate * 0.49);
    let names = ["LOW PASS · 12 dB", "HIGH PASS · 12 dB", "BAND PASS"];
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .mb_2()
        .overflow_hidden()
        .child(caption(
            names[mode.min(2)].into(),
            format!("{cutoff:.0} Hz"),
            theme,
        ))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    let at = Bounds::new(
                        at.origin + point(px(12.), px(0.)),
                        size((at.size.width - px(24.)).max(px(1.)), at.size.height),
                    );
                    grid(at, theme, window);
                    let points: Vec<_> = (0..=192)
                        .map(|i| {
                            let x = i as f64 / 192.;
                            let hz = 20. * (maximum / 20.).powf(x);
                            (
                                x as f32,
                                ((24. - response_db(coeffs, hz, sample_rate)) / 72.).clamp(0., 1.)
                                    as f32,
                            )
                        })
                        .collect();
                    crate::plugin_graph_paint::area(
                        at,
                        &points,
                        1. / 3.,
                        alpha(theme.accent, 0.13).into(),
                        window,
                    );
                    trace(at, points.into_iter(), rgb(theme.accent).into(), 2., window);
                },
            )
            .w_full()
            .h(px(152.)),
        )
        .child(caption(
            "20 Hz    ·    1 kHz    ·    20 kHz".into(),
            "+24 / −48 dB".into(),
            theme,
        ))
}
pub fn lfo(values: &[f64], theme: Theme) -> Div {
    let (shape, rate, phase) = (values[0] as usize, values[1], values[2]);
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .mb_2()
        .overflow_hidden()
        .child(caption(
            "NOTE TRIGGER".into(),
            format!("{rate:.2} Hz"),
            theme,
        ))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    grid(at, theme, window);
                    let points: Vec<_> = (0..=192)
                        .map(|i| {
                            let x = i as f64 / 192.;
                            (
                                x as f32,
                                0.5 - oxitone_instruments::wavetable::lfo_value(shape, x + phase)
                                    as f32
                                    * 0.42,
                            )
                        })
                        .collect();
                    crate::plugin_graph_paint::area(
                        at,
                        &points,
                        0.5,
                        alpha(theme.tracks[2], 0.14).into(),
                        window,
                    );
                    trace(
                        at,
                        points.into_iter(),
                        rgb(theme.tracks[2]).into(),
                        2.,
                        window,
                    );
                },
            )
            .w_full()
            .h(px(124.)),
        )
        .child(caption(
            "−1    /    0    /    +1".into(),
            format!("Cycle {:.3} s", 1. / rate.max(0.01)),
            theme,
        ))
}

pub fn interactive_filter(
    element: Div,
    mode: &str,
    cutoff: &str,
    resonance: &str,
    this: &crate::plugin_window::PluginWindow,
    cx: &mut Context<crate::plugin_window::PluginWindow>,
) -> Div {
    use crate::{
        plugin_graph_handle::{Axis, Handle},
        plugin_parameter_drag::Range,
    };
    let details = this.details.as_ref().unwrap();
    let p = |id: &str| {
        details
            .parameters
            .iter()
            .find(|p| !p.host && p.spec.id == id)
            .unwrap()
    };
    let range = Range::frequency(20_000f64.min(this.project.snapshot.sample_rate as f64 * 0.49));
    let sample_rate = this.project.snapshot.sample_rate as f64;
    let hz = p(cutoff).value.clamp(10., sample_rate / 2. - 1.);
    let kind = [
        BiquadKind::Lowpass,
        BiquadKind::Highpass,
        BiquadKind::Bandpass,
    ][(p(mode).value as usize).min(2)];
    let gain = response_db(
        design(kind, sample_rate, hz, 0.5 + 9.5 * p(resonance).value, 0.),
        hz,
        sample_rate,
    );
    let handle = Handle {
        auxiliary: None,
        label: "F".into(),
        position: (
            range.fraction(p(cutoff).value) as f32,
            ((24. - gain) / 72.) as f32,
        ),
        color: 0,
        x: Some(Axis::new(cutoff, range)),
        y: Some(Axis::new(resonance, Range::parameter(&p(resonance).spec))),
    };
    element.relative().child(
        div()
            .absolute()
            .left(px(12.))
            .right(px(12.))
            .top(px(24.))
            .h(px(152.))
            .child(crate::plugin_graph_handle::overlay(&[handle], this, cx)),
    )
}

#[cfg(test)]
mod tests {
    use super::{design, response_db, BiquadKind};
    #[test]
    fn source_response_agrees_with_butterworth_cutoff_and_stopbands() {
        let rate = 48000.;
        let low = design(
            BiquadKind::Lowpass,
            rate,
            1000.,
            std::f64::consts::FRAC_1_SQRT_2,
            0.,
        );
        let high = design(
            BiquadKind::Highpass,
            rate,
            1000.,
            std::f64::consts::FRAC_1_SQRT_2,
            0.,
        );
        for c in [low, high] {
            assert!((response_db(c, 1000., rate) + 3.0103).abs() < 0.001);
        }
        assert!(response_db(low, 20., rate).abs() < 0.01);
        assert!(response_db(high, 20., rate) < -60.);
        assert!(response_db(low, 10000., rate) < -40.);
        assert!(response_db(high, 10000., rate).abs() < 0.01);
    }
}
