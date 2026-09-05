mod common;

use oxitone_core::wire::{ParameterMapping, ParameterRate};
use oxitone_graph::builtin_params::*;
use oxitone_graph::descriptor::validate_parameter_specs;

fn ids(specs: &[oxitone_core::wire::ParameterSpec]) -> Vec<&str> {
    specs.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn builtin_sets_match_the_contract_full_list() {
    // 04-api-contracts.md 内建图节点 stable parameter ID 全集.
    assert_eq!(ids(&project_parameters()), ["tempo"]);
    assert_eq!(
        ids(&channel_parameters()),
        ["level", "pan", "mute", "swing"]
    );
    assert_eq!(
        ids(&mixer_channel_parameters()),
        ["level", "balance", "mute", "masterSendRatio"]
    );
    assert_eq!(
        ids(&sample_clip_parameters()),
        ["level", "tone", "gain", "pan", "rate"]
    );
    assert_eq!(ids(&effect_insert_parameters()), ["mix", "bypass"]);
}

#[test]
fn every_builtin_spec_is_valid_and_automatable() {
    for kind in [
        BuiltinEntityKind::Project,
        BuiltinEntityKind::Channel,
        BuiltinEntityKind::MixerChannel,
        BuiltinEntityKind::SampleClip,
        BuiltinEntityKind::EffectInsert,
    ] {
        let specs = parameters_for(kind);
        validate_parameter_specs(&specs).unwrap();
        assert!(specs.iter().all(|s| s.automation == Some(true)));
        assert!(specs.iter().all(|s| s.rate == ParameterRate::Control));
    }
    validate_parameter_specs(&[send_ratio_parameter("mix_0002")]).unwrap();
}

#[test]
fn tempo_uses_log_mapping_over_bpm_range() {
    let tempo = &project_parameters()[0];
    assert_eq!(tempo.mapping, Some(ParameterMapping::Log));
    assert_eq!((tempo.min, tempo.max), (20.0, 999.0));
    assert_eq!(tempo.default, 120.0);
}

#[test]
fn documented_ranges_match() {
    let channel = channel_parameters();
    assert_eq!((channel[0].min, channel[0].max), (0.0, 2.0)); // level
    assert_eq!((channel[1].min, channel[1].max), (-1.0, 1.0)); // pan
    assert_eq!((channel[3].min, channel[3].max), (0.0, 1.0)); // swing

    let mixer = mixer_channel_parameters();
    assert_eq!((mixer[1].min, mixer[1].max), (-1.0, 1.0)); // balance
    assert_eq!((mixer[3].min, mixer[3].max), (0.0, 1.0)); // masterSendRatio

    let clip = sample_clip_parameters();
    assert_eq!(clip[4].mapping, Some(ParameterMapping::Log)); // rate
    assert_eq!((clip[4].min, clip[4].max), (0.25, 4.0));

    let mix = &effect_insert_parameters()[0];
    assert_eq!((mix.min, mix.max, mix.default), (0.0, 1.0, 1.0));
}

#[test]
fn mute_and_bypass_are_step_parameters() {
    for spec in [
        channel_parameters()[2].clone(),
        effect_insert_parameters()[1].clone(),
    ] {
        assert_eq!(spec.mapping, Some(ParameterMapping::Enum));
        assert_eq!((spec.min, spec.max), (0.0, 1.0));
    }
}

#[test]
fn send_ratio_parameter_round_trips() {
    let id = send_ratio_parameter_id("mix_0002");
    assert_eq!(id, "send.mix_0002.ratio");
    assert_eq!(parse_send_ratio_parameter(&id), Some("mix_0002"));
    assert_eq!(parse_send_ratio_parameter("send..ratio"), None);
    assert_eq!(parse_send_ratio_parameter("send.mix_0002.level"), None);
    assert_eq!(parse_send_ratio_parameter("level"), None);

    let found = find_parameter(BuiltinEntityKind::MixerChannel, &id).unwrap();
    assert_eq!((found.min, found.max, found.default), (0.0, 1.0, 1.0));
    assert!(find_parameter(BuiltinEntityKind::Channel, &id).is_none());
}

#[test]
fn structured_sample_fields_are_listed() {
    for field in [
        "startFrame",
        "endFrame",
        "normalize",
        "fadeIn",
        "fadeOut",
        "crossfade",
    ] {
        assert!(NON_AUTOMATABLE_SAMPLE_FIELDS.contains(&field));
    }
}
