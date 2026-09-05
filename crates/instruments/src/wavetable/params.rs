//! WavetableSynth parameter table (02-domain-spec.md §内置 WavetableSynth).
//! Indices match the descriptor order; instances keep a dense value table.

use oxitone_core::wire::ParameterSpec;
use oxitone_graph::descriptor::PluginDescriptor;

use crate::params::{bipolar, enum_spec, seconds, smoothed, spec, stepped_continuous};
use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};

pub const OSC_A_WAVETABLE: usize = 0;
pub const OSC_A_PITCH: usize = 1;
pub const OSC_A_UNISON: usize = 2;
pub const OSC_A_DETUNE: usize = 3;
pub const OSC_A_SPREAD: usize = 4;
pub const OSC_B_WAVETABLE: usize = 5;
pub const OSC_B_PITCH: usize = 6;
pub const OSC_B_UNISON: usize = 7;
pub const OSC_B_DETUNE: usize = 8;
pub const OSC_B_SPREAD: usize = 9;
pub const OSC_MIX: usize = 10;
pub const FILTER_TYPE: usize = 11;
pub const FILTER_CUTOFF: usize = 12;
pub const FILTER_RESONANCE: usize = 13;
pub const FILTER_ENV_AMOUNT: usize = 14;
pub const FILTER_ENV_ATTACK: usize = 15;
pub const FILTER_ENV_DECAY: usize = 16;
pub const FILTER_ENV_SUSTAIN: usize = 17;
pub const FILTER_ENV_RELEASE: usize = 18;
pub const AMP_ATTACK: usize = 19;
pub const AMP_DECAY: usize = 20;
pub const AMP_SUSTAIN: usize = 21;
pub const AMP_RELEASE: usize = 22;
pub const VOICE_MODE: usize = 23;
pub const GLIDE: usize = 24;
pub const LEVEL: usize = 25;
pub const PAN: usize = 26;

/// Wavetable enum values (also the instance's table indices).
pub const WAVETABLE_SINE: usize = 0;
pub const WAVETABLE_SAW: usize = 1;
pub const WAVETABLE_SQUARE: usize = 2;
pub const WAVETABLE_TRIANGLE: usize = 3;
pub const WAVETABLE_COUNT: usize = 4;

pub const FILTER_LOWPASS: usize = 0;
pub const FILTER_HIGHPASS: usize = 1;
pub const FILTER_BANDPASS: usize = 2;

pub const MODE_POLY: usize = 0;
pub const MODE_MONO: usize = 1;
pub const MODE_LEGATO: usize = 2;

pub fn parameters() -> Vec<ParameterSpec> {
    vec![
        enum_spec("oscA.wavetable", "Osc A Wavetable", 0.0, 3.0, 1.0),
        spec(
            "oscA.pitch",
            "Osc A Pitch",
            ParameterUnit::Semitones,
            -24.0,
            24.0,
            0.0,
            ParameterSmoothing::None,
            ParameterMapping::Bipolar,
        ),
        enum_spec("oscA.unison", "Osc A Unison", 1.0, 8.0, 1.0),
        stepped_continuous("oscA.detune", "Osc A Detune (cents)", 0.0, 100.0, 8.0),
        stepped_continuous("oscA.spread", "Osc A Spread", 0.0, 1.0, 0.6),
        enum_spec("oscB.wavetable", "Osc B Wavetable", 0.0, 3.0, 2.0),
        spec(
            "oscB.pitch",
            "Osc B Pitch",
            ParameterUnit::Semitones,
            -24.0,
            24.0,
            0.0,
            ParameterSmoothing::None,
            ParameterMapping::Bipolar,
        ),
        enum_spec("oscB.unison", "Osc B Unison", 1.0, 8.0, 1.0),
        stepped_continuous("oscB.detune", "Osc B Detune (cents)", 0.0, 100.0, 8.0),
        stepped_continuous("oscB.spread", "Osc B Spread", 0.0, 1.0, 0.6),
        smoothed("osc.mix", "Osc Mix", 0.0, 1.0, 0.0),
        enum_spec("filter.type", "Filter Type", 0.0, 2.0, 0.0),
        spec(
            "filter.cutoff",
            "Filter Cutoff",
            ParameterUnit::Hz,
            20.0,
            20_000.0,
            18_000.0,
            ParameterSmoothing::OnePole,
            ParameterMapping::Log,
        ),
        smoothed("filter.resonance", "Filter Resonance", 0.0, 1.0, 0.0),
        spec(
            "filterEnv.amount",
            "Filter Env Amount",
            ParameterUnit::Semitones,
            -48.0,
            48.0,
            0.0,
            ParameterSmoothing::None,
            ParameterMapping::Bipolar,
        ),
        seconds("filterEnv.attack", "Filter Env Attack", 0.005),
        seconds("filterEnv.decay", "Filter Env Decay", 0.1),
        stepped_continuous("filterEnv.sustain", "Filter Env Sustain", 0.0, 1.0, 0.0),
        seconds("filterEnv.release", "Filter Env Release", 0.2),
        seconds("amp.attack", "Amp Attack", 0.005),
        seconds("amp.decay", "Amp Decay", 0.1),
        stepped_continuous("amp.sustain", "Amp Sustain", 0.0, 1.0, 0.8),
        seconds("amp.release", "Amp Release", 0.2),
        enum_spec("voiceMode", "Voice Mode", 0.0, 2.0, 0.0),
        spec(
            "glide",
            "Glide",
            ParameterUnit::Seconds,
            0.0,
            2.0,
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
        plugin_id: crate::WAVETABLE_PLUGIN_ID,
        plugin_version: crate::BUILTIN_PLUGIN_VERSION,
        kind: oxitone_graph::PluginKind::Instrument,
        input_layout: oxitone_graph::ChannelLayout::None,
        output_layout: oxitone_graph::ChannelLayout::Stereo,
        parameters: parameters(),
        capabilities: oxitone_graph::PluginCapabilities {
            sidechain_input: false,
            reports_tail: true,
        },
        state_schema: None,
        max_polyphony: Some(64),
    }
}
