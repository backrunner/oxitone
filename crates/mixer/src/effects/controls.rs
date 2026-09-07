//! Fixed control tables and per-sample smoothing for production effects.
use oxitone_core::wire::{
    ParameterMapping as Map, ParameterSmoothing as Smooth, ParameterSpec, ParameterUnit as Unit,
};
use oxitone_graph::ParameterEvent;

#[derive(Clone, Copy)]
pub(super) struct Control {
    pub id: &'static str,
    pub label: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    unit: Unit,
    mapping: Map,
}
impl Control {
    pub const fn linear(
        id: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        default: f64,
    ) -> Self {
        Self {
            id,
            label,
            min,
            max,
            default,
            unit: Unit::Normalized,
            mapping: Map::Linear,
        }
    }
    pub const fn log(
        id: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        default: f64,
    ) -> Self {
        Self {
            mapping: Map::Log,
            ..Self::linear(id, label, min, max, default)
        }
    }
    pub const fn hz(
        id: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        default: f64,
    ) -> Self {
        Self {
            unit: Unit::Hz,
            ..Self::log(id, label, min, max, default)
        }
    }
    pub const fn db(
        id: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        default: f64,
    ) -> Self {
        Self {
            unit: Unit::Db,
            ..Self::linear(id, label, min, max, default)
        }
    }
    pub const fn seconds(
        id: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        default: f64,
    ) -> Self {
        Self {
            unit: Unit::Seconds,
            ..Self::log(id, label, min, max, default)
        }
    }
    pub const fn choice(id: &'static str, label: &'static str, max: f64, default: f64) -> Self {
        Self {
            unit: Unit::Enum,
            mapping: Map::Enum,
            ..Self::linear(id, label, 0., max, default)
        }
    }
    pub fn spec(self) -> ParameterSpec {
        super::param(
            self.id,
            self.label,
            self.unit,
            self.min,
            self.max,
            self.default,
            if self.unit == Unit::Enum {
                Smooth::None
            } else {
                Smooth::OnePole
            },
            self.mapping,
        )
    }
}
pub(super) struct Controls<const N: usize> {
    table: &'static [Control; N],
    current: [f64; N],
    pub target: [f64; N],
    coefficient: f64,
}
impl<const N: usize> Controls<N> {
    pub fn new(table: &'static [Control; N], rate: f64) -> Self {
        let values = table.map(|p| p.default);
        Self {
            table,
            current: values,
            target: values,
            coefficient: 1. - (-1. / (0.005 * rate)).exp(),
        }
    }
    pub fn events(&mut self, events: &[ParameterEvent<'_>]) {
        for event in events {
            if let Some(i) = self.table.iter().position(|p| p.id == event.parameter_id) {
                if event.value.is_finite() {
                    let value = event.value.clamp(self.table[i].min, self.table[i].max);
                    self.target[i] = if self.table[i].unit == Unit::Enum {
                        value.round()
                    } else {
                        value
                    };
                }
            }
        }
    }
    #[inline]
    pub fn next(&mut self) -> [f64; N] {
        for i in 0..N {
            let delta = self.target[i] - self.current[i];
            if delta.abs() < 1e-8 || self.table[i].unit == Unit::Enum {
                self.current[i] = self.target[i];
            } else {
                self.current[i] += delta * self.coefficient;
            }
        }
        self.current
    }
    pub fn reset(&mut self) {
        self.current = self.target;
    }
}
