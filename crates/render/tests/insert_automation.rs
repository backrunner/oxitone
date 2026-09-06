#[path = "common/allocations.rs"]
mod allocations;
mod common;
#[path = "insert_automation/tempo.rs"]
mod tempo;

use common::*;
use oxitone_core::wire::{AutomationLaneSpec, ProjectSnapshot};
use oxitone_render::{
    builtin_registry, resolve_parameter_event, ParamTargetIndex, ParameterEventInput, RenderGraph,
    RenderGraphOptions, SampleStore,
};

const OWNERS: [&str; 3] = ["chn_a", "mix_bus", "mix_master"];

#[test]
fn automated_bypass_preserves_pdc_for_latent_inserts() {
    for owner in OWNERS {
        let mut s = snapshot(owner);
        let mut effect = effect_ref("oxitone.clipper", &[]);
        effect.bypass = Some(true);
        if owner == "chn_a" {
            s.channels[0].effect_chain[0] = effect;
        } else {
            s.mixer_channels
                .iter_mut()
                .find(|b| b.id == owner)
                .unwrap()
                .inserts[0] = effect;
        }
        let mut reference = graph(&s);
        let latency = reference.graph_latency_frames();
        assert!(latency > 0);
        let expected = render(&mut reference, 1024);
        if owner == "chn_a" {
            s.channels[0].effect_chain[0].bypass = Some(false);
        } else {
            s.mixer_channels
                .iter_mut()
                .find(|b| b.id == owner)
                .unwrap()
                .inserts[0]
                .bypass = Some(false);
        }
        s.automation.push(constant(owner, "insert.0.bypass", 1.));
        let mut automated = graph(&s);
        assert_eq!(automated.graph_latency_frames(), latency);
        assert_eq!(render(&mut automated, 1024), expected);
    }
}

fn snapshot(owner: &str) -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.tracks.push(track("trk_a", &["chn_a"], &["pcl_a"], &[]));
    s.patterns.push(pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (4, 1), 0.9)],
    ));
    s.pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (4, 1)));
    s.channels
        .push(channel("chn_a", "mix_bus", wavetable_ref(&[]), vec![]));
    s.mixer_channels
        .push(mixer_channel("mix_bus", vec![], vec![]));
    s.mixer_channels
        .push(mixer_channel("mix_master", vec![], vec![]));
    let effect = effect_ref("oxitone.utility", &[("polarity", 1.0)]);
    if owner == "chn_a" {
        s.channels[0].effect_chain.push(effect);
    } else {
        s.mixer_channels
            .iter_mut()
            .find(|b| b.id == owner)
            .unwrap()
            .inserts
            .push(effect);
    }
    s
}

fn lane(owner: &str, parameter: &str, source: serde_json::Value) -> AutomationLaneSpec {
    serde_json::from_value(serde_json::json!({
        "id": "auto_insert", "target": {"entityId": owner, "parameterId": parameter}, "source": source
    })).unwrap()
}

fn constant(owner: &str, parameter: &str, value: f64) -> AutomationLaneSpec {
    lane(
        owner,
        parameter,
        serde_json::json!({"kind":"constant", "value":value}),
    )
}

fn event(owner: &str, parameter: &str, value: f64, frame: u64) -> ParameterEventInput {
    ParameterEventInput {
        entity_id: owner.into(),
        parameter_id: parameter.into(),
        value,
        at_frame: Some(frame),
    }
}

