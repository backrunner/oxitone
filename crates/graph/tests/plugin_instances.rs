mod common;
use common::*;
use oxitone_core::{
    codes,
    wire::{AutomationLaneSpec, EffectRef, ProjectSnapshot},
};
use oxitone_graph::{
    compile_plan, validate, BindingTarget, CompileOptions, PlanSampleProvider, PluginRegistry,
};

struct NoSamples;
impl PlanSampleProvider for NoSamples {
    fn prepared_sample(
        &self,
        _: &oxitone_core::wire::SampleRef,
    ) -> Result<std::sync::Arc<oxitone_samples::PreparedSample>, oxitone_core::OxitoneError> {
        panic!("sample-free fixture")
    }
}
fn fixture() -> (ProjectSnapshot, PluginRegistry) {
    let mut registry = PluginRegistry::new();
    registry
        .register(mock_plugin(instrument_descriptor(
            SYNTH_ID,
            PLUGIN_VERSION,
            vec![param("level", 0., 4., 1.)],
        )))
        .unwrap();
    registry
        .register(mock_plugin(effect_descriptor(
            EFFECT_ID,
            PLUGIN_VERSION,
            vec![
                param("mix", 0., 2., 1.),
                param("filter.cutoff", 20., 20_000., 1000.),
            ],
        )))
        .unwrap();
    let mut snapshot = base_snapshot();
    snapshot.protocol_version = "1.1".into();
    snapshot.channels[0].instrument.instance_id = Some("ins_voice".into());
    snapshot.channels[0].effect_chain = ["ins_first", "ins_second"]
        .map(|id| EffectRef {
            instance_id: Some(id.into()),
            plugin_id: EFFECT_ID.into(),
            plugin_version: PLUGIN_VERSION.into(),
            parameters: Default::default(),
            resources: None,
            mix: None,
            bypass: None,
        })
        .into();
    (snapshot, registry)
}
fn lane(id: &str, instance: &str, scope: Option<&str>, parameter: &str) -> AutomationLaneSpec {
    let mut value = serde_json::json!({"id":id,"target":{"entityId":instance,"parameterId":parameter},"source":{"kind":"constant","value":0.5}});
    if let Some(scope) = scope {
        value["target"]["scope"] = scope.into();
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn plugin_and_host_names_are_distinct_and_literal_parameter_keys_survive() {
    let (mut s, registry) = fixture();
    s.automation = vec![
        lane("auto_voice", "ins_voice", Some("plugin"), "level"),
        lane("auto_channel", "chn_0001", None, "level"),
        lane("auto_fx", "ins_first", Some("plugin"), "mix"),
        lane("auto_host", "ins_first", Some("effectHost"), "mix"),
        lane("auto_cutoff", "ins_second", Some("plugin"), "filter.cutoff"),
    ];
    let plan = compile_plan(&s, &registry, &NoSamples, &CompileOptions::default()).unwrap();
    assert_eq!(plan.bindings.len(), 5);
    assert!(plan.bindings.iter().any(|b| b.target
        == BindingTarget::InstrumentParam {
            channel: 0,
            parameter_id: "level".into()
        }
        && b.spec.max == 4.));
    assert!(plan
        .bindings
        .iter()
        .any(|b| b.target == BindingTarget::ChannelLevel(0) && b.spec.max == 2.));
    for (parameter, max) in [
        ("insert.0.parameter.mix", 2.),
        ("insert.0.mix", 1.),
        ("insert.1.parameter.filter.cutoff", 20_000.),
    ] {
        assert!(plan.bindings.iter().any(|b| b.target
            == BindingTarget::EffectInsert {
                entity_id: "chn_0001".into(),
                parameter_id: parameter.into()
            }
            && b.spec.max == max));
    }
}

#[test]
fn typed_and_legacy_aliases_combine_once_and_require_explicit_rules() {
    let (mut s, registry) = fixture();
    s.automation = vec![
        lane("auto_a", "ins_first", Some("plugin"), "mix"),
        lane("auto_b", "chn_0001", None, "insert.0.parameter.mix"),
    ];
    assert_eq!(
        validate(&s, &registry).unwrap_err().path.as_deref(),
        Some("$.automation[0].combine")
    );
    s.automation[0].combine = Some(oxitone_core::wire::AutomationCombine::Replace);
    s.automation[1].combine = Some(oxitone_core::wire::AutomationCombine::Multiply);
    let plan = compile_plan(&s, &registry, &NoSamples, &CompileOptions::default()).unwrap();
    assert_eq!(plan.bindings.len(), 1);
    assert_eq!(plan.bindings[0].lanes.len(), 2);
    assert_eq!(
        oxitone_graph::compile::binding_value_at(&plan.bindings[0], 0., &Default::default()),
        0.25
    );
    assert_eq!(
        oxitone_graph::compile::map_normalized(&plan.bindings[0].spec, 0.25),
        0.5
    );
}

#[test]
fn reorder_resolves_same_instance_and_unsorted_channels_use_compiled_indices() {
    let (mut s, registry) = fixture();
    let mut earlier = s.channels[0].clone();
    earlier.id = "chn_0000".into();
    earlier.instrument.instance_id = Some("ins_earlier".into());
    earlier.effect_chain.clear();
    s.channels.push(earlier);
    s.tracks[0].channel_ids.push("chn_0000".into());
    s.channels[0].effect_chain.reverse();
    s.automation = vec![
        lane("auto_voice", "ins_voice", Some("plugin"), "level"),
        lane("auto_fx", "ins_first", Some("plugin"), "mix"),
    ];
    let plan = compile_plan(&s, &registry, &NoSamples, &CompileOptions::default()).unwrap();
    assert!(plan.bindings.iter().any(|b| b.target
        == BindingTarget::InstrumentParam {
            channel: 1,
            parameter_id: "level".into()
        }));
    assert!(plan.bindings.iter().any(|b| b.target
        == BindingTarget::EffectInsert {
            entity_id: "chn_0001".into(),
            parameter_id: "insert.1.parameter.mix".into()
        }));
}

#[test]
fn version_floor_duplicates_and_dangling_instances_fail_with_paths() {
    let (mut s, registry) = fixture();
    s.protocol_version = "1.0".into();
    assert_eq!(
        validate(&s, &registry).unwrap_err().code,
        codes::PROTOCOL_VERSION_UNSUPPORTED
    );
    assert_eq!(
        oxitone_core::wire::decode_project_snapshot(&serde_json::to_string(&s).unwrap())
            .unwrap_err()
            .code,
        codes::PROTOCOL_VERSION_UNSUPPORTED
    );
    s.protocol_version = "1.1".into();
    s.channels[0].effect_chain[0].instance_id = Some(s.id.clone());
    assert_eq!(
        validate(&s, &registry).unwrap_err().code,
        codes::INVALID_PROJECT
    );
    s.channels[0].effect_chain[0].instance_id = Some("ins_first".into());
    for (instance, scope, parameter) in [
        ("ins_missing", "plugin", "mix"),
        ("ins_voice", "effectHost", "mix"),
        ("ins_first", "effectHost", "filter.cutoff"),
        ("ins_first", "plugin", "absent"),
    ] {
        s.automation = vec![lane("auto_invalid", instance, Some(scope), parameter)];
        let error = validate(&s, &registry).unwrap_err();
        assert_eq!(error.code, codes::AUTOMATION_TARGET_INVALID);
        assert_eq!(error.path.as_deref(), Some("$.automation[0].target"));
    }
}
