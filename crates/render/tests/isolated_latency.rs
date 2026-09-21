mod common;
use common::*;
use oxitone_core::wire::ProjectSnapshot;
use oxitone_graph::execution::ProcessPosition;
use oxitone_graph::{HostContext, Plugin, PluginDescriptor, PluginInstance, ProcessContext};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use std::sync::Arc;

const DELAY: usize = 17;
struct Delayed(PluginDescriptor);
impl Plugin for Delayed {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.0
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Delay {
            samples: [[0.; DELAY]; 2],
            cursor: 0,
        })
    }
}
struct Delay {
    samples: [[f32; DELAY]; 2],
    cursor: usize,
}
impl PluginInstance for Delay {
    fn requires_isolation(&self) -> bool {
        true
    }
    fn prepare(&mut self, _: f64, _: u32) {}
    fn process(&mut self, _: &mut ProcessContext<'_>) {
        panic!("external effect entered callback")
    }
    fn process_isolated(
        &mut self,
        ctx: &mut ProcessContext<'_>,
        _: &ProcessPosition,
    ) -> Result<(), oxitone_core::OxitoneError> {
        for i in 0..ctx.frames {
            for ch in 0..2 {
                ctx.outputs[ch][i] = self.samples[ch][self.cursor] * 0.5;
                self.samples[ch][self.cursor] = ctx.inputs[ch][i];
            }
            self.cursor = (self.cursor + 1) % DELAY;
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.samples = [[0.; DELAY]; 2];
        self.cursor = 0;
    }
    fn latency_frames(&self) -> u64 {
        DELAY as u64
    }
    fn tail_frames(&self) -> u64 {
        DELAY as u64
    }
}

fn snapshot() -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.tracks
        .push(track("trk_a", &["chn_a", "chn_b"], &["pcl_a"], &[]));
    s.patterns.push(pattern(
        "pat_a",
        (1, 1),
        vec![note(60, (0, 1), (1, 1), 0.2)],
    ));
    s.pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (1, 1)));
    for id in ["chn_a", "chn_b"] {
        s.channels
            .push(channel(id, "mix_bus", wavetable_ref(&[]), vec![]));
    }
    s.mixer_channels
        .push(mixer_channel("mix_bus", vec![], vec![]));
    s.mixer_channels
        .push(mixer_channel("mix_master", vec![], vec![]));
    s
}
fn render(s: &ProjectSnapshot) -> (Vec<f32>, u64) {
    let mut registry = builtin_registry().unwrap();
    let mut descriptor = registry
        .lookup_descriptor("oxitone.utility", "1.0.0")
        .unwrap()
        .clone();
    descriptor.plugin_id = "fixture.delayed".into();
    descriptor.parameters.clear();
    registry.register(Arc::new(Delayed(descriptor))).unwrap();
    let mut graph = RenderGraph::compile(
        s,
        &registry,
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap();
    graph.transport_mut().begin_render(0);
    let mut output = vec![];
    // Alternate segment lengths so delay compensation crosses arbitrary boundaries.
    for frames in [37, 91, 17, 128, 83, 28, 128, 51, 77, 128] {
        let mut l = vec![0.; frames];
        let mut r = l.clone();
        graph.process_offline_block(&mut l, &mut r).unwrap();
        output.extend(l);
    }
    (output, graph.graph_latency_frames())
}

#[test]
fn external_intrinsic_latency_aligns_parallel_channels_serial_inserts_and_dry_wet() {
    let s = snapshot();
    let reference = render(&s);
    assert!(reference.0.iter().any(|v| v.abs() > 0.001));
    for owner in ["chn_a", "mix_bus", "mix_master"] {
        for (count, mix, bypass) in [
            (1, 1.0, false),
            (1, 0.25, false),
            (1, 1.0, true),
            (2, 1.0, false),
        ] {
            let mut modified = s.clone();
            let mut effect = effect_ref("fixture.delayed", &[]);
            effect.mix = Some(mix);
            effect.bypass = Some(bypass);
            let inserts = vec![effect; count];
            if owner == "chn_a" {
                modified.channels[0].effect_chain = inserts;
            } else {
                modified
                    .mixer_channels
                    .iter_mut()
                    .find(|b| b.id == owner)
                    .unwrap()
                    .inserts = inserts;
            }
            let (actual, latency) = render(&modified);
            let delay = DELAY * count;
            assert_eq!(latency, reference.1 + delay as u64);
            assert!(actual[..delay].iter().all(|v| *v == 0.));
            let processed_gain = if bypass {
                1.
            } else {
                (1. - mix * 0.5).powi(count as i32)
            };
            // One of two equal channels is processed; the other must be delayed by PDC.
            let gain = if owner == "chn_a" {
                (1. + processed_gain) * 0.5
            } else {
                processed_gain
            } as f32;
            for (frame, (actual, reference)) in actual[delay..].iter().zip(&reference.0).enumerate()
            {
                assert!(
                    (actual - reference * gain).abs() < 1e-6,
                    "{owner} count={count} mix={mix} bypass={bypass} frame={frame}"
                );
            }
        }
    }
}
