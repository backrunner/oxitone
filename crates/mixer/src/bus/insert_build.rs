//! Prepare insert instances, bus activation and delays relative to the owner's serial chain.
use super::{
    routes::{self, InputRoute, OutputRoute, SidechainInput},
    InsertSlot,
};
use oxitone_core::{wire::MixerChannelSpec, OxitoneError};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{topology::MixerRouting, HostContext, PluginRegistry};

pub(super) fn build(
    spec: &MixerChannelSpec,
    channels: &[MixerChannelSpec],
    routing: &MixerRouting,
    registry: &PluginRegistry,
    host: &HostContext,
    resources: Option<&dyn crate::effects::convolver::ImpulseProvider>,
) -> Result<(Vec<InsertSlot>, u64), OxitoneError> {
    let block = host.max_block_size as usize;
    let mut slots = Vec::new();
    let mut latency = 0u64;
    for effect in &spec.inserts {
        let plugin = registry
            .lookup_instance(
                &effect.plugin_id,
                &effect.plugin_version,
                effect.instance_id.as_deref(),
            )
            .ok_or_else(|| OxitoneError::new("InvalidProject", "unknown mixer insert plugin"))?;
        let descriptor = plugin.descriptor();
        let routes = effect
            .instance_id
            .as_ref()
            .and_then(|id| spec.insert_routes.as_ref()?.get(id));
        let mut inputs: Vec<_> = routes
            .into_iter()
            .flat_map(|r| r.inputs.iter().flatten())
            .map(|(index, _)| index.parse::<usize>().expect("validated input index"))
            .collect();
        inputs.sort_unstable();
        let mut outputs: Vec<_> = routes
            .into_iter()
            .flat_map(|r| r.outputs.iter().flatten())
            .map(|(index, target)| {
                (
                    index.parse::<usize>().expect("validated output index"),
                    routing
                        .order
                        .iter()
                        .position(|id| id == target)
                        .expect("validated destination"),
                )
            })
            .collect();
        outputs.sort_unstable();
        let accepts_sidechain = descriptor.capabilities.sidechain_input
            && !inputs.contains(&1)
            && channels.iter().any(|source| {
                source
                    .sends
                    .iter()
                    .any(|send| send.sidechain.unwrap_or(false) && send.destination_id == spec.id)
            });
        let mut instance = crate::effects::create_effect(plugin.as_ref(), effect, host, resources)?;
        if (!inputs.is_empty() || !outputs.is_empty()) && !instance.requires_isolation() {
            return Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "auxiliary insert routes require isolated processing",
            ));
        }
        let mut activation = inputs.clone();
        if accepts_sidechain && plugin.input_bus_count() > 1 {
            activation.push(1);
        }
        instance.configure_input_buses(&activation)?;
        instance
            .configure_output_buses(&outputs.iter().map(|(index, _)| *index).collect::<Vec<_>>())?;
        instance.try_prepare(host.sample_rate, host.max_block_size)?;
        let param_ids: Vec<String> = descriptor.parameters.iter().map(|p| p.id.clone()).collect();
        let mut pending = crate::parameter_queue::ParameterQueue::new(&descriptor.parameters);
        for (id, value) in &effect.parameters {
            let index = param_ids.iter().position(|p| p == id).ok_or_else(|| {
                OxitoneError::new("InvalidProject", format!("unknown insert parameter {id:?}"))
            })?;
            if !instance.initial_parameters_applied() {
                pending.set(index, *value);
            }
        }
        let mut mix = OnePoleSmoother::new(host.sample_rate, 20.);
        mix.snap(effect.mix.unwrap_or(1.) as f32);
        let slot_latency = instance.latency_frames();
        slots.push(InsertSlot {
            inputs: inputs
                .into_iter()
                .map(|index| InputRoute::new(index, latency as usize, block))
                .collect(),
            outputs: outputs
                .into_iter()
                .map(|(index, target)| OutputRoute::new(index, target, block))
                .collect(),
            sidechain: accepts_sidechain.then(|| SidechainInput::new(latency as usize, block)),
            dry_delay: routes::delay(slot_latency as usize, block),
            dry_l: vec![0.; block],
            dry_r: vec![0.; block],
            mix,
            bypass: effect.bypass.unwrap_or(false),
            instance,
            param_ids,
            specs: std::sync::Arc::new(descriptor.parameters.clone()),
            pending,
        });
        latency += slot_latency;
    }
    let mut remaining = latency;
    for slot in &mut slots {
        remaining -= slot.instance.latency_frames();
        for output in &mut slot.outputs {
            output.catchup = routes::delay(remaining as usize, block);
        }
    }
    Ok((slots, latency))
}
