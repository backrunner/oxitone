//! Sampler parameter table (02-domain-spec.md §内置 Sampler). Indices match
//! the descriptor order; instances keep a dense value table.

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterSpec, ParameterUnit};
use oxitone_graph::descriptor::PluginDescriptor;

use crate::params::{bipolar, enum_spec, seconds, smoothed, spec, stepped_continuous};

pub const ROOT_KEY: usize = 0;
pub const VELOCITY_SENSITIVITY: usize = 1;
pub const AMP_ATTACK: usize = 2;
pub const AMP_DECAY: usize = 3;
pub const AMP_SUSTAIN: usize = 4;
pub const AMP_RELEASE: usize = 5;
pub const LOOP_MODE: usize = 6;
pub const START_SECONDS: usize = 7;
pub const LEVEL: usize = 8;
pub const PAN: usize = 9;

pub const LOOP_OFF: usize = 0;
pub const LOOP_FORWARD: usize = 1;

pub fn parameters() -> Vec<ParameterSpec> {
    vec![
        enum_spec("rootKey", "Root Key", 0.0, 127.0, 60.0),
        stepped_continuous("velocitySensitivity", "Velocity Sensitivity", 0.0, 1.0, 1.0),
        seconds("amp.attack", "Amp Attack", 0.005),
        seconds("amp.decay", "Amp Decay", 0.1),
        stepped_continuous("amp.sustain", "Amp Sustain", 0.0, 1.0, 1.0),
        seconds("amp.release", "Amp Release", 0.2),
        enum_spec("loop", "Loop Mode", 0.0, 1.0, 0.0),
        spec(
            "start",
            "Start Offset",
            ParameterUnit::Seconds,
            0.0,
            600.0,
            0.0,
            ParameterSmoothing::None,
            ParameterMapping::Linear,
        ),
        smoothed("level", "Level", 0.0, 2.0, 1.0),
        bipolar("pan", "Pan", 0.0),
    ]
}

pub fn descriptor() -> PluginDescriptor {
    PluginDescriptor {
        plugin_id: crate::SAMPLER_PLUGIN_ID.into(),
        plugin_version: crate::BUILTIN_PLUGIN_VERSION.into(),
        kind: oxitone_graph::PluginKind::Instrument,
        input_layout: oxitone_graph::ChannelLayout::None,
        output_layout: oxitone_graph::ChannelLayout::Stereo,
        parameters: parameters(),
        capabilities: oxitone_graph::PluginCapabilities {
            sidechain_input: false,
            reports_tail: true,
        },
        state_schema: None,
        max_polyphony: Some(32),
    }
}
