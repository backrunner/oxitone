//! Direct controls for detector thresholds, ceilings and crossover/trim diagrams.
use crate::{
    plugin_graph_handle::{Axis, Handle},
    plugin_parameter_drag::Range,
    plugin_plot::{Input, Plot},
};

pub fn transfer(p: &Input<'_>, plot: &mut Plot) {
    let limiter = matches!(p.id(), "oxitone.limit" | "oxitone.limiter");
    let compressor = p.id() == "oxitone.compressor";
    let compactor = p.id() == "oxitone.compactor";
    if !(limiter || compressor || compactor || p.id() == "oxitone.gate") {
        return;
    }
    let input = if limiter { 12. } else { p.v("thresholdDb") };
    let output = crate::plugin_dynamics_plot::transfer(p, input, 0);
    let scale = Range::linear(-72., 12.);
    plot.handles.push(Handle {
        label: if limiter { "C" } else { "T" }.into(),
        position: (
            scale.fraction(input) as f32,
            1. - scale.fraction(output) as f32,
        ),
        color: 0,
        x: (!limiter).then(|| Axis::new("thresholdDb", scale)),
        y: if limiter {
            Some(Axis::new("ceilingDb", scale))
        } else if compressor {
            Some(Axis::new("makeupDb", scale))
        } else if compactor {
            Some(Axis::new("outputDb", scale))
        } else {
            None
        },
        auxiliary: if compressor {
            p.details
                .parameters
                .iter()
                .find(|v| v.spec.id == "kneeDb")
                .map(|v| Axis::new("kneeDb", Range::parameter(&v.spec)))
        } else {
            None
        },
    });
}

pub fn crossovers(p: &Input<'_>, plot: &mut Plot, edges: [f32; 4]) {
    for (i, id) in ["lowHz", "highHz"].into_iter().enumerate() {
        plot.handles.push(Handle {
            label: ["L", "H"][i].into(),
            position: (edges[i + 1], 0.2),
            color: i,
            x: Some(Axis::new(id, Range::frequency(p.hz_max()))),
            y: None,
            auxiliary: None,
        });
    }
    for (i, id) in ["lowGainDb", "midGainDb", "highGainDb"]
        .into_iter()
        .enumerate()
    {
        plot.handles.push(Handle {
            label: (i + 1).to_string(),
            position: (
                (edges[i] + edges[i + 1]) / 2.,
                ((18. - p.v(id)) / 36.) as f32,
            ),
            color: i,
            x: None,
            y: Some(Axis::new(id, Range::linear(-18., 18.))),
            auxiliary: None,
        });
    }
}
