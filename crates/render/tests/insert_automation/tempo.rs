//! Probe the actual DSP-facing seconds values, including tempo changes.
use super::*;
use oxitone_core::wire::*;
use oxitone_graph::{
    ChannelLayout, HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance,
    PluginKind, ProcessContext,
};
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};
use std::sync::{Arc, OnceLock};

struct Probe;
struct Instance(f64);
impl Plugin for Probe {
    fn descriptor(&self) -> &'static PluginDescriptor {
        static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| PluginDescriptor {
            plugin_id: "test.seconds".into(),
            plugin_version: "1.0.0".into(),
            kind: PluginKind::Effect,
            input_layout: ChannelLayout::Stereo,
            output_layout: ChannelLayout::Stereo,
            parameters: [
                ("delay.timeBeats", ParameterUnit::Beats),
                ("delay.timeSeconds", ParameterUnit::Seconds),
                ("internal", ParameterUnit::Normalized),
            ]
            .into_iter()
            .map(|(id, unit)| ParameterSpec {
                id: id.into(),
                label: id.into(),
                unit,
                min: 0.,
                max: 4.,
                default: 1.,
                smoothing: ParameterSmoothing::None,
                rate: ParameterRate::Control,
                mapping: Some(ParameterMapping::Linear),
                automation: Some(id != "internal"),
            })
            .collect(),
            capabilities: PluginCapabilities::default(),
            state_schema: None,
            max_polyphony: None,
        })
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance(1.))
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, _: f64, _: u32) {}
    fn reset(&mut self) {}
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            assert_ne!(
                event.parameter_id, "delay.timeBeats",
                "host must convert beats"
            );
            if event.parameter_id == "delay.timeSeconds" {
                self.0 = event.value;
            }
        }
        for out in ctx.outputs.iter_mut() {
            out[..ctx.frames].fill(self.0 as f32);
        }
    }
}

fn probe_snapshot(owner: &str, parameters: &[(&str, f64)]) -> ProjectSnapshot {
    let mut s = snapshot(owner);
    let effect = effect_ref("test.seconds", parameters);
    if owner == "chn_a" {
        s.channels[0].effect_chain[0] = effect;
    } else {
        s.mixer_channels
            .iter_mut()
            .find(|b| b.id == owner)
            .unwrap()
            .inserts[0] = effect;
    }
    s.tempo_map.push(TempoSegment {
        start_beat: beat(2, 375),
        bpm: 240.,
        curve: None,
    }); // frame 128
    s
}

fn probe_graph(s: &ProjectSnapshot) -> RenderGraph {
    let mut registry = builtin_registry().unwrap();
    registry.register(Arc::new(Probe)).unwrap();
    RenderGraph::compile(
        s,
        &registry,
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap()
}

fn normalized(mut g: RenderGraph, baseline: f32) -> Vec<f32> {
    render(&mut g, 384)
        .into_iter()
        .map(|v| v / baseline)
        .collect()
}

#[test]
fn beat_automation_uses_effective_tempo_with_or_without_initial_beats() {
    for owner in OWNERS {
        let baseline = render(&mut probe_graph(&probe_snapshot(owner, &[])), 1)[0];
        for parameters in [vec![], vec![("delay.timeBeats", 0.25)]] {
            let mut s = probe_snapshot(owner, &parameters);
            s.automation
                .push(constant(owner, "insert.0.parameter.delay.timeBeats", 0.5)); // 2 beats
            let values = normalized(probe_graph(&s), baseline);
            for value in &values[..128] {
                assert!((*value - 1.).abs() < 1e-6);
            }
            for value in &values[128..] {
                assert!((*value - 0.5).abs() < 1e-6);
            }
            s.automation.clear();
            let mut host = probe_graph(&s);
            host.enqueue_parameter(&event(owner, "insert.0.parameter.delay.timeBeats", 2., 0))
                .unwrap();
            assert_eq!(normalized(host, baseline), values);
        }
    }
}

#[test]
fn explicit_seconds_disable_sync_and_beats_reenable_it_without_allocations() {
    for owner in OWNERS {
        let s = probe_snapshot(owner, &[("delay.timeBeats", 2.)]);
        let baseline = render(&mut probe_graph(&probe_snapshot(owner, &[])), 1)[0];
        let mut g = probe_graph(&s);
        g.enqueue_parameter(&event(
            owner,
            "insert.0.parameter.delay.timeSeconds",
            0.75,
            64,
        ))
        .unwrap();
        g.enqueue_parameter(&event(owner, "insert.0.parameter.delay.timeBeats", 1., 256))
            .unwrap();
        let mut result = [0.; 384];
        let mut r = [0.; 128];
        g.transport_mut().begin_render(0);
        assert_eq!(
            allocations::count(|| {
                for chunk in result.chunks_mut(128) {
                    g.process_block(chunk, &mut r);
                }
                g.seek(0);
            }),
            (0, 0)
        );
        for (frame, value) in result.iter().enumerate() {
            let expected = if frame < 64 {
                1.
            } else if frame < 256 {
                0.75
            } else {
                0.25
            };
            assert!(
                (value / baseline - expected).abs() < 1e-6,
                "{owner}, frame {frame}"
            );
        }
    }
}

#[test]
fn tempo_lane_is_authoritative_and_plugin_opt_out_is_respected() {
    for owner in OWNERS {
        let mut s = probe_snapshot(owner, &[("delay.timeBeats", 2.)]);
        let baseline = render(&mut probe_graph(&probe_snapshot(owner, &[])), 1)[0];
        s.automation.push(constant(
            &s.id,
            "tempo",
            (60_f64 / 20.).ln() / (999_f64 / 20.).ln(),
        ));
        for value in normalized(probe_graph(&s), baseline) {
            assert!((value - 2.).abs() < 1e-6);
        }
        let mut g = probe_graph(&s);
        assert_eq!(
            g.enqueue_parameter(&event(owner, "insert.0.parameter.internal", 1., 0))
                .unwrap_err()
                .code,
            "AutomationTargetInvalid"
        );
        s.automation.clear();
        s.automation
            .push(constant(owner, "insert.0.parameter.internal", 0.5));
        let mut registry = builtin_registry().unwrap();
        registry.register(Arc::new(Probe)).unwrap();
        assert_eq!(
            oxitone_graph::validate(&s, &registry).unwrap_err().code,
            "AutomationTargetInvalid"
        );
    }
}