fn graph(s: &ProjectSnapshot) -> RenderGraph {
    RenderGraph::compile(
        s,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap()
}

fn render(g: &mut RenderGraph, frames: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(frames);
    let mut l = vec![0.; g.block_size()];
    let mut r = l.clone();
    g.transport_mut().begin_render(0);
    while result.len() < frames {
        g.process_block(&mut l, &mut r);
        result.extend_from_slice(&l[..l.len().min(frames - result.len())]);
    }
    result
}

#[test]
fn plugin_automation_overrides_initial_and_host_values_on_all_owners() {
    for owner in OWNERS {
        let s = snapshot(owner);
        let reference = render(&mut graph(&s), 256);
        assert!(reference.iter().any(|v| v.abs() > 0.001));
        let mut automated = s.clone();
        automated
            .automation
            .push(constant(owner, "insert.0.parameter.polarity", 0.));
        let mut g = graph(&automated);
        g.enqueue_parameter(&event(owner, "insert.0.parameter.polarity", 1., 0))
            .unwrap();
        let output = render(&mut g, 256);
        for (a, b) in output.iter().zip(&reference) {
            assert_eq!(*a, -*b, "{owner}");
        }
        // Control-thread session index and offline enqueue have the same target.
        let mut g = graph(&s);
        let index = ParamTargetIndex::from_graph(&g);
        let resolved = resolve_parameter_event(
            &index,
            &event(owner, "insert.0.parameter.polarity", 0., 0),
            0,
        )
        .unwrap();
        g.insert_queued_parameter(resolved);
        assert_eq!(render(&mut g, 256), output);
    }
}

#[test]
fn insert_gate_and_host_event_change_at_the_exact_frame_across_block_sizes() {
    for owner in OWNERS {
        for block in [64, 128, 256] {
            let mut s = snapshot(owner);
            s.block_size = block;
            let reference = render(&mut graph(&s), 256);
            s.automation.push(lane(
                owner,
                "insert.0.parameter.polarity",
                serde_json::json!({
                    "kind":"gate", "periodBeats":{"numerator":1,"denominator":100},
                    "duty":0.5, "on":1., "off":0.
                }),
            ));
            let automated = render(&mut graph(&s), 256);
            s.automation.clear();
            let mut g = graph(&s);
            g.enqueue_parameter(&event(owner, "insert.0.parameter.polarity", 0., 120))
                .unwrap();
            g.enqueue_parameter(&event(owner, "insert.0.parameter.polarity", 1., 240))
                .unwrap();
            assert_eq!(render(&mut g, 256), automated);
            assert_eq!(&automated[..120], &reference[..120]);
            for n in 120..240 {
                assert_eq!(automated[n], -reference[n]);
            }
        }
    }
}

#[test]
fn mix_and_bypass_match_host_events_including_first_block() {
    for owner in OWNERS {
        for (parameter, value) in [("insert.0.mix", 0.), ("insert.0.bypass", 1.)] {
            let s = snapshot(owner);
            let mut automated = s.clone();
            automated.automation.push(constant(owner, parameter, value));
            let output = render(&mut graph(&automated), 16_384);
            let mut host = graph(&s);
            host.enqueue_parameter(&event(owner, parameter, value, 0))
                .unwrap();
            assert_eq!(render(&mut host, 16_384), output);
            let inverted = render(&mut graph(&s), 16_384);
            for n in 16_000..16_384 {
                assert!((output[n] + inverted[n]).abs() < 1e-6);
            }
        }
    }
}

#[test]
fn invalid_insert_paths_and_ranges_fail_before_render() {
    for owner in OWNERS {
        let s = snapshot(owner);
        for parameter in [
            "insert.1.mix",
            "insert.0.parameter.missing",
            "insert.0.parameter.",
            "insert.-1.mix",
            "insert.+0.mix",
            "insert.00.mix",
            "insert.999999999999999999999999.mix",
        ] {
            let mut invalid = s.clone();
            invalid.automation.push(constant(owner, parameter, 0.5));
            let error =
                oxitone_graph::validate(&invalid, &builtin_registry().unwrap()).unwrap_err();
            assert_eq!(error.code, "AutomationTargetInvalid");
            assert_eq!(error.path.as_deref(), Some("$.automation[0].target"));
            assert_eq!(
                graph(&s)
                    .enqueue_parameter(&event(owner, parameter, 0.5, 0))
                    .unwrap_err()
                    .code,
                error.code
            );
        }
        for value in [-1., 2., f64::NAN, f64::INFINITY] {
            assert_eq!(
                graph(&s)
                    .enqueue_parameter(&event(owner, "insert.0.parameter.polarity", value, 0))
                    .unwrap_err()
                    .code,
                "AutomationRange"
            );
        }
    }
}

#[test]
fn insert_first_block_events_and_seek_allocate_and_free_nothing() {
    for owner in OWNERS {
        let mut s = snapshot(owner);
        s.automation
            .push(constant(owner, "insert.0.parameter.polarity", 0.));
        let mut g = graph(&s);
        g.enqueue_parameter(&event(owner, "insert.0.mix", 0.5, 64))
            .unwrap();
        g.transport_mut().begin_render(0);
        let (mut l, mut r) = ([0.; 128], [0.; 128]);
        assert_eq!(
            allocations::count(|| {
                g.process_block(&mut l, &mut r);
                g.seek(0);
                g.process_block(&mut l, &mut r);
            }),
            (0, 0),
            "{owner}"
        );
    }
}

#[test]
fn send_ratio_host_target_and_master_validation_are_consistent() {
    let mut s = snapshot("chn_a");
    s.mixer_channels[0].sends.push(send("mix_fx", 1., false));
    s.mixer_channels
        .push(mixer_channel("mix_fx", vec![], vec![]));
    graph(&s)
        .enqueue_parameter(&event("mix_bus", "send.mix_fx.ratio", 0., 0))
        .unwrap();
    s.automation
        .push(constant("mix_master", "masterSendRatio", 0.));
    assert_eq!(
        oxitone_graph::validate(&s, &builtin_registry().unwrap())
            .unwrap_err()
            .code,
        "AutomationTargetInvalid"
    );
}
