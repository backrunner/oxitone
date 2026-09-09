use crate::{
    engine::Engine,
    plugin_details::{resolve, DetailTarget},
    wire,
};
use oxitone_core::wire::{EffectRef, ProjectSnapshot};
use serde_json::json;

fn accept(engine: &mut Engine, mut snapshot: ProjectSnapshot) {
    snapshot.revision = engine.current.as_ref().unwrap().snapshot.revision + 1;
    let revision = snapshot.revision;
    let frame = wire::decode(
        json!({"protocolVersion":"1.0", "type":"snapshot", "snapshot":snapshot,
        "assetBaseDir":"/tmp", "hash":"ab".repeat(32)}),
    )
    .unwrap();
    let result = engine.handle(frame);
    assert_eq!(result["revision"], revision.to_string(), "{result}");
}
fn delay(mix: f64) -> EffectRef {
    serde_json::from_value(
        json!({"pluginId":"oxitone.delay", "pluginVersion":"1.0.0", "parameters":{}, "mix":mix}),
    )
    .unwrap()
}

#[test]
fn panel_identity_follows_reordering_and_detaches_on_replacement() {
    let mut engine = crate::tests::engine();
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.protocol_version = "1.2".into();
    let mut first = delay(0.4);
    first.instance_id = Some("ins_first".into());
    let mut second = delay(0.8);
    second.instance_id = Some("ins_second".into());
    snapshot.channels[0].effect_chain = vec![first, second];
    accept(&mut engine, snapshot);
    let p = engine.current.as_ref().unwrap();
    let target = DetailTarget::ChannelInsert(p.snapshot.channels[0].id.clone(), 0);
    let identity = crate::plugin_identity::key(p, &target);
    let mut snapshot = p.snapshot.clone();
    snapshot.channels[0].effect_chain.swap(0, 1);
    accept(&mut engine, snapshot);
    let p = engine.current.as_ref().unwrap();
    let current = crate::plugin_identity::follow(p, &target, &identity).unwrap();
    assert_eq!(current.slot(), Some(1));
    assert_eq!(resolve(p, &current).unwrap().source["mix"], 0.4);
    let mut snapshot = p.snapshot.clone();
    snapshot.channels[0].effect_chain[1].instance_id = Some("ins_replacement".into());
    accept(&mut engine, snapshot);
    assert!(
        crate::plugin_identity::follow(engine.current.as_ref().unwrap(), &target, &identity)
            .is_none()
    );
}

#[test]
fn instrument_details_merge_source_defaults_and_scoped_automation() {
    let mut engine = crate::tests::engine();
    let target = DetailTarget::Instrument("chn_keys".into());
    let initial = resolve(engine.current.as_ref().unwrap(), &target).unwrap();
    assert!(!initial.parameters.is_empty());
    assert!(initial
        .parameters
        .iter()
        .all(|p| !p.explicit && p.value == p.spec.default));
    let spec = initial
        .parameters
        .iter()
        .find(|p| p.spec.automation == Some(true))
        .unwrap()
        .spec
        .clone();
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.channels[0]
        .instrument
        .parameters
        .insert(spec.id.clone(), spec.max);
    snapshot.automation.push(
        serde_json::from_value(
            json!({"id":"auto_details", "target":{"entityId":"chn_keys", "parameterId":spec.id},
        "source":{"kind":"constant", "value":0.3}}),
        )
        .unwrap(),
    );
    accept(&mut engine, snapshot);
    let detail = resolve(engine.current.as_ref().unwrap(), &target).unwrap();
    let parameter = detail
        .parameters
        .iter()
        .find(|p| p.spec.id == spec.id)
        .unwrap();
    assert!(parameter.explicit);
    assert_eq!(
        parameter.value, spec.max,
        "show source, not the evaluated automation value"
    );
    assert_eq!(parameter.automation, ["auto_details"]);
    assert_eq!(parameter.fraction(), 1.);
    assert!(detail.info.library.is_none());
}

