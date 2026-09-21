use super::*;
use crate::wire::Frame;

fn replace(engine: &mut Engine, revision: u64, level: f64) -> Value {
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.revision = revision;
    snapshot.channels[0].level = level;
    engine.handle(Frame::Snapshot {
        snapshot: Box::new(snapshot),
        asset_base_dir: "/tmp".into(),
        plugins: vec![],
        vst3_plugins: vec![],
        plugin_uis: Value::Null,
        allow_plugins: None,
        hash: "ab".repeat(32),
    })
}

#[test]
fn presentation_revisions_reject_late_control_results_even_when_the_graph_is_reused() {
    let mut engine = crate::tests::engine();
    let inventory = engine.vst3_instances("1".into());
    assert_eq!(inventory["inventory"]["state"], "active");
    let generation = inventory["inventory"]["graphGeneration"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(replace(&mut engine, 2, 1.)["type"], "state");
    assert_eq!(
        engine.vst3_instances("2".into())["inventory"]["graphGeneration"],
        generation
    );
    let late = ControlCompleted {
        snapshot_revision: "1".into(),
        graph_generation: generation.clone(),
        result: Ok(json!({})),
    };
    assert_eq!(engine.complete_vst3_control(late)["code"], "SourceChanged");
    assert_eq!(engine.vst3_instances("1".into())["code"], "SourceChanged");
    assert_eq!(replace(&mut engine, 3, 0.5)["type"], "state");
    assert_ne!(
        engine.vst3_instances("3".into())["inventory"]["graphGeneration"],
        generation
    );
    let late = ControlCompleted {
        snapshot_revision: "3".into(),
        graph_generation: generation,
        result: Ok(json!({})),
    };
    assert_eq!(
        engine.complete_vst3_control(late)["code"],
        "PluginTaskConflict"
    );
}

#[test]
fn malformed_instance_commands_fail_before_background_dispatch() {
    let engine = crate::tests::engine();
    let generation = engine.vst3_instances("1".into())["inventory"]["graphGeneration"].clone();
    let request = json!({"instanceControlVersion":2,"graphGeneration":generation,"instanceId":"missing","command":{"kind":"poll"}});
    assert_eq!(
        engine
            .prepare_vst3_control("1".into(), request, Instant::now())
            .err()
            .unwrap()
            .code,
        "ProtocolVersionUnsupported"
    );
}

#[test]
fn queued_and_scheduled_controls_expire_before_vendor_execution() {
    let engine = crate::tests::engine();
    let generation = engine.vst3_instances("1".into())["inventory"]["graphGeneration"].clone();
    let request = json!({"instanceControlVersion":1,"graphGeneration":generation,"instanceId":"missing","command":{"kind":"poll"},"timeoutMs":5000});
    let expired = Instant::now() - Duration::from_secs(6);
    assert_eq!(
        engine
            .prepare_vst3_control("1".into(), request.clone(), expired)
            .err()
            .unwrap()
            .code,
        "PluginHostTimeout"
    );
    let mut job = engine
        .prepare_vst3_control("1".into(), request, Instant::now())
        .unwrap();
    job.deadline = expired;
    // This must expire before even looking up the absent plugin instance.
    assert_eq!(job.run().result.unwrap_err().code, "PluginHostTimeout");
}
