//! Static detector curves and timing diagrams, not measured gain-reduction meters.
use crate::plugin_plot::{Input, Plot, Region};

pub fn build(p: &Input<'_>) -> Vec<Plot> {
    let bands = matches!(p.id(), "oxitone.multiband" | "oxitone.multiband-dynamics");
    let mut plot = Plot::new(
        if bands {
            "Band dynamics"
        } else if matches!(p.id(), "oxitone.limit" | "oxitone.limiter") {
            "Ceiling target"
        } else {
            "Transfer"
        },
        "Input / output · static detector target",
    );
    plot.x = [
        (-72., "−72"),
        (-48., "−48"),
        (-24., "−24"),
        (0., "0 dB"),
        (12., "+12"),
    ]
    .map(|(db, s)| (((db + 72.) / 84.) as f32, s.into()))
    .to_vec();
    plot.y = [
        (0., "+12"),
        (1. / 7., "0 dB"),
        (3. / 7., "−24"),
        (5. / 7., "−48"),
        (1., "−72"),
    ]
    .map(|(y, s)| (y, s.into()))
    .to_vec();
    plot.curve("", 2, |x| 1. - x);
    if bands {
        for (band, name) in ["Low", "Mid", "High"].into_iter().enumerate() {
            plot.curve(name, band, |x| {
                (12. - transfer(p, -72. + 84. * x, band)) / 84.
            });
        }
    } else {
        plot.curve("Output", 0, |x| {
            (12. - transfer(p, -72. + 84. * x, 0)) / 84.
        });
        if p.id() == "oxitone.gate" {
            let threshold = (p.v("thresholdDb") + 72.) / 84.;
            let close = (p.v("thresholdDb") - p.v("hysteresisDb") + 72.) / 84.;
            plot.regions.push(Region {
                label: String::new(),
                rect: [close as f32, 0., (threshold - close) as f32, 1.],
                color: 1,
            });
            plot.detail = "Shaded range retains the current gate state".into();
        }
    }
    vec![plot, if bands { crossovers(p) } else { timing(p) }]
}
pub fn transfer(p: &Input<'_>, input: f64, band: usize) -> f64 {
    match p.id() {
        "oxitone.compressor" => {
            let over = input - p.v("thresholdDb");
            let knee = p.v("kneeDb");
            let gain = if knee > 0. && over.abs() <= knee / 2. {
                (1. / p.v("ratio") - 1.) * (over + knee / 2.).powi(2) / (2. * knee)
            } else {
                (1. / p.v("ratio") - 1.) * over.max(0.)
            };
            input + gain + p.v("makeupDb")
        }
        "oxitone.compactor" => {
            input
                + (p.v("thresholdDb") - input).max(0.).min(p.v("upwardDb"))
                    * ((input + 84.) / 18.).clamp(0., 1.)
                + p.v("outputDb")
        }
        "oxitone.gate" => {
            input
                + if input < p.v("thresholdDb") {
                    p.v("rangeDb")
                } else {
                    0.
                }
        }
        "oxitone.limit" | "oxitone.limiter" => {
            // Static envelope target; excludes oversampling and reconstruction reserve.
            (input + p.v("inputDb")).min(p.v("ceilingDb"))
        }
        _ => {
            let production = p.id() == "oxitone.multiband-dynamics";
            let lower = if production {
                [-42., -40., -44.][band]
            } else {
                p.v("lowerThresholdDb")
            };
            let upper = if production {
                [-24., -22., -26.][band]
            } else {
                p.v("upperThresholdDb")
            };
            let floor = if production {
                (input + 84.) / 18.
            } else {
                (input + 72.) / 12.
            };
            let up = (lower - input).max(0.).min(p.v("upwardDb")) * floor.clamp(0., 1.);
            let down = -(input - upper).max(0.) * (1. - 1. / p.v("downwardRatio"));
            let trim = p.v(["lowGainDb", "midGainDb", "highGainDb"][band]);
            let trim_db = if production {
                trim * p.v("depth")
            } else {
                20. * (1. + (10f64.powf(trim / 20.) - 1.) * p.v("depth")).log10()
            };
            input + (up + down) * p.v("depth") + trim_db + p.v("outputDb")
        }
    }
}
fn timing(p: &Input<'_>) -> Plot {
    let attack = p.v("attackMs");
    let release = p.v("releaseMs").max(1.);
    let hold = p.v("holdMs");
    let end = (attack + hold + release * 4.).max(1.);
    let mut plot = Plot::new(
        "Envelope timing",
        if p.id() == "oxitone.compactor" {
            format!("Transient emphasis {:+.0}%", p.v("transient") * 100.)
        } else {
            "Unit-step timing schematic".into()
        },
    );
    plot.x = vec![
        (0., "0".into()),
        (0.5, format!("{:.0} ms", end / 2.)),
        (1., format!("{end:.0} ms")),
    ];
    plot.y = vec![(0., "1".into()), (1., "0".into())];
    plot.curve("Envelope", 0, |x| {
        let t = x * end;
        if t < attack {
            1. - t / attack.max(0.001)
        } else if t < attack + hold {
            0.
        } else if p.id() == "oxitone.gate" {
            ((t - attack - hold) / release).min(1.)
        } else {
            1. - (-(t - attack - hold) / release).exp()
        }
    });
    plot
}
fn crossovers(p: &Input<'_>) -> Plot {
    let mut plot = Plot::new("Crossover bands", "Frequency split / band trim");
    p.frequency_axis(&mut plot);
    plot.y = vec![(0., "+18 dB".into()), (0.5, "0".into()), (1., "−18".into())];
    let edge = |hz: f64| ((hz / 20.).ln() / (p.hz_max() / 20.).ln()).clamp(0., 1.) as f32;
    let edges = [0., edge(p.v("lowHz")), edge(p.v("highHz")), 1.];
    for (i, name) in ["Low", "Mid", "High"].into_iter().enumerate() {
        plot.regions.push(Region {
            label: name.into(),
            color: i,
            rect: [edges[i], 0., edges[i + 1] - edges[i], 1.],
        });
        let gain = p.v(["lowGainDb", "midGainDb", "highGainDb"][i]);
        plot.traces.push(crate::plugin_plot::Trace {
            label: String::new(),
            color: i,
            points: vec![
                (edges[i], ((18. - gain) / 36.) as f32),
                (edges[i + 1], ((18. - gain) / 36.) as f32),
            ],
        });
    }
    plot
}
