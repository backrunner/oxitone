//! Control-thread construction and bus activation for channel instruments.
use crate::assets::SampleStore;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_graph::abi::{HostContext, PluginInstance};
use oxitone_graph::PluginRegistry;
use std::sync::Arc;

fn invalid(message: impl Into<String>, path: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

/// Instrument instance plus its parameter table, specs and initial events.
type CreatedInstrument = (
    Box<dyn PluginInstance>,
    Arc<Vec<String>>,
    Arc<Vec<oxitone_core::wire::ParameterSpec>>,
    Vec<(usize, f64)>,
);

/// Create the instrument instance for one channel plan.
pub(super) fn create_instrument(
    channel: &oxitone_graph::compile::ChannelPlan,
    host: &HostContext,
    registry: &PluginRegistry,
    samples: &SampleStore,
    tempo: &oxitone_transport::CompiledTempoMap,
    sample_rate: f64,
    max_block: u32,
) -> Result<CreatedInstrument, OxitoneError> {
    let reference = &channel.instrument;
    let builtin = matches!(
        reference.plugin_id.as_str(),
        oxitone_instruments::WAVETABLE_PLUGIN_ID
            | oxitone_instruments::SAMPLER_PLUGIN_ID
            | oxitone_graph::multisampler::PLUGIN_ID
            | oxitone_instruments::SLICER_PLUGIN_ID
    );
    let descriptor = registry
        .instance_descriptor(
            &reference.plugin_id,
            &reference.plugin_version,
            reference.instance_id.as_deref(),
        )
        .ok_or_else(|| {
            invalid(
                format!(
                    "unknown plugin {}@{}",
                    reference.plugin_id, reference.plugin_version
                ),
                format!("$.channels[{}].instrument", channel.id),
            )
        })?;
    let param_ids: Arc<Vec<String>> =
        Arc::new(descriptor.parameters.iter().map(|p| p.id.clone()).collect());
    let param_specs: Arc<Vec<oxitone_core::wire::ParameterSpec>> =
        Arc::new(descriptor.parameters.clone());
    let mut initial = Vec::new();
    let mut instance = if builtin {
        let config = oxitone_instruments::InstrumentConfig {
            parameters: &reference.parameters,
            resources: reference.resources.as_ref(),
            state: reference.state.as_ref(),
        };
        let beat_to_frame = move |beat: oxitone_core::Beat| Some(tempo.beat_to_frame(beat));
        oxitone_instruments::create_builtin_instance(
            &reference.plugin_id,
            host,
            &config,
            samples,
            Some(&beat_to_frame),
        )?
    } else {
        let plugin = registry
            .lookup_instance(
                &reference.plugin_id,
                &reference.plugin_version,
                reference.instance_id.as_deref(),
            )
            .expect("descriptor lookup succeeded");
        for (id, value) in &reference.parameters {
            let Some(index) = param_ids.iter().position(|p| p == id) else {
                return Err(invalid(
                    format!("unknown parameter {id:?} on {}", reference.plugin_id),
                    format!("$.channels[{}].instrument.parameters", channel.id),
                ));
            };
            initial.push((index, *value));
        }
        plugin.try_create_configured(host, &reference.parameters, reference.state.as_ref())?
    };
    instance.configure_output_buses(&channel.output_routes.keys().copied().collect::<Vec<_>>())?;
    instance.configure_midi_output(
        reference
            .instance_id
            .as_ref()
            .is_some_and(|id| channel.midi_routes.contains_key(id)),
    )?;
    if !channel.output_routes.is_empty() && !instance.requires_isolation() {
        return Err(OxitoneError::new(
            "PluginCapabilityUnsupported",
            "auxiliary output routing requires an isolated instrument adapter",
        ));
    }
    instance.try_prepare(sample_rate, max_block)?;
    if instance.initial_parameters_applied() {
        initial.clear();
    }
    Ok((instance, param_ids, param_specs, initial))
}
