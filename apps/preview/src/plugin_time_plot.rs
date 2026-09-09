//! Bounded echoes, decay envelopes and phase trajectories derived from source settings.
use crate::plugin_plot::{Input, Plot, Trace};
use std::f64::consts::TAU;

/// Explicit seconds take precedence in the engine. Do not offer an ineffective beats control.
pub fn active_parameter(details: &crate::plugin_details::PluginDetails, id: &str) -> bool {
    if details.info.descriptor.plugin_id != "oxitone.delay"
        || !matches!(id, "timeBeats" | "timeSeconds")
    {
        return true;
    }
    let seconds = details
        .parameters
        .iter()
        .any(|v| !v.host && v.spec.id == "timeSeconds" && v.explicit);
    id == if seconds { "timeSeconds" } else { "timeBeats" }
}

pub fn build(p: &Input<'_>) -> Vec<Plot> {
    match p.id() {
        "oxitone.delay" => vec![echoes(p)],
        "oxitone.reverb" => vec![decay(p)],
        _ => vec![motion(p)],
    }
}
fn echoes(p: &Input<'_>) -> Plot {
    let seconds = active_parameter(p.details, "timeSeconds");
    let interval = p.v(if seconds { "timeSeconds" } else { "timeBeats" });
    let unit = if seconds { "s" } else { "beats" };
    let mut plot = Plot::new(
        "Echo pattern",
        format!(
            "{interval:.3} {unit} spacing · feedback {:.0}%",
            p.v("feedback") * 100.
        ),
    );
    plot.x = (0..=4)
        .map(|i| {
            (
                i as f32 / 4.,
                format!("{:.2} {unit}", interval * i as f64 * 2.),
            )
        })
        .collect();
    plot.y = vec![(0., "0 dB".into()), (0.5, "−30".into()), (1., "−60".into())];
    for i in 0..8 {
        let level = if i == 0 {
            0.
        } else {
            20. * p.v("feedback").max(1e-12).log10() * i as f64
        };
        if level < -60. {
            continue;
        }
        let x = (i + 1) as f32 / 8.;
        let ch = if p.v("pingPong") >= 0.5 { i % 2 } else { 0 };
        plot.traces.push(Trace {
            label: if i < 2 && p.v("pingPong") >= 0.5 {
                ["L", "R"][ch].into()
            } else {
                String::new()
            },
            color: ch,
            points: vec![(x, 1.), (x, (-level / 60.) as f32)],
        });
    }
    plot
}
fn decay(p: &Input<'_>) -> Plot {
    let time = p.v("decaySeconds");
    let predelay = p.v("predelayMs") / 1000.;
    let end = time + predelay;
    let mut plot = Plot::new(
        "Room decay",
        format!("RT60 {time:.2} s · predelay {:.0} ms", p.v("predelayMs")),
    );
    plot.x = (0..=4)
        .map(|i| (i as f32 / 4., format!("{:.2} s", end * i as f64 / 4.)))
        .collect();
    plot.y = vec![(0., "0 dB".into()), (0.5, "−30".into()), (1., "−60".into())];
    plot.curve("Envelope", 0, |x| {
        if x * end < predelay {
            1.
        } else {
            (x * end - predelay) / time
        }
    });
    plot.detail.push_str(" · decay envelope schematic");
    plot
}
fn motion(p: &Input<'_>) -> Plot {
    let phaser = p.id() == "oxitone.phaser";
    let rate = p.v("rateHz");
    let duration = 1. / rate;
    let (base, depth, stereo, max) = match p.id() {
        "oxitone.chorus" => (p.v("delayMs"), p.v("depth") * 8., 0.25, 46.),
        "oxitone.flanger" => (
            p.v("delayMs"),
            p.v("depthMs") * 0.5,
            p.v("stereo") * 0.5,
            20.,
        ),
        _ => (p.v("centerHz"), p.v("depth") * 2., 0.25, p.hz_max()),
    };
    let mut plot = Plot::new(
        if phaser {
            "Allpass sweep"
        } else {
            "Stereo delay sweep"
        },
        format!("{rate:.2} Hz · one modulation cycle"),
    );
    plot.x = (0..=4)
        .map(|i| (i as f32 / 4., format!("{:.2} s", duration * i as f64 / 4.)))
        .collect();
    plot.y = if phaser {
        vec![
            (0., "20k Hz".into()),
            (0.5, "632".into()),
            (1., "20".into()),
        ]
    } else {
        vec![
            (0., format!("{max:.0} ms")),
            (0.5, format!("{:.0}", max / 2.)),
            (1., "0".into()),
        ]
    };
    for ch in 0..2 {
        plot.curve(["L", "R"][ch], ch, |x| {
            let wave = (TAU * (x + ch as f64 * stereo)).sin();
            if phaser {
                1. - ((base * 2f64.powf(depth * wave) / 20.).ln() / (max / 20.).ln())
            } else {
                1. - (base + depth * (1. + wave)) / max
            }
        });
    }
    plot
}
