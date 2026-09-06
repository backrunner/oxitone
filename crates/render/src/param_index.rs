//! Immutable control-thread parameter metadata for realtime sessions.
use crate::graph::RenderGraph;
use oxitone_core::wire::ParameterSpec;
use std::collections::{BTreeMap, BTreeSet};

/// Everything parameter resolution needs from the graph, abstracted so the
/// realtime session can resolve against an immutable snapshot
/// ([`ParamTargetIndex`]) while the compiled graph lives on the render
/// worker thread.
pub(crate) trait ParamGraphView {
    fn effect_targets(&self) -> &crate::effect_targets::EffectTargetIndex;
    fn channel_index(&self, entity: &str) -> Option<usize>;
    fn channel_id(&self, channel: usize) -> &str;
    fn instrument_param_ids(&self, channel: usize) -> &[String];
    fn instrument_specs(&self, channel: usize) -> &[ParameterSpec];
    fn has_mixer_bus(&self, id: &str) -> bool;
    fn has_send_route(&self, route: &(String, String)) -> bool;
    fn clip_index(&self, entity: &str) -> Option<usize>;
}

/// Immutable snapshot of every resolvable parameter target, built at
/// compile/session start (control thread). Lets `setParameter` keep its
/// synchronous validation while the graph itself is owned by the realtime
/// worker.
pub struct ParamTargetIndex {
    effect_targets: crate::effect_targets::EffectTargetIndex,
    channels: BTreeMap<String, usize>,
    channel_ids: Vec<String>,
    instrument_param_ids: Vec<std::sync::Arc<Vec<String>>>,
    instrument_specs: Vec<std::sync::Arc<Vec<ParameterSpec>>>,
    mixer_buses: BTreeSet<String>,
    mixer_sends: BTreeSet<(String, String)>,
    clip_ids: BTreeMap<String, usize>,
}

impl ParamTargetIndex {
    pub fn from_graph(graph: &RenderGraph) -> Self {
        let mut channels = BTreeMap::new();
        let mut channel_ids = Vec::new();
        let mut instrument_param_ids = Vec::new();
        let mut instrument_specs = Vec::new();
        for (index, channel) in graph.channels.iter().enumerate() {
            channels.insert(channel.id.clone(), index);
            channel_ids.push(channel.id.clone());
            instrument_param_ids.push(channel.instrument_param_ids.clone());
            instrument_specs.push(channel.instrument_specs.clone());
        }
        Self {
            effect_targets: graph.effect_targets.clone(),
            channels,
            channel_ids,
            instrument_param_ids,
            instrument_specs,
            mixer_buses: graph.mixer.bus_order().into_iter().collect(),
            mixer_sends: graph.mixer_sends.clone(),
            clip_ids: graph
                .plan
                .sample_clips
                .iter()
                .enumerate()
                .map(|(index, clip)| (clip.id.clone(), index))
                .collect(),
        }
    }
}

impl ParamGraphView for ParamTargetIndex {
    fn effect_targets(&self) -> &crate::effect_targets::EffectTargetIndex {
        &self.effect_targets
    }
    fn channel_index(&self, entity: &str) -> Option<usize> {
        self.channels.get(entity).copied()
    }

    fn channel_id(&self, channel: usize) -> &str {
        &self.channel_ids[channel]
    }

    fn instrument_param_ids(&self, channel: usize) -> &[String] {
        &self.instrument_param_ids[channel]
    }

    fn instrument_specs(&self, channel: usize) -> &[ParameterSpec] {
        &self.instrument_specs[channel]
    }

    fn has_mixer_bus(&self, id: &str) -> bool {
        self.mixer_buses.contains(id)
    }

    fn has_send_route(&self, route: &(String, String)) -> bool {
        self.mixer_sends.contains(route)
    }

    fn clip_index(&self, entity: &str) -> Option<usize> {
        self.clip_ids.get(entity).copied()
    }
}

impl ParamGraphView for RenderGraph {
    fn effect_targets(&self) -> &crate::effect_targets::EffectTargetIndex {
        &self.effect_targets
    }
    fn channel_index(&self, entity: &str) -> Option<usize> {
        self.channel_index.get(entity).copied()
    }

    fn channel_id(&self, channel: usize) -> &str {
        &self.channels[channel].id
    }

    fn instrument_param_ids(&self, channel: usize) -> &[String] {
        &self.channels[channel].instrument_param_ids
    }

    fn instrument_specs(&self, channel: usize) -> &[ParameterSpec] {
        &self.channels[channel].instrument_specs
    }

    fn has_mixer_bus(&self, id: &str) -> bool {
        self.mixer.bus_order().iter().any(|bus| bus == id)
    }

    fn has_send_route(&self, route: &(String, String)) -> bool {
        self.mixer_sends.contains(route)
    }

    fn clip_index(&self, entity: &str) -> Option<usize> {
        self.plan
            .sample_clips
            .iter()
            .position(|clip| clip.id == entity)
    }
}
