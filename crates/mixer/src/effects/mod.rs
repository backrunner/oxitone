//! Phase 1 built-in effect plugins (02-domain-spec.md §Mixer 与 routing).
//! Every plugin declares `oxitone.<name>` at version `1.0.0`, stereo in/out,
//! and the full `ParameterSpec` table (unit/range/default/smoothing/rate/
//! automation). All parameters are control-rate: parameter events are applied
//! in frame order at block granularity and smoothed inside the block where the
//! spec declares smoothing. Host-side `mix`/`bypass` is stacked by the
//! compiler (04-api-contracts.md §EffectInsert) and is not part of the plugin.

pub mod chorus;
pub mod clipper;
pub mod compressor;
pub mod delay;
pub mod eq;
pub mod filter;
pub mod gate;
pub mod limit;
pub mod oversample;
pub mod phaser;
pub mod reverb;
pub mod saturator;
pub mod utility;

use oxitone_core::wire::{
    ParameterMapping, ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit,
};
use oxitone_graph::{ChannelLayout, Plugin, PluginCapabilities, PluginDescriptor, PluginKind};

/// All Phase 1 built-in effects as ABI v1 plugins, ready for registration.
pub fn builtin_effect_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(eq::EqPlugin),
        Box::new(limit::LimitPlugin),
        Box::new(clipper::ClipperPlugin),
        Box::new(filter::FilterPlugin),
        Box::new(phaser::PhaserPlugin),
        Box::new(reverb::ReverbPlugin),
        Box::new(compressor::CompressorPlugin),
        Box::new(delay::DelayPlugin),
        Box::new(gate::GatePlugin),
        Box::new(chorus::ChorusPlugin),
        Box::new(saturator::SaturatorPlugin),
        Box::new(utility::UtilityPlugin),
    ]
}

/// Continuous control-rate parameter.
#[allow(clippy::too_many_arguments)]
pub(crate) fn param(
    id: &str,
    label: &str,
    unit: ParameterUnit,
    min: f64,
    max: f64,
    default: f64,
    smoothing: ParameterSmoothing,
    mapping: ParameterMapping,
) -> ParameterSpec {
    ParameterSpec {
        id: id.to_string(),
        label: label.to_string(),
        unit,
        min,
        max,
        default,
        smoothing,
        rate: ParameterRate::Control,
        automation: Some(true),
        mapping: Some(mapping),
    }
}

/// Step (enumerated) parameter; never smoothed.
pub(crate) fn enum_param(id: &str, label: &str, min: f64, max: f64, default: f64) -> ParameterSpec {
    ParameterSpec {
        unit: ParameterUnit::Enum,
        smoothing: ParameterSmoothing::None,
        mapping: Some(ParameterMapping::Enum),
        ..param(
            id,
            label,
            ParameterUnit::Enum,
            min,
            max,
            default,
            ParameterSmoothing::None,
            ParameterMapping::Enum,
        )
    }
}

/// Stereo effect descriptor shell shared by all built-ins.
pub(crate) fn descriptor(
    plugin_id: &'static str,
    parameters: Vec<ParameterSpec>,
    capabilities: PluginCapabilities,
) -> PluginDescriptor {
    PluginDescriptor {
        plugin_id,
        plugin_version: "1.0.0",
        kind: PluginKind::Effect,
        input_layout: ChannelLayout::Stereo,
        output_layout: ChannelLayout::Stereo,
        parameters,
        capabilities,
        state_schema: None,
        max_polyphony: None,
    }
}

/// Copy the stereo input buses onto the output buses for `ctx.frames`.
/// Effects that process in place call this first, then mutate the outputs.
pub(crate) fn pass_inputs(ctx: &mut oxitone_graph::ProcessContext<'_>) {
    for (input, output) in ctx.inputs.iter().zip(ctx.outputs.iter_mut()) {
        output[..ctx.frames].copy_from_slice(&input[..ctx.frames]);
    }
}

/// dB → linear amplitude.
pub(crate) fn db_to_linear(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}
