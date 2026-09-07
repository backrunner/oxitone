use crate::{
    plugin_details::{resolve, DetailTarget},
    plugin_layout::Layout,
    plugin_layout_registry,
    plugin_layout_validation::validate,
    wire,
};
use serde_json::{json, Value};
use std::sync::Arc;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../schemas/fixtures/plugin-ui.json")).unwrap()
}
fn send(
    engine: &mut crate::engine::Engine,
    ui: Value,
    mutate: impl FnOnce(&mut oxitone_core::wire::ProjectSnapshot),
) -> Value {
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.revision += 1;
    mutate(&mut snapshot);
    engine.handle(
        wire::decode(
            json!({"protocolVersion":"1.0", "type":"snapshot", "snapshot":snapshot,
        "pluginUis":ui, "assetBaseDir":"/tmp", "hash":"ab".repeat(32)}),
        )
        .unwrap(),
    )
}

#[test]
fn typescript_layout_fixture_and_builtin_wavetable_bind_real_descriptor_parameters() {
    let engine = crate::tests::engine();
    let project = engine.current.unwrap();
    let detail = resolve(&project, &DetailTarget::Instrument("chn_keys".into())).unwrap();
    let layout: Layout = serde_json::from_value(fixture()).unwrap();
    validate(&layout, &detail.info.descriptor).unwrap();
    let builtin = crate::plugin_layout_builtin::panel(&detail);
    validate(&builtin, &detail.info.descriptor).unwrap();
    let ids: std::collections::HashSet<_> = builtin
        .pages
        .iter()
        .flat_map(|p| &p.groups)
        .flat_map(|g| &g.controls)
        .flat_map(|c| c.bindings())
        .collect();
    assert!(detail
        .info
        .descriptor
        .parameters
        .iter()
        .all(|p| ids.contains(p.id.as_str())));
}

#[test]
fn sub_and_optional_oscillator_bindings_reject_incompatible_parameter_units() {
    let descriptor = oxitone_instruments::wavetable::descriptor();
    for control in [
        json!({"kind":"subOscillator", "wave":"sub.wave", "octave":"pan", "level":"sub.level"}),
        json!({"kind":"oscillator", "wave":"oscA.wavetable", "morphTo":"oscA.morphTo", "position":"oscA.position",
            "phase":"oscA.phase", "unison":"oscA.unison", "detune":"oscA.detune", "spread":"oscA.spread", "bank":"filter.cutoff"}),
    ] {
        let mut candidate = fixture();
        candidate["pages"][0]["groups"][0]["controls"] = json!([control]);
        let layout: Layout = serde_json::from_value(candidate).unwrap();
        assert!(validate(&layout, descriptor).is_err());
    }
}

#[test]
fn layout_only_refresh_reuses_graph_telemetry_and_preserves_transport() {
    let mut engine = crate::tests::engine();
    let old = engine.current.as_ref().unwrap().clone();
    engine.handle(wire::decode(json!({"protocolVersion":"1.0", "type":"transport", "command":{"command":"seek", "frame":"48000"}})).unwrap());
    assert_eq!(
        send(&mut engine, json!([fixture()]), |_| {})["revision"],
        "2"
    );
    let next = engine.current.as_ref().unwrap();
    assert!(!Arc::ptr_eq(&old, next));
    assert!(Arc::ptr_eq(&old.telemetry, &next.telemetry));
    assert_eq!(engine.state()["cursor"], "48000");
    engine.handle(
        wire::decode(
            json!({"protocolVersion":"1.0", "type":"transport", "command":{"command":"play"}}),
        )
        .unwrap(),
    );
    let mut updated = fixture();
    updated["title"] = json!("New panel");
    assert_eq!(send(&mut engine, json!([updated]), |_| {})["revision"], "3");
    assert!(Arc::ptr_eq(
        &old.telemetry,
        &engine.current.as_ref().unwrap().telemetry
    ));
    assert_eq!(
        engine
            .current
            .as_ref()
            .unwrap()
            .panels
            .layouts
            .values()
            .next()
            .unwrap()
            .title,
        "New panel"
    );
}

#[test]
fn invalid_ui_retains_compatible_layout_while_valid_music_and_mix_advance() {
    let mut engine = crate::tests::engine();
    send(&mut engine, json!([fixture()]), |_| {});
    let old = engine.current.as_ref().unwrap().clone();
    let mut invalid = fixture();
    invalid["pages"][0]["groups"][0]["controls"][0]["parameter"] = json!("unknown");
    assert_eq!(
        send(&mut engine, json!([invalid]), |snapshot| {
            snapshot.channels[0].effect_chain.push(serde_json::from_value(json!({"pluginId":"oxitone.delay", "pluginVersion":"1.0.0", "parameters":{}, "mix":0.27})).unwrap());
        })["revision"],
        "3"
    );
    let new = engine.current.as_ref().unwrap();
    assert!(!Arc::ptr_eq(&old.telemetry, &new.telemetry));
    assert!(Arc::ptr_eq(
        old.panels.layouts.values().next().unwrap(),
        new.panels.layouts.values().next().unwrap()
    ));
    assert_eq!(new.panels.errors.len(), 1);
    let effect = resolve(new, &DetailTarget::ChannelInsert("chn_keys".into(), 0)).unwrap();
    assert_eq!(
        effect
            .parameters
            .iter()
            .find(|p| p.host && p.spec.id == "mix")
            .unwrap()
            .value,
        0.27
    );
    send(&mut engine, json!([fixture()]), |_| {});
    assert!(engine.current.as_ref().unwrap().panels.errors.is_empty());
    send(&mut engine, json!([]), |_| {});
    assert!(
        engine.current.as_ref().unwrap().panels.layouts.is_empty(),
        "removing registration restores built-in panel"
    );
}