#[test]
fn repeated_effects_and_master_inserts_have_distinct_addresses_and_host_values() {
    let mut engine = crate::tests::engine();
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.channels[0].effect_chain = vec![delay(0.25), delay(0.75)];
    snapshot.channels[0].effect_chain[0].bypass = Some(true);
    snapshot.mixer_channels.push(
        serde_json::from_value(
            json!({"id":"mix_master", "name":"Main output", "level":1, "balance":0,
        "inserts":[delay(0.5)], "sends":[]}),
        )
        .unwrap(),
    );
    snapshot.automation.push(
        serde_json::from_value(
            json!({"id":"auto_wet", "target":{"entityId":"chn_keys", "parameterId":"insert.0.mix"},
        "source":{"kind":"constant", "value":0.1}}),
        )
        .unwrap(),
    );
    accept(&mut engine, snapshot);
    let project = engine.current.as_ref().unwrap();
    for (target, mix) in [
        (DetailTarget::ChannelInsert("chn_keys".into(), 0), 0.25),
        (DetailTarget::ChannelInsert("chn_keys".into(), 1), 0.75),
        (DetailTarget::BusInsert("mix_master".into(), 0), 0.5),
    ] {
        let detail = resolve(project, &target).unwrap();
        let wet = detail
            .parameters
            .iter()
            .find(|p| p.host && p.spec.id == "mix")
            .unwrap();
        assert_eq!(wet.value, mix);
        assert_eq!(wet.automation.len(), usize::from(mix == 0.25));
        assert!(detail.parameters.iter().any(|p| !p.host && !p.explicit));
        if mix == 0.25 {
            assert_eq!(
                detail
                    .parameters
                    .iter()
                    .find(|p| p.host && p.spec.id == "bypass")
                    .unwrap()
                    .value,
                1.
            );
        }
    }
    assert!(resolve(project, &DetailTarget::Instrument("mix_master".into())).is_none());
}

#[test]
fn watch_reordering_removal_and_return_follow_the_addressed_slot() {
    let mut engine = crate::tests::engine();
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.channels[0].effect_chain = vec![delay(0.2), delay(0.9)];
    accept(&mut engine, snapshot.clone());
    let first = DetailTarget::ChannelInsert("chn_keys".into(), 0);
    let second = DetailTarget::ChannelInsert("chn_keys".into(), 1);
    snapshot.channels[0].effect_chain.remove(0);
    accept(&mut engine, snapshot.clone());
    assert_eq!(
        resolve(engine.current.as_ref().unwrap(), &first)
            .unwrap()
            .source["mix"],
        0.9
    );
    assert!(resolve(engine.current.as_ref().unwrap(), &second).is_none());
    snapshot.channels[0].effect_chain.clear();
    accept(&mut engine, snapshot.clone());
    assert!(resolve(engine.current.as_ref().unwrap(), &first).is_none());
    snapshot.channels[0].effect_chain.push(delay(0.6));
    accept(&mut engine, snapshot);
    assert_eq!(
        resolve(engine.current.as_ref().unwrap(), &first)
            .unwrap()
            .source["mix"],
        0.6
    );
}

#[test]
fn rejected_watch_keeps_the_last_accepted_detail_and_descriptor() {
    let mut engine = crate::tests::engine();
    let old = engine.current.as_ref().unwrap().clone();
    let mut snapshot = old.snapshot.clone();
    snapshot.revision += 1;
    snapshot.channels[0].instrument.plugin_id = "missing.plugin".into();
    let frame = wire::decode(
        json!({"protocolVersion":"1.0", "type":"snapshot", "snapshot":snapshot,
        "assetBaseDir":"/tmp", "hash":"ab".repeat(32)}),
    )
    .unwrap();
    assert_eq!(engine.handle(frame)["type"], "rejected");
    assert!(std::sync::Arc::ptr_eq(
        &old,
        engine.current.as_ref().unwrap()
    ));
    assert_eq!(
        resolve(&old, &DetailTarget::Instrument("chn_keys".into()))
            .unwrap()
            .info
            .descriptor
            .plugin_id,
        "oxitone.wavetable"
    );
}
