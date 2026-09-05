//! Shared fixtures for oxitone-graph integration tests.
#![allow(dead_code)]

use std::sync::Arc;

use oxitone_core::wire::{
    ChannelSpec, InstrumentRef, MixerChannelSpec, ParameterMapping, ParameterRate,
    ParameterSmoothing, ParameterSpec, ParameterUnit, ProjectSnapshot, TempoSegment,
    TimeSignatureSegment, TrackSpec,
};
use oxitone_core::Beat;
use oxitone_graph::{
    ChannelLayout, HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance,
    PluginKind, PluginRegistry, ProcessContext, StateSchemaId,
};

pub fn param(id: &str, min: f64, max: f64, default: f64) -> ParameterSpec {
    ParameterSpec {
        id: id.to_string(),
        label: id.to_string(),
        unit: ParameterUnit::Normalized,
        min,
        max,
        default,
        smoothing: ParameterSmoothing::Linear,
        rate: ParameterRate::Control,
        automation: Some(true),
        mapping: Some(ParameterMapping::Linear),
    }
}

pub fn instrument_descriptor(
    plugin_id: &'static str,
    plugin_version: &'static str,
    parameters: Vec<ParameterSpec>,
) -> PluginDescriptor {
    PluginDescriptor {
        plugin_id,
        plugin_version,
        kind: PluginKind::Instrument,
        input_layout: ChannelLayout::None,
        output_layout: ChannelLayout::Stereo,
        parameters,
        capabilities: PluginCapabilities::default(),
        state_schema: None,
        max_polyphony: Some(16),
    }
}

pub fn effect_descriptor(
    plugin_id: &'static str,
    plugin_version: &'static str,
    parameters: Vec<ParameterSpec>,
) -> PluginDescriptor {
    PluginDescriptor {
        plugin_id,
        plugin_version,
        kind: PluginKind::Effect,
        input_layout: ChannelLayout::Stereo,
        output_layout: ChannelLayout::Stereo,
        parameters,
        capabilities: PluginCapabilities::default(),
        state_schema: None,
        max_polyphony: None,
    }
}

struct MockPlugin(&'static PluginDescriptor);
struct MockInstance;

impl Plugin for MockPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        self.0
    }

    fn create(&self, _host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(MockInstance)
    }
}

impl PluginInstance for MockInstance {
    fn prepare(&mut self, _sample_rate: f64, _max_block_size: u32) {}
    fn process(&mut self, _ctx: &mut ProcessContext<'_>) {}
    fn reset(&mut self) {}
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}

pub fn mock_plugin(descriptor: PluginDescriptor) -> Arc<dyn Plugin> {
    Arc::new(MockPlugin(Box::leak(Box::new(descriptor))))
}

pub const SYNTH_ID: &str = "oxi.test.synth";
pub const EFFECT_ID: &str = "oxi.test.effect";
pub const PLUGIN_VERSION: &str = "1.0.0";

pub fn synth_descriptor() -> PluginDescriptor {
    instrument_descriptor(
        SYNTH_ID,
        PLUGIN_VERSION,
        vec![
            param("gainDb", -24.0, 24.0, 0.0),
            param("cutoff", 20.0, 20_000.0, 1_000.0),
            ParameterSpec {
                automation: None,
                ..param("internal", 0.0, 1.0, 0.5)
            },
        ],
    )
}

pub fn slicer_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        state_schema: Some(StateSchemaId("oxitone.slicer.slices@1")),
        ..instrument_descriptor(
            "oxitone.slicer",
            PLUGIN_VERSION,
            vec![param("level", 0.0, 2.0, 1.0)],
        )
    }
}

pub fn base_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(mock_plugin(synth_descriptor())).unwrap();
    registry
        .register(mock_plugin(effect_descriptor(
            EFFECT_ID,
            PLUGIN_VERSION,
            vec![param("mix2", 0.0, 1.0, 0.5)],
        )))
        .unwrap();
    registry
}

pub fn instrument_ref() -> InstrumentRef {
    InstrumentRef {
        plugin_id: SYNTH_ID.to_string(),
        plugin_version: PLUGIN_VERSION.to_string(),
        parameters: Default::default(),
        resources: None,
        state: None,
    }
}

pub fn beat(numerator: i64, denominator: u32) -> Beat {
    Beat::new(numerator, denominator).unwrap()
}

/// Minimal valid snapshot: one track/channel routed to one bus, 120 BPM 4/4.
pub fn base_snapshot() -> ProjectSnapshot {
    ProjectSnapshot {
        protocol_version: "1.0".to_string(),
        revision: 1,
        id: "prj_0001".to_string(),
        name: None,
        sample_rate: 48_000,
        block_size: 128,
        seed: 1,
        tempo_map: vec![TempoSegment {
            start_beat: beat(0, 1),
            bpm: 120.0,
            curve: None,
        }],
        time_signature_map: vec![TimeSignatureSegment {
            start_bar: 1,
            numerator: 4,
            denominator: 4,
        }],
        markers: vec![],
        tracks: vec![TrackSpec {
            id: "trk_0001".to_string(),
            name: None,
            channel_ids: vec!["chn_0001".to_string()],
            tempo: None,
            pattern_clip_ids: vec![],
            sample_clip_ids: vec![],
            enabled: None,
            midi_channel: None,
        }],
        patterns: vec![],
        pattern_clips: vec![],
        sample_clips: vec![],
        samples: vec![],
        channels: vec![ChannelSpec {
            id: "chn_0001".to_string(),
            name: None,
            instrument: instrument_ref(),
            effect_chain: vec![],
            level: 1.0,
            pan: 0.0,
            swing: None,
            mixer_channel_id: "mix_0001".to_string(),
            mute: None,
            solo: None,
        }],
        mixer_channels: vec![MixerChannelSpec {
            id: "mix_0001".to_string(),
            name: None,
            level: 1.0,
            balance: 0.0,
            master_send_ratio: None,
            inserts: vec![],
            sends: vec![],
            mute: None,
            solo: None,
        }],
        automation: vec![],
    }
}
