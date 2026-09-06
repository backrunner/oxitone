//! Control-thread descriptors and resolved realtime insert setters.

use super::MixerEngine;
use oxitone_core::wire::ParameterSpec;
use std::sync::Arc;

impl MixerEngine {
    pub fn bus_index(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// Control thread only: returns immutable tables in insert order.
    pub fn insert_specs(&self, bus: usize) -> Vec<Arc<Vec<ParameterSpec>>> {
        self.buses[bus]
            .inserts
            .iter()
            .map(|s| s.specs.clone())
            .collect()
    }

    /// Resolved indices and validated values only; allocation-free.
    pub fn set_insert_mix_at(&mut self, bus: usize, insert: usize, mix: f64) {
        self.buses[bus].inserts[insert].mix.set_target(mix as f32);
    }

    pub fn set_insert_bypass_at(&mut self, bus: usize, insert: usize, bypass: bool) {
        self.buses[bus].inserts[insert].bypass = bypass;
    }

    pub fn set_insert_parameter_at(&mut self, bus: usize, insert: usize, index: usize, value: f64) {
        self.buses[bus].inserts[insert].pending.set(index, value);
    }
}
