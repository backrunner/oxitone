mod common;
use common::*;
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

#[test]
fn impulse_resources_work_on_channels_buses_and_master_with_pdc() {
    let directory = out_dir("convolver");
    let mut base = base_snapshot();
    base.samples
        .push(sample_asset(&directory, "smp_ir", &[0.5], (1, 1)));
    base.channels.push(channel(
        "chn_s",
        "mix_bus",
        wavetable_ref(&[("level", 0.1)]),
        vec![],
    ));
    base.tracks
        .push(track("trk_s", &["chn_s"], &["pcl_s"], &[]));
    base.patterns.push(pattern(
        "pat_s",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 0.3)],
    ));
    base.pattern_clips
        .push(pattern_clip("pcl_s", "pat_s", "trk_s", (0, 1), (4, 1)));
    base.mixer_channels
        .push(mixer_channel("mix_bus", vec![], vec![]));
    base.mixer_channels
        .push(mixer_channel("mix_master", vec![], vec![]));
    let reference = render_frames(&base, 4096);
    for owner in ["chn_s", "mix_bus", "mix_master"] {
        let mut snapshot = base.clone();
        let mut effect = effect_ref("oxitone.convolver", &[]);
        effect.resources = Some([("impulse".into(), "smp_ir".into())].into());
        if owner == "chn_s" {
            snapshot.channels[0].effect_chain.push(effect);
        } else {
            snapshot
                .mixer_channels
                .iter_mut()
                .find(|b| b.id == owner)
                .unwrap()
                .inserts
                .push(effect);
        }
        let wet = render_frames(&snapshot, 4352);
        assert_eq!(wet.2, reference.2 + 256);
        for (a, b) in reference.0.iter().zip(&wet.0[256..]) {
            assert!((a * 0.5 - b).abs() < 2e-6, "{owner}");
        }
        if owner == "chn_s" {
            snapshot.channels[0].effect_chain[0].bypass = Some(true);
        } else {
            snapshot
                .mixer_channels
                .iter_mut()
                .find(|b| b.id == owner)
                .unwrap()
                .inserts[0]
                .bypass = Some(true);
        }
        let dry = render_frames(&snapshot, 4352);
        assert_eq!(dry.2, wet.2);
        for (a, b) in reference.0.iter().zip(&dry.0[256..]) {
            assert!((a - b).abs() < 1e-7);
        }
    }
}

#[test]
fn bad_impulse_binding_is_rejected_before_playback() {
    let mut snapshot = base_snapshot();
    let mut effect = effect_ref("oxitone.convolver", &[]);
    effect.resources = Some([("impulse".into(), "smp_missing".into())].into());
    snapshot
        .mixer_channels
        .push(mixer_channel("mix_master", vec![effect], vec![]));
    let result = RenderGraph::compile(
        &snapshot,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &RenderGraphOptions::default(),
    );
    let error = match result {
        Ok(_) => panic!("missing impulse accepted"),
        Err(e) => e,
    };
    assert_eq!(error.code, oxitone_core::error::codes::INVALID_PROJECT);
}
