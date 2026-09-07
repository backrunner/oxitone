#[path = "common/allocations.rs"]
mod allocations;
mod common;
use common::*;
use oxitone_graph::{HostContext, Plugin, PluginDescriptor, PluginInstance, ProcessContext};
use oxitone_render::{ParameterEventInput, RenderGraph, SampleStore};
use std::sync::Arc;

struct ManyParameters(PluginDescriptor);
impl Plugin for ManyParameters {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.0
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instrument)
    }
}
struct Instrument;
impl PluginInstance for Instrument {
    fn prepare(&mut self, _: f64, _: u32) {}
    fn reset(&mut self) {}
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        assert_eq!(ctx.parameter_events.len(), 128);
        let mut seen = [false; 128];
        for event in ctx.parameter_events {
            let index: usize = event.parameter_id[1..].parse().unwrap();
            assert!(!seen[index]);
            seen[index] = true;
            assert_eq!(event.value, if index == 127 { 1. } else { 0.5 });
        }
        assert!(seen.iter().all(|v| *v));
        for output in ctx.outputs.iter_mut() {
            output[..ctx.frames].fill(0.25);
        }
    }
}

#[test]
fn large_instrument_keeps_all_initial_parameters_and_host_then_automation_priority() {
    let mut registry = oxitone_render::builtin_registry().unwrap();
    let mut descriptor = oxitone_instruments::wavetable::descriptor().clone();
    descriptor.plugin_id = "test.many".into();
    let template = descriptor.parameters.last().unwrap().clone();
    descriptor.parameters = (0..128)
        .map(|i| {
            let mut spec = template.clone();
            spec.id = format!("p{i}");
            spec.min = 0.;
            spec.max = 1.;
            spec.default = 0.;
            spec.mapping = Some(oxitone_core::wire::ParameterMapping::Linear);
            spec
        })
        .collect();
    registry
        .register(Arc::new(ManyParameters(descriptor)))
        .unwrap();
    let mut s = base_snapshot();
    let mut instrument = wavetable_ref(&[]);
    instrument.plugin_id = "test.many".into();
    instrument.parameters = (0..128).map(|i| (format!("p{i}"), 0.5)).collect();
    s.channels
        .push(channel("chn_many", "mix_master", instrument, vec![]));
    s.automation.push(
        serde_json::from_value(serde_json::json!({
            "id": "auto_last", "target": {"entityId": "chn_many", "parameterId": "p127"},
            "source": {"kind": "constant", "value": 1.}, "combine": "replace"
        }))
        .unwrap(),
    );
    let mut g =
        RenderGraph::compile(&s, &registry, &SampleStore::new(None), &Default::default()).unwrap();
    for value in [0.25, 0.75] {
        g.enqueue_parameter(&ParameterEventInput {
            entity_id: "chn_many".into(),
            parameter_id: "p127".into(),
            value,
            at_frame: None,
        })
        .unwrap();
    }
    g.transport_mut().begin_render(0);
    assert_eq!(
        allocations::count(|| g.process_block(&mut [0.; 128], &mut [0.; 128])),
        (0, 0)
    );
}
