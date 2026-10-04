//! Parameter-space gesture math shared by dials and graph handles.
use crate::plugin_details::ParameterDetail;
use oxitone_core::wire::{ParameterMapping, ParameterSpec, ParameterUnit};

#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub logarithmic: bool,
}
impl Range {
    pub fn linear(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            logarithmic: false,
        }
    }
    pub fn frequency(max: f64) -> Self {
        Self {
            min: 20.,
            max,
            logarithmic: true,
        }
    }
    pub fn parameter(spec: &ParameterSpec) -> Self {
        Self {
            min: spec.min,
            max: spec.max,
            logarithmic: spec.mapping == Some(ParameterMapping::Log) && spec.min > 0.,
        }
    }
    pub fn fraction(self, value: f64) -> f64 {
        if self.logarithmic {
            (value.max(f64::MIN_POSITIVE) / self.min).ln() / (self.max / self.min).ln()
        } else {
            (value - self.min) / (self.max - self.min).max(f64::EPSILON)
        }
    }
    pub fn value(self, fraction: f64) -> f64 {
        if self.logarithmic {
            self.min * (self.max / self.min).powf(fraction)
        } else {
            self.min + (self.max - self.min) * fraction
        }
    }
}

pub struct Binding {
    pub parameter: ParameterDetail,
    pub range: Range,
    pub horizontal: bool,
    pub pixels: f64,
    fraction: f64,
}
impl Binding {
    pub fn new(parameter: ParameterDetail, range: Range, horizontal: bool, pixels: f64) -> Self {
        let fraction = range.fraction(parameter.value);
        Self {
            parameter,
            range,
            horizontal,
            pixels: pixels.max(1.),
            fraction,
        }
    }
    pub fn advance(&mut self, dx: f64, dy: f64, fine: bool) -> f64 {
        let distance = if self.horizontal { dx } else { -dy };
        // A perpendicular drag must not round or clamp an untouched parameter.
        if distance == 0. {
            return self.parameter.value;
        }
        let spec = &self.parameter.spec;
        let low = self.range.fraction(spec.min);
        let high = self.range.fraction(spec.max);
        self.fraction =
            (self.fraction + distance / self.pixels * if fine { 0.1 } else { 1. }).clamp(low, high);
        let value = self.range.value(self.fraction).clamp(spec.min, spec.max);
        self.parameter.value = if spec.unit == ParameterUnit::Enum {
            value.round()
        } else {
            value
        };
        self.parameter.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logarithmic_graph_drag_tracks_octaves_and_preserves_untouched_values() {
        let descriptor = oxitone_instruments::wavetable::descriptor();
        let mut parameter = crate::plugin_plot_tests::details(descriptor.clone())
            .parameters
            .into_iter()
            .find(|p| p.spec.id == "filter.cutoff")
            .unwrap();
        parameter.value = 1000.;
        let range = Range::frequency(20_000.);
        let mut binding = Binding::new(parameter, range, true, 600.);
        assert_eq!(binding.advance(0., 80., false), 1000.);
        let octave = 600. * 2f64.ln() / 1000f64.ln();
        assert!((binding.advance(octave, 0., false) - 2000.).abs() < 1e-8);
        assert!((binding.advance(-octave * 10., 0., true) - 1000.).abs() < 1e-8);
        assert_eq!(binding.advance(1e6, 0., false), binding.parameter.spec.max);
    }
}
