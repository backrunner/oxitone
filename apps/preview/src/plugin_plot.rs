//! Cached, source-derived diagrams. Never samples a plugin instance or opens an audio device.
use crate::{model::ViewProject, plugin_details::PluginDetails};

#[derive(Clone, Debug)]
pub struct Trace {
    pub label: String,
    pub points: Vec<(f32, f32)>,
    pub color: usize,
}
#[derive(Clone, Debug)]
pub struct Region {
    pub label: String,
    pub rect: [f32; 4],
    pub color: usize,
}
#[derive(Clone, Debug)]
pub struct Plot {
    pub title: String,
    pub detail: String,
    pub x: Vec<(f32, String)>,
    pub y: Vec<(f32, String)>,
    pub traces: Vec<Trace>,
    pub regions: Vec<Region>,
}
impl Plot {
    pub fn new(title: &str, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            x: vec![],
            y: vec![],
            traces: vec![],
            regions: vec![],
        }
    }
    pub fn curve(&mut self, label: &str, color: usize, f: impl Fn(f64) -> f64) {
        self.traces.push(Trace {
            label: label.into(),
            color,
            points: (0..=256)
                .map(|i| {
                    let x = i as f64 / 256.;
                    (x as f32, f(x).clamp(0., 1.) as f32)
                })
                .collect(),
        });
    }
}
pub struct Input<'a> {
    pub details: &'a PluginDetails,
    pub sample_rate: f64,
}
impl Input<'_> {
    pub fn id(&self) -> &str {
        &self.details.info.descriptor.plugin_id
    }
    pub fn v(&self, id: &str) -> f64 {
        self.details
            .parameters
            .iter()
            .find(|p| !p.host && p.spec.id == id)
            .map_or(0., |p| p.value)
    }
    pub fn hz_max(&self) -> f64 {
        20_000_f64.min(self.sample_rate * 0.49)
    }
    pub fn hz(&self, x: f64) -> f64 {
        20. * (self.hz_max() / 20.).powf(x)
    }
    pub fn frequency_axis(&self, plot: &mut Plot) {
        plot.x = [20., 100., 1000., 10_000., self.hz_max()]
            .into_iter()
            .filter(|hz| *hz <= self.hz_max())
            .map(|hz| {
                (
                    ((hz / 20.).ln() / (self.hz_max() / 20.).ln()) as f32,
                    if hz >= 1000. {
                        format!("{:.0}k", hz / 1000.)
                    } else {
                        format!("{hz:.0}")
                    },
                )
            })
            .collect();
    }
}

pub fn build(details: &PluginDetails, project: &ViewProject) -> Vec<Plot> {
    if details.info.library.is_some() || details.info.descriptor.plugin_version != "1.0.0" {
        return vec![];
    }
    let input = Input {
        details,
        sample_rate: project.snapshot.sample_rate as f64,
    };
    match input.id() {
        "oxitone.eq"
        | "oxitone.filter"
        | "oxitone.nonlinear-filter"
        | "oxitone.convolver"
        | "oxitone.resonator" => crate::plugin_filter_plot::build(&input),
        "oxitone.compressor"
        | "oxitone.gate"
        | "oxitone.limit"
        | "oxitone.limiter"
        | "oxitone.compactor"
        | "oxitone.multiband"
        | "oxitone.multiband-dynamics" => crate::plugin_dynamics_plot::build(&input),
        "oxitone.delay" | "oxitone.reverb" | "oxitone.chorus" | "oxitone.flanger"
        | "oxitone.phaser" => crate::plugin_time_plot::build(&input),
        "oxitone.clipper"
        | "oxitone.saturator"
        | "oxitone.distortion"
        | "oxitone.tape"
        | "oxitone.bitcrush"
        | "oxitone.pitch-shifter"
        | "oxitone.frequency-shifter"
        | "oxitone.spreader"
        | "oxitone.utility" => crate::plugin_color_plot::build(&input),
        "oxitone.sampler" | "oxitone.multisampler" | "oxitone.slicer" => {
            crate::plugin_sample_plot::build(&input, project)
        }
        _ => vec![],
    }
}
