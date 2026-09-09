use oxitone_core::wire::AutomationLaneSpec;
use serde_json::{json, Value};

fn lane(entity: &str, parameter: &str, scope: Value) -> AutomationLaneSpec {
    serde_json::from_value(json!({"id":"auto_test", "target":{
        "entityId":entity,"parameterId":parameter,"scope":scope},
        "source":{"kind":"constant","value":0.5}}))
    .unwrap()
}

#[test]
fn channel_and_instrument_targets_keep_their_distinct_namespaces() {
    let mut p = crate::tests::engine().current.unwrap();
    let p = std::sync::Arc::get_mut(&mut p).unwrap();
    p.snapshot.channels[0].name = Some("Keys".into());
    p.snapshot.channels[0].instrument.instance_id = Some("ins_keys".into());
    let channel = p.automation_target(&lane("chn_keys", "level", Value::Null));
    let instrument = p.automation_target(&lane("ins_keys", "level", json!("plugin")));
    assert_eq!(channel.owner, "Keys");
    assert_eq!(channel.parameter, "Volume");
    assert_eq!(channel.device, None);
    assert_eq!(instrument.device.as_deref(), Some("Wavetable"));
    assert_ne!(channel.key, instrument.key);
    assert_eq!(
        p.automation_label(&lane("chn_keys", "pan", Value::Null)),
        "Pan — Keys"
    );
    assert_eq!(
        p.automation_target(&lane("prj_view", "tempo", Value::Null))
            .parameter,
        "Tempo"
    );
}

#[test]
fn repeated_external_effects_use_descriptor_labels_and_follow_reordering() {
    let mut p = crate::tests::engine().current.unwrap();
    let p = std::sync::Arc::get_mut(&mut p).unwrap();
    let mut info = p.plugins.values().next().unwrap().clone();
    info.descriptor.plugin_id = "vendor.texture".into();
    let spec = &mut info.descriptor.parameters[0];
    spec.id = "grain.blurAmount".into();
    spec.label = "Grain diffusion".into();
    p.plugins
        .insert(("vendor.texture".into(), "1.0.0".into()), info);
    p.snapshot.channels[0].effect_chain = ["ins_first", "ins_second"].map(|id| {
        serde_json::from_value(json!({"pluginId":"vendor.texture", "pluginVersion":"1.0.0", "instanceId":id, "parameters":{}})).unwrap()
    }).into();
    let target = lane("ins_first", "grain.blurAmount", json!("plugin"));
    let before = p.automation_target(&target);
    assert_eq!(before.parameter, "Grain diffusion");
    assert_eq!(before.device.as_deref(), Some("1 Texture"));
    assert_eq!(
        before,
        p.automation_target(&lane(
            "chn_keys",
            "insert.0.parameter.grain.blurAmount",
            Value::Null
        ))
    );
    assert_ne!(
        before.key,
        p.automation_target(&lane("ins_second", "grain.blurAmount", json!("plugin")))
            .key
    );
    assert_eq!(
        p.automation_target(&lane("ins_first", "mix", json!("effectHost")))
            .parameter,
        "Dry / wet"
    );
    p.snapshot.channels[0].effect_chain.swap(0, 1);
    let after = p.automation_target(&target);
    assert_eq!(before.key, after.key);
    assert_eq!(after.device.as_deref(), Some("2 Texture"));
}

#[test]
fn bus_sends_and_unnamed_entities_have_musical_context() {
    let mut p = crate::tests::engine().current.unwrap();
    let p = std::sync::Arc::get_mut(&mut p).unwrap();
    p.snapshot.mixer_channels = serde_json::from_value(json!([
        {"id":"mix_music", "name":"Music", "level":1,"balance":0,"inserts":[],"sends":[]},
        {"id":"mix_reverb", "name":"Reverb", "level":1,"balance":0,"inserts":[],"sends":[]}
    ]))
    .unwrap();
    assert_eq!(
        p.automation_target(&lane("mix_music", "send.mix_reverb.ratio", Value::Null))
            .parameter,
        "Send to Reverb"
    );
    assert_eq!(
        p.automation_target(&lane("mix_music", "balance", Value::Null))
            .owner,
        "Music"
    );
    assert_eq!(
        p.automation_target(&lane("chn_keys", "level", Value::Null))
            .owner,
        "Channel 1"
    );
}

#[test]
fn external_fallback_uses_published_labels_instead_of_id_suffixes() {
    let mut descriptor = oxitone_instruments::wavetable::descriptor().clone();
    descriptor.plugin_id = "vendor.texture".into();
    descriptor.parameters.truncate(1);
    descriptor.parameters[0].id = "grainCloud.blurAmount".into();
    descriptor.parameters[0].label = "Grain diffusion".into();
    let details = crate::plugin_plot_tests::details(descriptor);
    let panel = crate::plugin_layout_builtin::panel(&details);
    let group = &panel.pages[0].groups[0];
    assert_eq!(group.title, "Grain Cloud");
    assert_eq!(group.controls[0].label(), Some("Grain diffusion"));
    assert_eq!(group.controls[0].bindings(), ["grainCloud.blurAmount"]);
}
