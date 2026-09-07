mod common;

use common::*;
use oxitone_core::codes;
use oxitone_core::wire::{
    AutomationCombine, AutomationLaneSpec, AutomationPoint, AutomationSourceSpec, AutomationTarget,
    ChanceSpec, CurveKind, MixerChannelSpec, ProjectSnapshot, SampleClipSpec, SampleFormat,
    SampleRef, SendSpec, WaveKind,
};
use oxitone_graph::validate;

fn lane_to(entity: &str, parameter: &str) -> AutomationLaneSpec {
    AutomationLaneSpec {
        id: "auto_0001".to_string(),
        target: AutomationTarget {
            entity_id: entity.to_string(),
            parameter_id: parameter.to_string(),
        },
        source: AutomationSourceSpec::Constant { value: 0.5 },
        combine: None,
        loop_spec: None,
        last_beat: None,
    }
}

fn err(snapshot: &ProjectSnapshot) -> oxitone_core::OxitoneError {
    validate(snapshot, &base_registry()).unwrap_err()
}

fn add_sample(snapshot: &mut ProjectSnapshot) {
    snapshot.samples.push(SampleRef {
        id: "smp_0001".to_string(),
        asset_uri: "assets/a.wav".to_string(),
        sha256: "ab".repeat(32),
        format: SampleFormat::Wav,
        sample_rate: 48_000,
        channels: 2,
        frames: 192_000,
        edits: None,
        musical_length_beats: None,
        provenance: None,
    });
    snapshot.sample_clips.push(SampleClipSpec {
        id: "scl_0001".to_string(),
        sample_id: "smp_0001".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: Some(beat(8, 1)),
        gain: None,
        pan: None,
        rate: None,
        loop_spec: None,
        tempo_sync: None,
        stretch_algorithm: None,
        enabled: None,
    });
}

