//! Instrument maps come from source regions/slices. No synthetic sample waveform is displayed.
use crate::{
    model::ViewProject,
    plugin_plot::{Input, Plot, Region, Trace},
};

pub fn build(p: &Input<'_>, _project: &ViewProject) -> Vec<Plot> {
    if p.id() == "oxitone.slicer" {
        return vec![slices(p)];
    }
    let mut mapping = Plot::new("Keyboard map", "Key / velocity");
    mapping.x = [0, 36, 60, 84, 127]
        .map(|n| (n as f32 / 128., crate::piano_layout::note_name(n)))
        .to_vec();
    mapping.y = vec![(0., "127".into()), (0.5, "64".into()), (1., "1".into())];
    if p.id() == "oxitone.multisampler" {
        if let Some(regions) = p
            .details
            .state
            .as_ref()
            .and_then(|s| s.get("regions"))
            .and_then(|s| s.as_array())
        {
            for (i, region) in regions.iter().enumerate() {
                let pair = |id: &str, index: usize| region[id][index].as_f64().unwrap_or(0.);
                let (lo, hi, soft, loud) = (
                    pair("keyRange", 0),
                    pair("keyRange", 1),
                    pair("velocityRange", 0),
                    pair("velocityRange", 1),
                );
                mapping.regions.push(Region {
                    label: region["resource"].as_str().unwrap_or("").into(),
                    color: i % 2,
                    rect: [
                        (lo / 128.) as f32,
                        (1. - loud / 127.) as f32,
                        ((hi - lo + 1.) / 128.) as f32,
                        ((loud - soft + 1.) / 127.) as f32,
                    ],
                });
            }
        }
    } else {
        let root = p.v("rootKey") as u8;
        mapping.regions.push(Region {
            label: format!("Root {}", crate::piano_layout::note_name(root)),
            rect: [0., 0., 1., 1.],
            color: 0,
        });
        mapping.traces.push(Trace {
            label: String::new(),
            color: 1,
            points: vec![(root as f32 / 128., 0.), (root as f32 / 128., 1.)],
        });
    }
    vec![mapping, envelope(p)]
}
fn envelope(p: &Input<'_>) -> Plot {
    let (a, d, s, r) = (
        p.v("amp.attack"),
        p.v("amp.decay"),
        p.v("amp.sustain"),
        p.v("amp.release"),
    );
    let hold = (a + d + r).max(0.1) * 0.3;
    let end = (a + d + hold + r).max(0.001);
    let mut plot = Plot::new("Amplitude envelope", "ADSR · illustrative sustain hold");
    plot.x = vec![(0., "0".into()), (1., format!("{end:.2} s"))];
    plot.y = vec![(0., "1".into()), (0.5, "0.5".into()), (1., "0".into())];
    plot.traces.push(Trace {
        label: String::new(),
        color: 0,
        points: [(0., 0.), (a, 1.), (a + d, s), (a + d + hold, s), (end, 0.)]
            .map(|(x, y)| ((x / end) as f32, (1. - y) as f32))
            .to_vec(),
    });
    plot
}
fn slices(p: &Input<'_>) -> Plot {
    let state = p.details.state.as_ref();
    let trigger = state.and_then(|s| s["triggerNote"].as_u64()).unwrap_or(36) as u8;
    let source = state.and_then(|s| s.get("slices"));
    let count = source.and_then(|s| {
        s.as_array()
            .map(Vec::len)
            .or_else(|| s["grid"].as_u64().map(|n| n as usize))
    });
    let Some(count) = count else {
        let sensitivity = source
            .and_then(|s| s["onset"]["sensitivity"].as_f64())
            .unwrap_or(0.5);
        let mut plot = Plot::new(
            "Onset detection",
            format!(
                "Automatic slices · first key {}",
                crate::piano_layout::note_name(trigger)
            ),
        );
        plot.x = vec![(0., "Less sensitive".into()), (1., "More sensitive".into())];
        plot.regions.push(Region {
            label: format!("Sensitivity {:.0}%", sensitivity * 100.),
            color: 0,
            rect: [0., 0.3, sensitivity as f32, 0.4],
        });
        return plot;
    };
    let mut plot = Plot::new(
        "Slice triggers",
        format!(
            "{count} slices · {}",
            state
                .and_then(|s| s["playMode"].as_str())
                .unwrap_or("oneshot")
        ),
    );
    plot.y = vec![(0., "2×".into()), (0.5, "1×".into()), (1., "0".into())];
    for i in 0..count.min(64) {
        let key = trigger.saturating_add(i as u8);
        let level = source
            .and_then(|s| s.as_array())
            .and_then(|s| s.get(i))
            .and_then(|s| s["level"].as_f64())
            .unwrap_or(1.)
            * p.v("level");
        plot.regions.push(Region {
            label: crate::piano_layout::note_name(key),
            color: i % 2,
            rect: [
                i as f32 / count as f32,
                (1. - level / 2.).max(0.) as f32,
                1. / count as f32,
                (level / 2.).min(1.) as f32,
            ],
        });
        if i % count.div_ceil(8) == 0 {
            plot.x
                .push((i as f32 / count as f32, crate::piano_layout::note_name(key)));
        }
    }
    plot
}
