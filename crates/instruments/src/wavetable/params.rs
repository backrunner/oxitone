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
pub const OSC_A_MORPH_TO: usize = 27;
pub const OSC_A_POSITION: usize = 28;
pub const OSC_A_PHASE: usize = 29;
pub const OSC_A_PHASE_SPREAD: usize = 30;
pub const OSC_B_MORPH_TO: usize = 31;
pub const OSC_B_POSITION: usize = 32;
pub const OSC_B_PHASE: usize = 33;
pub const OSC_B_PHASE_SPREAD: usize = 34;
pub const SUB_LEVEL: usize = 35;
pub const SUB_OCTAVE: usize = 36;
pub const NOISE_LEVEL: usize = 37;
pub const LFO_SHAPE: usize = 38;
pub const LFO_RATE: usize = 39;
pub const LFO_PHASE: usize = 40;
pub const LFO_PITCH: usize = 41;
pub const LFO_CUTOFF: usize = 42;
pub const LFO_POSITION_A: usize = 43;
pub const LFO_POSITION_B: usize = 44;
pub const LFO_LEVEL: usize = 45;

/// Wavetable enum values (also the instance's table indices).
pub const WAVETABLE_COUNT: usize = 6;

pub const FILTER_LOWPASS: usize = 0;
pub const FILTER_HIGHPASS: usize = 1;
pub const FILTER_BANDPASS: usize = 2;

pub const MODE_POLY: usize = 0;
pub const MODE_MONO: usize = 1;
pub const MODE_LEGATO: usize = 2;

pub fn parameters() -> Vec<ParameterSpec> {
    let mut parameters = vec![
        enum_spec("oscA.wavetable", "Osc A Wavetable", 0.0, 5.0, 1.0),
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
        enum_spec("oscB.wavetable", "Osc B Wavetable", 0.0, 5.0, 2.0),
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
    ];
    parameters.extend(super::motion::parameters());
    parameters
}

pub fn descriptor() -> PluginDescriptor {
    PluginDescriptor {
        plugin_id: crate::WAVETABLE_PLUGIN_ID.into(),
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
        max_polyphony: Some(64),
    }
}
