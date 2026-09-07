//! Control-thread mixer construction, resource preparation and routing/PDC.
use super::{Bus, InsertSlot, MixerEngine, SendSlot};
use crate::meter::{BusMeter, TruePeakMeter};
use crate::pdc::{plan_pdc, DelayLine};
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EntityId, MixerChannelSpec};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::abi::HostContext;
use oxitone_graph::registry::PluginRegistry;
use oxitone_graph::topology::{build_mixer_routing, MASTER_MIXER_CHANNEL_ID};
use std::collections::BTreeMap;

impl MixerEngine {
    /// Compile from validated mixer channel specs. Insert plugins are
    /// created through the registry (ABI v1), prepared, and seeded with the
    /// `EffectRef` parameter values. Control thread; allocates.
    pub fn build(
        sample_rate: f64,
        max_block_size: u32,
        channels: &[MixerChannelSpec],
        registry: &PluginRegistry,
    ) -> Result<Self, OxitoneError> {
        Self::build_with_resources(sample_rate, max_block_size, channels, registry, None)
    }

    pub fn build_with_resources(
        sample_rate: f64,
        max_block_size: u32,
        channels: &[MixerChannelSpec],
        registry: &PluginRegistry,
        resources: Option<&dyn crate::effects::convolver::ImpulseProvider>,
    ) -> Result<Self, OxitoneError> {
        let routing = build_mixer_routing(channels)?;
        let specs: BTreeMap<&str, &MixerChannelSpec> =
            channels.iter().map(|c| (c.id.as_str(), c)).collect();
        let max_block = max_block_size as usize;
        let host = HostContext {
            sample_rate,
            max_block_size,
        };

        let mut insert_latency: BTreeMap<EntityId, u64> = BTreeMap::new();
        let mut insert_chains: BTreeMap<EntityId, Vec<InsertSlot>> = BTreeMap::new();
        for id in &routing.order {
            let mut slots = Vec::new();
            let mut latency = 0u64;
            if let Some(spec) = specs.get(id.as_str()) {
                for effect in &spec.inserts {
                    let plugin = registry
                        .lookup(&effect.plugin_id, &effect.plugin_version)
                        .ok_or_else(|| {
                            OxitoneError::with_path(
                                codes::INVALID_PROJECT,
                                format!(
                                    "unknown plugin {}@{}",
                                    effect.plugin_id, effect.plugin_version
                                ),
                                format!("$.mixerChannels.{id}.inserts"),
                            )
                        })?;
                    let descriptor = plugin.descriptor();
                    let mut instance =
                        crate::effects::create_effect(plugin.as_ref(), effect, &host, resources)?;
                    instance.try_prepare(sample_rate, max_block_size)?;
                    let param_ids: Vec<String> =
                        descriptor.parameters.iter().map(|p| p.id.clone()).collect();
                    let mut pending =
                        crate::parameter_queue::ParameterQueue::new(&descriptor.parameters);
                    for (param_id, value) in &effect.parameters {
                        let index =
                            param_ids
                                .iter()
                                .position(|id| id == param_id)
                                .ok_or_else(|| {
                                    OxitoneError::with_path(
                                        codes::INVALID_PROJECT,
                                        format!(
                                            "unknown parameter {param_id:?} on {}",
                                            effect.plugin_id
                                        ),
                                        format!("$.mixerChannels.{id}.inserts"),
                                    )
                                })?;
                        pending.set(index, *value);
                    }
                    latency += instance.latency_frames();
                    let mut mix = OnePoleSmoother::new(sample_rate, 20.0);
                    mix.snap(effect.mix.unwrap_or(1.0) as f32);
                    let mut dry_delay =
                        DelayLine::new(instance.latency_frames() as usize, max_block);
                    dry_delay.set_delay(instance.latency_frames() as usize);
                    slots.push(InsertSlot {
                        mix,
                        bypass: effect.bypass.unwrap_or(false),
                        dry_delay,
                        dry_l: vec![0.0; max_block],
                        dry_r: vec![0.0; max_block],
                        instance,
                        accepts_sidechain: descriptor.capabilities.sidechain_input
                            && channels.iter().any(|source| {
                                source.sends.iter().any(|send| {
                                    send.sidechain.unwrap_or(false) && send.destination_id == *id
                                })
                            }),
                        param_ids,
                        specs: std::sync::Arc::new(descriptor.parameters.clone()),
                        pending,
                    });
                }
            }
            insert_latency.insert(id.clone(), latency);
            insert_chains.insert(id.clone(), slots);
        }

        let pdc = plan_pdc(&routing, &insert_latency);
        let max_edge_delay = pdc.edge_delays.values().copied().max().unwrap_or(0) as usize;

        let mut buses = Vec::new();
        let mut index = BTreeMap::new();
        for (position, id) in routing.order.iter().enumerate() {
            let is_master = id == MASTER_MIXER_CHANNEL_ID;
            let spec = specs.get(id.as_str());
            let mut level = OnePoleSmoother::new(sample_rate, 20.0);
            level.snap(spec.map(|s| s.level as f32).unwrap_or(1.0));
            let sends = spec
                .map(|s| {
                    s.sends
                        .iter()
                        .filter_map(|send| {
                            let dest = routing
                                .order
                                .iter()
                                .position(|d| d == &send.destination_id)?;
                            let sidechain = send.sidechain.unwrap_or(false);
                            let mut delay = DelayLine::new(max_edge_delay, max_block);
                            delay.set_delay(
                                pdc.edge_delays
                                    .get(&(id.clone(), send.destination_id.clone(), sidechain))
                                    .copied()
                                    .unwrap_or(0) as usize,
                            );
                            Some(SendSlot {
                                dest_index: dest,
                                sidechain,
                                pre_fader: send.pre_fader.unwrap_or(false),
                                ratio: send.ratio as f32,
                                delay,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut master_delay = DelayLine::new(max_edge_delay, max_block);
            master_delay.set_delay(
                pdc.edge_delays
                    .get(&(id.clone(), MASTER_MIXER_CHANNEL_ID.to_string(), false))
                    .copied()
                    .unwrap_or(0) as usize,
            );
            index.insert(id.clone(), position);
            buses.push(Bus {
                id: id.clone(),
                is_master,
                inserts: insert_chains.remove(id).unwrap_or_default(),
                level,
                balance: spec.map(|s| s.balance as f32).unwrap_or(0.0),
                mute: spec.and_then(|s| s.mute).unwrap_or(false),
                solo: spec.and_then(|s| s.solo).unwrap_or(false),
                master_send_ratio: spec.and_then(|s| s.master_send_ratio).unwrap_or(1.0) as f32,
                sends,
                master_delay,
                sum_l: vec![0.0; max_block],
                sum_r: vec![0.0; max_block],
                sc_l: vec![0.0; max_block],
                sc_r: vec![0.0; max_block],
                work_l: vec![0.0; max_block],
                work_r: vec![0.0; max_block],
                out_l: vec![0.0; max_block],
                out_r: vec![0.0; max_block],
                meter: BusMeter::new(),
            });
        }

        Ok(Self {
            sample_rate,
            max_block,
            buses,
            index,
            respect_solo: false,
            pdc,
            true_peak: TruePeakMeter::new(max_block),
            stem_taps: None,
            stem_block: None,
        })
    }
}