#[test]
fn builtin_targets_resolve_for_every_entity_kind() {
    let mut s = base_snapshot();
    add_sample(&mut s);
    s.mixer_channels.push(MixerChannelSpec {
        id: "mix_0002".to_string(),
        ..s.mixer_channels[0].clone()
    });
    s.mixer_channels[0].sends.push(SendSpec {
        destination_id: "mix_0002".to_string(),
        ratio: 0.5,
        pre_fader: None,
        sidechain: None,
    });
    for (i, (entity, parameter)) in [
        ("prj_0001", "tempo"),
        ("chn_0001", "level"),
        ("chn_0001", "cutoff"), // instrument plugin parameter
        ("mix_0001", "masterSendRatio"),
        ("mix_0001", "send.mix_0002.ratio"),
        ("scl_0001", "rate"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut lane = lane_to(entity, parameter);
        lane.id = format!("auto_{i:04}");
        s.automation.push(lane);
    }
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn unknown_entities_and_parameters_are_invalid_targets() {
    let mut s = base_snapshot();
    s.automation.push(lane_to("chn_missing", "level"));
    let e = err(&s);
    assert_eq!(e.code, codes::AUTOMATION_TARGET_INVALID);
    assert_eq!(e.path.as_deref(), Some("$.automation[0].target"));

    let mut s = base_snapshot();
    s.automation.push(lane_to("chn_0001", "notAParam"));
    assert_eq!(err(&s).code, codes::AUTOMATION_TARGET_INVALID);

    let mut s = base_snapshot();
    s.automation
        .push(lane_to("mix_0001", "send.mix_0002.ratio"));
    assert_eq!(err(&s).code, codes::AUTOMATION_TARGET_INVALID);

    let mut s = base_snapshot();
    s.automation.push(lane_to("prj_0001", "volume"));
    assert_eq!(err(&s).code, codes::AUTOMATION_TARGET_INVALID);
}

#[test]
fn parameters_without_automation_true_are_rejected() {
    let mut s = base_snapshot();
    s.automation.push(lane_to("chn_0001", "internal")); // automation: None on the spec
    let e = err(&s);
    assert_eq!(e.code, codes::AUTOMATION_TARGET_INVALID);
    assert!(e.message.contains("automation: true"), "{}", e.message);
}

#[test]
fn sample_structured_edit_fields_are_not_automatable() {
    let mut s = base_snapshot();
    add_sample(&mut s);
    for (i, field) in ["startFrame", "endFrame", "normalize", "fadeIn"]
        .into_iter()
        .enumerate()
    {
        let mut lane = lane_to("smp_0001", field);
        lane.id = format!("auto_{i:04}");
        s.automation.push(lane);
    }
    let e = err(&s);
    assert_eq!(e.code, codes::AUTOMATION_TARGET_INVALID);
    assert!(e.message.contains("structured edit"), "{}", e.message);

    let mut s = base_snapshot();
    add_sample(&mut s);
    s.automation.push(lane_to("smp_0001", "level"));
    assert_eq!(err(&s).code, codes::AUTOMATION_TARGET_INVALID);
}

fn chance_source() -> AutomationSourceSpec {
    AutomationSourceSpec::Chance(ChanceSpec {
        probability: 0.5,
        seed: 7,
        smooth_beats: None,
        random_phase: None,
        rate: Some(2.0),
        interval_beats: None,
    })
}

#[test]
fn at_most_one_tempo_lane() {
    let mut s = base_snapshot();
    s.automation.push(lane_to("prj_0001", "tempo"));
    let mut second = lane_to("prj_0001", "tempo");
    second.id = "auto_0002".to_string();
    second.combine = Some(AutomationCombine::Add);
    s.automation.push(second);
    let e = err(&s);
    assert_eq!(e.code, codes::TEMPO_AUTOMATION_CONFLICT);
    assert_eq!(e.path.as_deref(), Some("$.automation[1]"));
}

#[test]
fn tempo_lane_sources_must_be_transport_invariant() {
    let mut s = base_snapshot();
    let mut lane = lane_to("prj_0001", "tempo");
    lane.source = chance_source();
    s.automation.push(lane);
    let e = err(&s);
    assert_eq!(e.code, codes::AUTOMATION_TEMPO_RESTRICTION);

    // chance nested inside a combination is still banned
    let mut s = base_snapshot();
    let mut lane = lane_to("prj_0001", "tempo");
    lane.source = AutomationSourceSpec::Binary {
        op: oxitone_core::wire::BinaryOp::Add,
        left: Box::new(AutomationSourceSpec::Constant { value: 0.2 }),
        right: Box::new(chance_source()),
        amount: None,
    };
    s.automation.push(lane);
    assert_eq!(err(&s).code, codes::AUTOMATION_TEMPO_RESTRICTION);

    // curve/wave sources are fine on tempo lanes
    let mut s = base_snapshot();
    let mut lane = lane_to("prj_0001", "tempo");
    lane.source = AutomationSourceSpec::Wave {
        wave: WaveKind::Sine,
        period_beats: beat(8, 1),
        phase: None,
        min: Some(0.2),
        max: Some(0.9),
        pulse_width: None,
    };
    s.automation.push(lane);
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn multiple_lanes_on_one_target_require_combine() {
    let mut s = base_snapshot();
    s.automation.push(lane_to("chn_0001", "level"));
    let mut second = lane_to("chn_0001", "level");
    second.id = "auto_0002".to_string();
    s.automation.push(second);
    let e = err(&s);
    assert_eq!(e.code, codes::AUTOMATION_TARGET_INVALID);
    assert!(e.message.contains("combine"), "{}", e.message);

    let mut s = base_snapshot();
    let mut first = lane_to("chn_0001", "level");
    first.combine = Some(AutomationCombine::Replace);
    s.automation.push(first);
    let mut second = lane_to("chn_0001", "level");
    second.id = "auto_0002".to_string();
    second.combine = Some(AutomationCombine::Multiply);
    s.automation.push(second);
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn source_structural_errors_carry_the_lane_path() {
    let mut s = base_snapshot();
    let mut lane = lane_to("chn_0001", "level");
    lane.source = AutomationSourceSpec::Chance(ChanceSpec {
        probability: 0.5,
        seed: 7,
        smooth_beats: None,
        random_phase: None,
        rate: Some(2.0),
        interval_beats: Some(beat(1, 1)),
    });
    s.automation.push(lane);
    let e = err(&s);
    assert_eq!(e.code, codes::AUTOMATION_CHANCE_FREQUENCY);
    assert_eq!(e.path.as_deref(), Some("$.automation[0].source"));
}

#[test]
fn curve_sources_validate_as_targets_input() {
    let mut s = base_snapshot();
    let mut lane = lane_to("chn_0001", "pan");
    lane.source = AutomationSourceSpec::Curve {
        interpolation: CurveKind::Linear,
        points: vec![
            AutomationPoint {
                beat: beat(0, 1),
                value: 0.25,
                curve: None,
            },
            AutomationPoint {
                beat: beat(32, 1),
                value: 0.75,
                curve: None,
            },
        ],
    };
    s.automation.push(lane);
    assert!(validate(&s, &base_registry()).is_ok());
}
