#[path = "common/allocations.rs"]
mod allocations;
mod common;
use common::*;
use oxitone_render::{RenderGraph, SampleStore};

#[test]
fn mapped_voices_stealing_and_seek_are_allocation_free() {
    let dir = out_dir("multisampler");
    let signal: Vec<_> = (0..48000)
        .map(|i| (std::f64::consts::TAU * 440. * i as f64 / 48000.).sin() as f32 * 0.1)
        .collect();
    let mut s = base_snapshot();
    s.samples
        .push(sample_asset(&dir, "smp_bank", &signal, (2, 1)));
    let mut instrument = wavetable_ref(&[("amp.attack", 0.), ("loop", 1.)]);
    instrument.plugin_id = "oxitone.multisampler".into();
    instrument.resources = Some(std::collections::BTreeMap::from([(
        "a".into(),
        "smp_bank".into(),
    )]));
    instrument.state = Some(serde_json::json!({"version":1,"regions":[
        {"resource":"a","rootKey":60,"keyRange":[0,127],"velocityRange":[1,64],"gain":0.7},
        {"resource":"a","rootKey":72,"keyRange":[0,127],"velocityRange":[65,127]}
    ]}));
    s.channels
        .push(channel("chn_bank", "mix_master", instrument, vec![]));
    s.tracks
        .push(track("trk_bank", &["chn_bank"], &["pcl_bank"], &[]));
    s.patterns.push(pattern(
        "pat_bank",
        (4, 1),
        (0..40)
            .map(|i| {
                note(
                    48 + i,
                    (i as i64, 128),
                    (2, 1),
                    if i % 2 == 0 { 0.4 } else { 0.9 },
                )
            })
            .collect(),
    ));
    s.pattern_clips.push(pattern_clip(
        "pcl_bank",
        "pat_bank",
        "trk_bank",
        (0, 1),
        (4, 1),
    ));
    let registry = oxitone_render::builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let mut graph = RenderGraph::compile(&s, &registry, &store, &Default::default()).unwrap();
    graph.transport_mut().begin_render(0);
    let (mut l, mut r) = ([0.; 128], [0.; 128]);
    let mut peak = 0f32;
    assert_eq!(
        allocations::count(|| {
            for _ in 0..3 {
                graph.seek(0);
                for _ in 0..160 {
                    graph.process_block(&mut l, &mut r);
                    for value in l {
                        peak = peak.max(value.abs());
                    }
                }
            }
        }),
        (0, 0)
    );
    assert!(peak > 0.01 && peak.is_finite());
    s.channels[0]
        .instrument
        .resources
        .as_mut()
        .unwrap()
        .insert("a".into(), "smp_missing".into());
    assert_eq!(
        oxitone_graph::validate(&s, &registry).unwrap_err().code,
        oxitone_core::codes::INVALID_PROJECT
    );
    s.channels[0].instrument.state = None;
    assert!(oxitone_graph::validate(&s, &registry).is_err());
}
