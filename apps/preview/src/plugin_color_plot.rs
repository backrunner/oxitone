//! Transfer functions and explicitly labelled reference signals for creative processing.
use crate::plugin_plot::{Input, Plot, Trace};
use std::f64::consts::{FRAC_PI_2, TAU};

pub fn build(p: &Input<'_>) -> Vec<Plot> {
    match p.id() {
        "oxitone.utility" | "oxitone.spreader" => vec![stereo(p)],
        "oxitone.pitch-shifter" | "oxitone.frequency-shifter" => vec![shift(p)],
        "oxitone.bitcrush" => vec![crush(p)],
        "oxitone.tape" => vec![transfer(p), tape_motion(p)],
        _ => vec![transfer(p)],
    }
}
pub fn shape(p: &Input<'_>, sample: f64) -> f64 {
    let x = sample * 10f64.powf(p.v("driveDb") / 20.);
    let cubic = |x: f64| {
        if x.abs() <= 1. {
            x - x.powi(3) / 3.
        } else {
            x.signum() * 2. / 3.
        }
    };
    let y = match p.id() {
        "oxitone.clipper" => {
            if p.v("mode") < 0.5 {
                x.clamp(-1., 1.)
            } else {
                cubic(x)
            }
        }
        "oxitone.saturator" if p.v("curve") >= 0.5 => {
            if x.abs() <= 1.5 {
                x - x.powi(3) * 4. / 27.
            } else {
                x.signum()
            }
        }
        "oxitone.distortion" => match p.v("mode") as u8 {
            1 => x.clamp(-1., 1.),
            2 => (x * FRAC_PI_2).sin(),
            3 => (x + p.v("bias")).tanh() - p.v("bias").tanh(),
            _ => x.tanh(),
        },
        _ => x.tanh(),
    };
    y * 10f64.powf(p.v("outputDb") / 20.)
}
fn transfer(p: &Input<'_>) -> Plot {
    let mut plot = Plot::new(
        "Waveshaping",
        "Static input / output · before bandwidth filtering",
    );
    plot.x = vec![(0., "−1".into()), (0.5, "0".into()), (1., "+1".into())];
    plot.y = vec![
        (0., "+2".into()),
        (0.25, "+1".into()),
        (0.5, "0".into()),
        (0.75, "−1".into()),
        (1., "−2".into()),
    ];
    plot.curve("", 2, |x| 0.5 - (x * 2. - 1.) / 4.);
    plot.curve("Output", 0, |x| 0.5 - shape(p, x * 2. - 1.) / 4.);
    plot
}
fn tape_motion(p: &Input<'_>) -> Plot {
    let mut plot = Plot::new("Tape motion", "Wow / flutter · delay modulation");
    plot.x = vec![(0., "0".into()), (0.5, "1 s".into()), (1., "2 s".into())];
    plot.y = vec![
        (0., "+1.32".into()),
        (0.5, "0 ms".into()),
        (1., "−1.32".into()),
    ];
    plot.curve("Motion", 1, |x| {
        0.5 - 0.5
            * (p.v("wow") * ((TAU * x * 2. * 0.37).sin() + 0.2 * (TAU * x * 2. * 0.37 * 3.).sin())
                + 0.12 * p.v("flutter") * (TAU * x * 2. * 6.71).sin())
            / 1.32
    });
    plot
}
fn crush(p: &Input<'_>) -> Plot {
    let mut plot = Plot::new(
        "Digital texture",
        "1 kHz reference sine · sample hold / quantization",
    );
    plot.x = vec![(0., "0".into()), (0.5, "2 ms".into()), (1., "4 ms".into())];
    plot.y = vec![(0., "+1".into()), (0.5, "0".into()), (1., "−1".into())];
    plot.curve("Input", 2, |x| 0.5 - 0.4 * (TAU * x * 4.).sin());
    let rate = p.v("rateHz").min(p.sample_rate);
    let steps = 2f64.powf(p.v("bits") - 1.);
    let mut points = vec![];
    let mut t = 0.;
    let mut random = 0x12345678u32;
    while t <= 0.004 {
        let value = ((TAU * t * 1000.).sin() * 0.8 * steps).round() / steps
            * 10f64.powf(p.v("outputDb") / 20.);
        let y = (0.5 - value * 0.5).clamp(0., 1.) as f32;
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        let jitter = random as f64 / u32::MAX as f64 - 0.5;
        let next = t + ((1. + jitter * p.v("jitter")) / rate).max(1. / p.sample_rate);
        points.push(((t / 0.004) as f32, y));
        points.push(((next.min(0.004) / 0.004) as f32, y));
        t = next;
    }
    plot.traces.push(Trace {
        label: "Output".into(),
        color: 0,
        points,
    });
    plot
}
fn shift(p: &Input<'_>) -> Plot {
    let pitch = p.id() == "oxitone.pitch-shifter";
    let mut plot = if pitch {
        Plot::new(
            "Pitch mapping",
            format!(
                "{:+.2} semitones · duration preserved",
                p.v("semitones") + p.v("cents") / 100.
            ),
        )
    } else {
        Plot::new(
            "Frequency translation",
            "Signed output frequency / input frequency",
        )
    };
    if pitch {
        plot.x = [0, 36, 60, 84, 127]
            .map(|n| (n as f32 / 127., crate::piano_layout::note_name(n)))
            .to_vec();
        plot.y = [0, 36, 60, 84, 127]
            .map(|n| (1. - n as f32 / 127., crate::piano_layout::note_name(n)))
            .to_vec();
        plot.curve("", 2, |x| 1. - x);
        plot.curve("Output", 0, |x| {
            1. - (x * 127. + p.v("semitones") + p.v("cents") / 100.) / 127.
        });
    } else {
        plot.x = vec![(0., "0".into()), (0.5, "1k Hz".into()), (1., "2k".into())];
        let low = (p.v("shiftHz") - p.v("stereoHz").abs() * 0.5).min(0.);
        let high = (2000. + p.v("shiftHz") + p.v("stereoHz").abs() * 0.5).max(2000.);
        plot.y = (0..=4)
            .map(|i| {
                let y = i as f64 / 4.;
                (
                    y as f32,
                    format!("{:.1}k", (high - y * (high - low)) / 1000.),
                )
            })
            .collect();
        plot.curve("", 2, |x| (high - x * 2000.) / (high - low));
        for ch in 0..2 {
            plot.curve(["L", "R"][ch], ch, |x| {
                (high
                    - (x * 2000. + p.v("shiftHz") + (ch as f64 * 2. - 1.) * p.v("stereoHz") * 0.5))
                    / (high - low)
            });
        }
    }
    plot
}
fn stereo(p: &Input<'_>) -> Plot {
    let utility = p.id() == "oxitone.utility";
    let mut width = p.v("width");
    if utility && p.v("mono") >= 0.5 {
        width = 0.;
    }
    let gain = if utility {
        10f64.powf(p.v("gainDb") / 20.)
    } else {
        1.
    };
    let mut plot = Plot::new(
        "Stereo field",
        if utility {
            "M/S reference circle · width / gain".into()
        } else {
            format!(
                "High-band M/S reference · bass mono below {:.0} Hz",
                p.v("bassMonoHz")
            )
        },
    );
    plot.x = vec![
        (0., "−Side".into()),
        (0.5, "0".into()),
        (1., "+Side".into()),
    ];
    plot.y = vec![(0., "+Mid".into()), (0.5, "0".into()), (1., "−Mid".into())];
    for (label, scale, color) in [("Input", 1., 2), ("Output", width, 0)] {
        plot.traces.push(Trace {
            label: label.into(),
            color,
            points: (0..=192)
                .map(|i| {
                    let angle = i as f64 / 192. * TAU;
                    let g = if color == 0 { gain } else { 1. };
                    (
                        (0.5 + 0.2 * angle.sin() * scale * g).clamp(0., 1.) as f32,
                        (0.5 - 0.2 * angle.cos() * g).clamp(0., 1.) as f32,
                    )
                })
                .collect(),
        });
    }
    plot
}
