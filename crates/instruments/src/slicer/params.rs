//! Slicer parameter table. Slice structure lives in the structured state
//! (compile-time, immutable); these are the runtime playback parameters.
//! Indices match the descriptor order.

use oxitone_core::wire::{
    ParameterMapping, ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit,
};
use oxitone_graph::descriptor::PluginDescriptor;

use crate::params::{bipolar, smoothed};

pub const LEVEL: usize = 0;
pub const PAN: usize = 1;
pub const TEMPO_FACTOR: usize = 2;

pub fn parameters() -> Vec<ParameterSpec> {
    vec![
        smoothed("level", "Level", 0.0, 2.0, 1.0),
        bipolar("pan", "Pan", 0.0),
        // Host-driven repitch factor for tempoSync: 'repitch' (the graph
        // compiler multiplies the baked tempo factor in per block). Not a
        // user automation target — the compiler owns it.
        ParameterSpec {
            id: "tempoFactor".to_string(),
            label: "Tempo Factor".to_string(),
            unit: ParameterUnit::Normalized,
            min: 0.25,
            max: 4.0,
            default: 1.0,
            smoothing: ParameterSmoothing::None,
            rate: ParameterRate::Control,
            automation: Some(false),
            mapping: Some(ParameterMapping::Log),
        },
    ]
}

pub fn descriptor() -> PluginDescriptor {
    PluginDescriptor {
        plugin_id: crate::SLICER_PLUGIN_ID.into(),
        plugin_version: crate::BUILTIN_PLUGIN_VERSION.into(),
        kind: oxitone_graph::PluginKind::Instrument,
        input_layout: oxitone_graph::ChannelLayout::None,
        output_layout: oxitone_graph::ChannelLayout::Stereo,
        parameters: parameters(),
        capabilities: oxitone_graph::PluginCapabilities {
            sidechain_input: false,
            reports_tail: true,
        },
        state_schema: Some(oxitone_graph::SLICER_STATE_SCHEMA_ID),
        max_polyphony: Some(super::state::MAX_SLICES as u32),
    }
}