#[test]
fn untrusted_hash_cannot_skip_audio_validation_or_replace_last_accepted_ui() {
    let mut engine = crate::tests::engine();
    send(&mut engine, json!([fixture()]), |_| {});
    let old = engine.current.as_ref().unwrap().clone();
    assert_eq!(
        send(&mut engine, json!([]), |snapshot| snapshot.channels[0]
            .instrument
            .plugin_id =
            "missing.plugin".into())["type"],
        "rejected"
    );
    assert!(Arc::ptr_eq(&old, engine.current.as_ref().unwrap()));
}

#[test]
fn malformed_panels_never_reject_valid_music_and_have_bounded_fallback() {
    let engine = crate::tests::engine();
    let catalog = &engine.current.as_ref().unwrap().plugins;
    let mut cases = vec![json!({}), json!([{}]), json!(vec![fixture(); 65])];
    for (field, value) in [
        ("uiVersion", json!("2.0")),
        ("pages", json!([])),
        ("size", json!({"width":1,"height":2})),
        ("script", json!("execute")),
    ] {
        let mut invalid = fixture();
        invalid[field] = value;
        cases.push(json!([invalid]));
    }
    for control in [
        json!({"kind":"nativeView", "parameter":"pan"}),
        json!({"kind":"toggle", "parameter":"pan"}),
        json!({"kind":"choice", "parameter":"voiceMode", "options":[{"value":0,"label":"A"},{"value":0,"label":"B"}]}),
        json!({"kind":"envelope", "attack":"pan", "decay":"amp.decay", "sustain":"amp.sustain", "release":"amp.release"}),
        json!({"kind":"filterResponse", "mode":"filter.type", "cutoff":"pan", "resonance":"filter.resonance"}),
        json!({"kind":"lfoCurve", "shape":"oscA.wavetable", "rate":"lfo.rateHz", "phase":"lfo.phase"}),
        json!({"kind":"oscillator", "wave":"oscA.wavetable", "morphTo":"oscA.morphTo", "position":"pan",
            "phase":"oscA.phase", "unison":"oscA.unison", "detune":"oscA.detune", "spread":"oscA.spread"}),
        json!({"kind":"modulation", "routes":[]}),
        json!({"kind":"modulation", "routes":[{"label":"Pitch", "amount":"missing"}]}),
    ] {
        let mut invalid = fixture();
        invalid["pages"][0]["groups"][0]["controls"] = json!([control]);
        cases.push(json!([invalid]));
    }
    let mut duplicate = fixture();
    duplicate["pages"] = json!([duplicate["pages"][0], duplicate["pages"][0]]);
    cases.push(json!([duplicate]));
    let mut over_budget = fixture();
    over_budget["pages"][0]["groups"] = json!((0..16)
        .map(|i| json!({"id":format!("g{i}"),"title":"Group","columns":4,
        "controls":vec![json!({"kind":"knob","parameter":"pan"});32]}))
        .collect::<Vec<_>>());
    cases.push(json!([over_budget]));
    for raw in cases {
        let panels = plugin_layout_registry::resolve(&raw, catalog, None);
        assert!(panels.layouts.is_empty(), "{raw}");
        assert!(panels.global_error.is_some() || !panels.errors.is_empty());
    }
}

#[test]
fn logarithmic_dial_uses_descriptor_mapping() {
    let engine = crate::tests::engine();
    let mut detail = resolve(
        engine.current.as_ref().unwrap(),
        &DetailTarget::Instrument("chn_keys".into()),
    )
    .unwrap();
    let cutoff = detail
        .parameters
        .iter_mut()
        .find(|p| p.spec.id == "filter.cutoff")
        .unwrap();
    cutoff.value = (cutoff.spec.min * cutoff.spec.max).sqrt();
    assert!((cutoff.fraction() - 0.5).abs() < 1e-6);
}

#[test]
#[ignore = "focused release benchmark; run with --ignored --nocapture"]
fn benchmark_layout_validation() {
    let engine = crate::tests::engine();
    let catalog = &engine.current.as_ref().unwrap().plugins;
    for count in [8, 256] {
        let mut value = fixture();
        value["pages"][0]["groups"] = json!((0..count / 8)
            .map(|i| json!({"id":format!("g{i}"),"title":"Group","columns":4,
            "controls":vec![json!({"kind":"knob","parameter":"pan"});8]}))
            .collect::<Vec<_>>());
        // 256 controls use two pages, within the 16-group/page budget.
        if count == 256 {
            let groups = value["pages"][0]["groups"].as_array().unwrap().clone();
            value["pages"][0]["groups"] = json!(&groups[..16]);
            let mut second = value["pages"][0].clone();
            second["id"] = json!("second");
            value["pages"].as_array_mut().unwrap().push(second);
        }
        let raw = json!([value]);
        let mut times = Vec::new();
        for i in 0..1100 {
            let start = std::time::Instant::now();
            let result = std::hint::black_box(plugin_layout_registry::resolve(
                std::hint::black_box(&raw),
                catalog,
                None,
            ));
            assert_eq!(result.layouts.len(), 1);
            if i >= 100 {
                times.push(start.elapsed().as_secs_f64() * 1e6);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "Plugin layout benchmark {}",
            json!({"controls":count,"iterations":times.len(),"medianUs":times[500],"p95Us":times[950],"p99Us":times[990],"scope":"control-thread parse/validate; excludes GPUI paint and audio"})
        );
    }
}
