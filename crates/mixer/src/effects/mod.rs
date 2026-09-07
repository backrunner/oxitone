//! Phase 1 built-in effect plugins (02-domain-spec.md §Mixer 与 routing).
//! Every plugin declares `oxitone.<name>` at version `1.0.0`, stereo in/out,
//! and the full `ParameterSpec` table (unit/range/default/smoothing/rate/
//! automation). All parameters are control-rate: parameter events are applied
//! in frame order at block granularity and smoothed inside the block where the
//! spec declares smoothing. Host-side `mix`/`bypass` is stacked by the
//! compiler (04-api-contracts.md §EffectInsert) and is not part of the plugin.

pub mod bitcrush;
pub mod chorus;
pub mod clipper;
pub mod compactor;
pub mod compressor;
mod controls;
pub mod convolver;
mod crossover;
pub mod delay;
pub mod distortion;
pub mod eq;
pub mod filter;
pub mod flanger;
mod fractional;
pub mod frequency_shifter;
pub mod gate;
pub mod limit;
pub mod limiter;
pub mod multiband;
pub mod multiband_dynamics;
pub mod nonlinear_filter;
pub mod oversample;
mod peak_window;
pub mod phaser;
pub mod pitch_shifter;
pub mod resonator;
pub mod reverb;
pub mod saturator;
pub mod spreader;
pub mod tape;
pub mod utility;
mod wet_shape;

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
        Box::new(distortion::DistortionPlugin),
        Box::new(multiband::MultibandPlugin),
        Box::new(nonlinear_filter::NonlinearFilterPlugin),
        Box::new(compactor::CompactorPlugin),
        Box::new(multiband_dynamics::MultibandDynamicsPlugin),
        Box::new(resonator::ResonatorPlugin),
        Box::new(frequency_shifter::FrequencyShifterPlugin),
        Box::new(pitch_shifter::PitchShifterPlugin),
        Box::new(flanger::FlangerPlugin),
        Box::new(convolver::ConvolverPlugin),
        Box::new(bitcrush::BitcrushPlugin),
        Box::new(tape::TapePlugin),
        Box::new(spreader::SpreaderPlugin),
        Box::new(limiter::LimiterPlugin),
    ]
}

/// Control-thread creation, with optional prepared convolution resources.
pub fn create_effect(
    plugin: &dyn Plugin,
    reference: &oxitone_core::wire::EffectRef,
    host: &oxitone_graph::HostContext,
    resources: Option<&dyn convolver::ImpulseProvider>,
) -> Result<Box<dyn oxitone_graph::PluginInstance>, oxitone_core::OxitoneError> {
    if reference.plugin_id == "oxitone.convolver" {
        if let Some(bindings) = &reference.resources {
            let id = bindings
                .get("impulse")
                .filter(|id| !id.is_empty())
                .filter(|_| bindings.len() == 1)
                .ok_or_else(|| {
                    oxitone_core::OxitoneError::new(
                        oxitone_core::error::codes::INVALID_PROJECT,
                        "convolver resources require exactly one impulse sample ID",
                    )
                })?;
            let provider = resources.ok_or_else(|| {
                oxitone_core::OxitoneError::new(
                    oxitone_core::error::codes::ASSET_UNAVAILABLE,
                    "convolver impulse provider is unavailable",
                )
            })?;
            return convolver::from_impulse(provider.impulse(id)?, host.sample_rate);
        }
    }
    plugin.try_create(host)
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
        plugin_id: plugin_id.into(),
        plugin_version: "1.0.0".into(),
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
