#[path = "common/allocations.rs"]
mod allocations;
mod common;
use common::*;
use oxitone_core::wire::{AutomationLaneSpec, ProjectSnapshot};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

fn lane(id: &str, instance: &str, scope: &str, parameter: &str, value: f64) -> AutomationLaneSpec {
    serde_json::from_value(serde_json::json!({"id":id,"target":{"entityId":instance,"scope":scope,"parameterId":parameter},"source":{"kind":"constant","value":value}})).unwrap()
}
fn fixture() -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.protocol_version = "1.1".into();
    s.tracks.push(track("trk_a", &["chn_a"], &["pcl_a"], &[]));
    s.patterns.push(pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (4, 1), 0.9)],
    ));
    s.pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (4, 1)));
    let mut instrument = wavetable_ref(&[]);
    instrument.instance_id = Some("ins_voice".into());
    let mut first = effect_ref("oxitone.utility", &[("gainDb", -6.)]);
    first.instance_id = Some("ins_first".into());
    let mut second = effect_ref("oxitone.utility", &[("polarity", 1.)]);
    second.instance_id = Some("ins_second".into());
    s.channels
        .push(channel("chn_a", "mix_bus", instrument, vec![first, second]));
    s.mixer_channels
        .push(mixer_channel("mix_bus", vec![], vec![]));
    s
}
fn render(s: &ProjectSnapshot, block: u32) -> Vec<f32> {
    let mut graph = RenderGraph::compile(
        s,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &RenderGraphOptions {
            compile: oxitone_graph::CompileOptions {
                block_size: Some(block),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    graph.transport_mut().begin_render(0);
    let mut left = vec![0.; block as usize];
    let mut right = left.clone();
    let mut output = vec![0.; 8192];
    let counts = allocations::count(|| {
        for chunk in output.chunks_mut(block as usize) {
            graph.process_block(&mut left, &mut right);
            chunk.copy_from_slice(&left[..chunk.len()]);
        }
    });
    assert_eq!(counts, (0, 0));
    assert!(!graph.faulted());
    output
}

#[test]
fn typed_parameters_render_offline_without_allocations_and_survive_chain_reordering() {
    let mut s = fixture();
    let baseline = render(&s, 128);
    assert!(baseline.iter().any(|v| v.abs() > 0.001));
    s.automation = vec![
        lane("auto_bypass", "ins_first", "effectHost", "bypass", 1.),
        lane("auto_polarity", "ins_second", "plugin", "polarity", 0.),
    ];
    let actual = render(&s, 128);
    let mut expected = s.clone();
    expected.automation.clear();
    expected.channels[0].effect_chain[0].bypass = Some(true);
    expected.channels[0].effect_chain[1]
        .parameters
        .insert("polarity".into(), 0.);
    assert_eq!(actual, render(&expected, 128));
    assert_ne!(actual, baseline);
    s.channels[0].effect_chain.reverse();
    for block in [64, 128, 256] {
        assert_eq!(render(&s, block), actual);
    }
    // Instruments use their own descriptor even when a Channel has a host parameter of the same name.
    s.automation
        .push(lane("auto_voice", "ins_voice", "plugin", "level", 0.));
    let muted = render(&s, 128);
    // The declared smoothing ramps the initial value down; it is not an instantaneous mute.
    let residual = muted[7680..]
        .iter()
        .fold(0_f32, |max, sample| max.max(sample.abs()));
    assert!(
        residual < 1e-6,
        "instrument level did not settle to zero: {residual}"
    );
    assert_eq!(s.channels[0].level, 1.);
}
