//! Frequency diagrams use the same EQ coefficients and filter parameter semantics as DSP.
use crate::{
    plugin_plot::{Input, Plot, Trace},
    plugin_response_view::response_db,
};
use oxitone_dsp::biquad::{design, BiquadKind};

pub fn build(p: &Input<'_>) -> Vec<Plot> {
    if p.id() == "oxitone.resonator" {
        return vec![modes(p)];
    }
    let title = match p.id() {
        "oxitone.eq" => "Equalizer",
        "oxitone.convolver" => "IR bandwidth",
        _ => "Filter response",
    };
    let detail = match p.id() {
        "oxitone.nonlinear-filter" => "Small-signal response · drive changes saturation",
        "oxitone.convolver" => "Wet filter response · impulse shapes the room",
        _ => "Frequency / gain",
    };
    let mut plot = Plot::new(title, detail);
    p.frequency_axis(&mut plot);
    plot.y = [
        (0., "+24"),
        (1. / 3., "0 dB"),
        (2. / 3., "−24"),
        (1., "−48"),
    ]
    .map(|(v, s)| (v, s.into()))
    .to_vec();
    if p.id() == "oxitone.eq" {
        let y = |db: f64| (24. - db) / 48.;
        plot.y = [
            (0., "+24"),
            (0.25, "+12"),
            (0.5, "0 dB"),
            (0.75, "−12"),
            (1., "−24"),
        ]
        .map(|(v, s)| (v, s.into()))
        .to_vec();
        let kinds = [
            BiquadKind::LowShelf,
            BiquadKind::Peak,
            BiquadKind::Peak,
            BiquadKind::HighShelf,
        ];
        let bands: Vec<_> = kinds
            .into_iter()
            .enumerate()
            .map(|(i, kind)| {
                let band = i + 1;
                let q = if band == 2 || band == 3 {
                    p.v(&format!("band{band}.q"))
                } else {
                    1.
                };
                design(
                    kind,
                    p.sample_rate,
                    p.v(&format!("band{band}.freqHz")),
                    q,
                    p.v(&format!("band{band}.gainDb")),
                )
            })
            .collect();
        for (i, coefficients) in bands.iter().enumerate() {
            plot.curve("", 2, |x| {
                y(response_db(*coefficients, p.hz(x), p.sample_rate))
            });
            let hz = p.v(&format!("band{}.freqHz", i + 1));
            let x = ((hz / 20.).ln() / (p.hz_max() / 20.).ln()) as f32;
            plot.regions.push(crate::plugin_plot::Region {
                label: String::new(),
                rect: [
                    x - 0.004,
                    (y(p.v(&format!("band{}.gainDb", i + 1))) - 0.02) as f32,
                    0.008,
                    0.04,
                ],
                color: 0,
            });
        }
        plot.curve("Sum", 0, |x| {
            y(bands
                .iter()
                .map(|c| response_db(*c, p.hz(x), p.sample_rate))
                .sum())
        });
    } else {
        plot.curve("Wet", 0, |x| y(filter_db(p, p.hz(x))));
    }
    vec![plot]
}
fn y(db: f64) -> f64 {
    (24. - db) / 72.
}

pub fn filter_db(p: &Input<'_>, hz: f64) -> f64 {
    match p.id() {
        "oxitone.convolver" => {
            (if p.v("lowpassHz") >= 19999.99 {
                0.
            } else {
                one_pole_db(
                    hz,
                    p.v("lowpassHz").min(p.sample_rate * 0.45),
                    p.sample_rate,
                    false,
                )
            }) + if p.v("highpassHz") <= 20.0001 {
                0.
            } else {
                one_pole_db(hz, p.v("highpassHz"), p.sample_rate, true)
            } + p.v("outputDb")
        }
        "oxitone.nonlinear-filter" => {
            let rate = p.sample_rate * 2.;
            let t = (std::f64::consts::PI * hz / rate).tan()
                / (std::f64::consts::PI * p.v("cutoffHz").min(rate * 0.225) / rate).tan();
            let k = 2. - 1.94 * p.v("resonance");
            let numerator = match p.v("mode") as u8 {
                1 => t * t,
                2 => t,
                3 => (1. - t * t).abs(),
                _ => 1.,
            };
            let magnitude = numerator / ((1. - t * t).powi(2) + (k * t).powi(2)).sqrt().max(1e-20);
            20. * magnitude.max(1e-12).log10() + p.v("driveDb") + p.v("outputDb")
        }
        _ => {
            let kind = [
                BiquadKind::Lowpass,
                BiquadKind::Highpass,
                BiquadKind::Bandpass,
            ][(p.v("mode") as usize).min(2)];
            response_db(
                design(kind, p.sample_rate, p.v("cutoffHz"), p.v("resonance"), 0.),
                hz,
                p.sample_rate,
            )
        }
    }
}
pub fn one_pole_db(hz: f64, cutoff: f64, rate: f64, highpass: bool) -> f64 {
    let r = (-std::f64::consts::TAU * cutoff / rate).exp();
    let cos = (std::f64::consts::TAU * hz / rate).cos();
    let numerator = if highpass {
        r * r * (2. - 2. * cos)
    } else {
        (1. - r).powi(2)
    };
    10. * (numerator / (1. + r * r - 2. * r * cos).max(1e-20))
        .max(1e-12)
        .log10()
}
fn modes(p: &Input<'_>) -> Plot {
    let decay = p.v("decaySeconds");
    let mut plot = Plot::new(
        "Resonant modes",
        format!("Four tuned modes · {decay:.2} s fundamental decay"),
    );
    p.frequency_axis(&mut plot);
    plot.y = vec![(0., format!("{decay:.2} s")), (1., "0".into())];
    for ch in 0..2 {
        for m in 0..4 {
            let hz = p.v("frequencyHz")
                * (m + 1) as f64
                * (1. + p.v("inharmonicity") * m as f64 * 0.173)
                * 2f64.powf((ch as f64 * 2. - 1.) * p.v("spread") * m as f64 / 1200.);
            if hz >= p.sample_rate * 0.48 {
                continue;
            }
            let x = ((hz / 20.).ln() / (p.hz_max() / 20.).ln()) as f32;
            let height = 1. / (1. + m as f64 * (1. - p.v("brightness")));
            plot.traces.push(Trace {
                label: if m == 0 {
                    ["L", "R"][ch].into()
                } else {
                    String::new()
                },
                color: ch,
                points: vec![(x, 1.), (x, 1. - height as f32)],
            });
        }
    }
    plot
}
