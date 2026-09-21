#[path = "insert_routing/allocations.rs"]
mod allocations;
#[path = "insert_routing/fixture.rs"]
mod fixture;
use fixture::*;
use oxitone_mixer::{ChannelInput, MixerEngine};

#[test]
fn aligns_direct_input_auxiliary_input_and_early_output_across_the_serial_chain() {
    let specs = routed();
    for source in ["owner", "source"] {
        let mut engine = MixerEngine::build(48000., BLOCK as u32, &specs, &registry()).unwrap();
        assert_eq!(engine.graph_latency_frames(), 21);
        let audio = render(&mut engine, source);
        let expected = if source == "owner" {
            1.
        } else {
            std::f32::consts::FRAC_1_SQRT_2
        };
        for (frame, sample) in audio.iter().enumerate() {
            assert!(
                (sample - if frame == 21 { expected } else { 0. }).abs() < 1e-6,
                "{source} frame {frame}: {sample}"
            );
        }
        engine.reset();
        assert_eq!(render(&mut engine, source), audio);
    }
}

#[test]
fn auxiliary_obeys_wet_mix_bypass_and_owner_fader_but_not_master_send_ratio() {
    for (mix, bypass, level, mute, expected) in [
        (1., false, 1., false, 0.5),
        (0.25, false, 1., false, 0.125),
        (1., true, 1., false, 0.),
        (1., false, 0.5, false, 0.25),
        (1., false, 1., true, 0.),
    ] {
        let mut specs = routed();
        let owner = &mut specs[0];
        owner.master_send_ratio = Some(0.);
        owner.inserts[1].mix = Some(mix);
        owner.inserts[1].bypass = Some(bypass);
        owner.level = level;
        owner.mute = Some(mute);
        let mut engine = MixerEngine::build(48000., BLOCK as u32, &specs, &registry()).unwrap();
        let audio = render(&mut engine, "owner");
        assert!((audio[21] - expected).abs() < 1e-6, "{audio:?}");
    }
}

#[test]
fn stems_include_auxiliary_master_routes_exactly_once() {
    let mut engine = MixerEngine::build(48000., BLOCK as u32, &routed(), &registry()).unwrap();
    engine.enable_stem_taps(true);
    let mut impulse = [0.; BLOCK];
    impulse[0] = 1.;
    let mut left = [0.; BLOCK];
    let mut right = [0.; BLOCK];
    let measured = allocations::count(|| {
        engine
            .process_with::<oxitone_graph::execution::Offline>(
                [ChannelInput {
                    bus_id: "owner",
                    left: &impulse,
                    right: &impulse,
                }]
                .into_iter(),
                &mut left,
                &mut right,
                &Default::default(),
            )
            .unwrap();
    });
    assert_eq!(
        measured,
        (0, 0),
        "prepared mixer routing allocates and frees nothing"
    );
    engine.capture_stem_segment(0, BLOCK);
    let stem = engine.stem_output("owner").unwrap().0;
    for (sample, tap) in left.iter().zip(stem) {
        assert!((sample - tap * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }
    assert!((left[21] - 1.).abs() < 1e-6);
}

#[test]
fn rejects_cycles_and_unavailable_physical_buses_before_processing() {
    let mut specs = routed();
    specs[0]
        .insert_routes
        .as_mut()
        .unwrap()
        .get_mut("routed")
        .unwrap()
        .outputs = Some([("1".into(), "source".into())].into());
    let err = MixerEngine::build(48000., BLOCK as u32, &specs, &registry())
        .err()
        .unwrap();
    assert!(err.message.contains("cycle"));
    let mut specs = routed();
    specs[0]
        .insert_routes
        .as_mut()
        .unwrap()
        .get_mut("routed")
        .unwrap()
        .inputs = Some([("3".into(), "source".into())].into());
    let err = MixerEngine::build(48000., BLOCK as u32, &specs, &registry())
        .err()
        .unwrap();
    assert_eq!(err.code, "PluginCapabilityUnsupported");
}

#[test]
fn broadcast_sidechain_catches_up_to_the_target_insert_position() {
    let mut specs = routed();
    specs[0]
        .insert_routes
        .as_mut()
        .unwrap()
        .get_mut("routed")
        .unwrap()
        .inputs = None;
    specs[1].sends.push(oxitone_core::wire::SendSpec {
        destination_id: "owner".into(),
        ratio: 1.,
        sidechain: Some(true),
        pre_fader: None,
    });
    let mut engine = MixerEngine::build(48000., BLOCK as u32, &specs, &registry()).unwrap();
    let audio = render(&mut engine, "source");
    for (frame, sample) in audio.iter().enumerate() {
        assert!(
            (sample - if frame == 21 { 1. } else { 0. }).abs() < 1e-6,
            "sidechain frame {frame}: {sample}"
        );
    }
}
