use crate::{
    mixer_model,
    mixer_routes::{connections, RouteKind},
    tests, wire,
};
use serde_json::json;

fn routed() -> oxitone_core::wire::ProjectSnapshot {
    let mut snapshot = tests::engine().current.unwrap().snapshot.clone();
    snapshot.channels[0].mixer_channel_id = "mix_music".into();
    snapshot.mixer_channels = serde_json::from_value(json!([
        {"id":"mix_music","name":"Music","level":1,"balance":0,"masterSendRatio":0,
         "inserts":[],"sends":[
            {"destinationId":"mix_room","ratio":0.25,"preFader":true},
            {"destinationId":"mix_detector","ratio":0.6,"sidechain":true,"preFader":false}]},
        {"id":"mix_room","name":"Room","level":1,"balance":0,"inserts":[],"sends":[]},
        {"id":"mix_detector","name":"Detector","level":1,"balance":0,"inserts":[],"sends":[]}
    ]))
    .unwrap();
    snapshot.automation = serde_json::from_value(json!([
        {"id":"auto_send","target":{"entityId":"mix_music","parameterId":"send.mix_room.ratio"},
         "source":{"kind":"constant","value":0.1}}
    ]))
    .unwrap();
    snapshot
}
#[test]
fn route_semantics_preserve_zero_master_sends_and_detector_taps() {
    let s = routed();
    let routes = connections(&s);
    let direct = &routes[0];
    assert_eq!(direct.kind, RouteKind::Output);
    assert_eq!(direct.destination_name, "Music");
    let master = routes
        .iter()
        .find(|r| r.source == "mix_music" && r.kind == RouteKind::Master)
        .unwrap();
    assert_eq!(master.ratio, 0., "disabled Master route remains visible");
    let room = routes.iter().find(|r| r.destination == "mix_room").unwrap();
    assert_eq!(
        room.ratio, 0.25,
        "source ratio must not become automated playback value"
    );
    assert!(room.pre_fader && room.automated);
    let sc = routes
        .iter()
        .find(|r| r.kind == RouteKind::Sidechain)
        .unwrap();
    assert!(
        sc.pre_fader,
        "engine sidechain convention overrides source false"
    );
    assert_eq!(sc.tap(), "Detector · pre-fader");
    assert!(!routes.iter().any(|r| r.source == "mix_master"));
}

#[test]
fn midi_connections_name_the_source_instance_and_keep_the_main_audio_output() {
    let mut snapshot = routed();
    let mut destination = snapshot.channels[0].clone();
    destination.id = "chn_midi".into();
    destination.name = Some("Receiver".into());
    snapshot.channels.push(destination);
    snapshot.channels[0].instrument.instance_id = Some("ins_midi".into());
    snapshot.channels[0].effect_chain = serde_json::from_value(json!([
        {"instanceId":"fx_midi","pluginId":"fixture.midi","pluginVersion":"1.0.0","parameters":{}}
    ]))
    .unwrap();
    snapshot.channels[0].midi_routes = Some(
        serde_json::from_value(json!({"ins_midi":["chn_midi"],"fx_midi":["chn_midi"]})).unwrap(),
    );
    let routes = connections(&snapshot);
    assert_eq!(routes[0].kind, RouteKind::Output);
    let midi: Vec<_> = routes
        .iter()
        .filter(|route| matches!(route.kind, RouteKind::Midi(_)))
        .collect();
    assert_eq!(midi.len(), 2);
    assert!(midi
        .iter()
        .all(|r| r.destination_name == "Receiver" && r.is_output() && r.is_send()));
    assert!(midi
        .iter()
        .any(|r| r.label() == "Instrument MIDI → Instrument"));
    assert!(midi
        .iter()
        .any(|r| r.label() == "Insert 1 MIDI → Instrument"));
}
#[test]
fn auxiliary_connections_keep_physical_order_and_distinct_routes_to_the_same_bus() {
    let mut snapshot = routed();
    snapshot.channels[0].output_routes = Some(
        [
            ("10".into(), "mix_room".into()),
            ("2".into(), "mix_room".into()),
        ]
        .into(),
    );
    let routes = connections(&snapshot);
    assert_eq!(routes[1].kind, RouteKind::Auxiliary(2));
    assert_eq!(routes[2].kind, RouteKind::Auxiliary(10));
    assert_eq!(routes[1].destination, routes[2].destination);
    assert!(routes[1].is_output() && !routes[1].is_send());
    assert_eq!(routes[1].label(), "Output 2 · Before inserts, post-fader");
    assert_eq!(routes.iter().filter(|r| r.source == "chn_keys").count(), 3);
}

#[test]
fn insert_routes_project_both_directions_with_current_chain_positions() {
    let mut snapshot = routed();
    snapshot.mixer_channels[1].inserts = serde_json::from_value(json!([
        {"instanceId":"fx_room","pluginId":"fixture.multi","pluginVersion":"1.0.0","parameters":{}}
    ]))
    .unwrap();
    snapshot.mixer_channels[1].insert_routes = Some(
        serde_json::from_value(json!({
            "fx_room":{"inputs":{"2":"mix_music"},"outputs":{"10":"mix_master","1":"mix_detector"}}
        }))
        .unwrap(),
    );
    let routes = connections(&snapshot);
    let input = routes
        .iter()
        .find(|route| matches!(route.kind, RouteKind::InsertInput { .. }))
        .unwrap();
    assert_eq!(
        (&*input.source, &*input.destination),
        ("mix_music", "mix_room")
    );
    assert_eq!(input.label(), "Insert 1 input 2 · Post-fader");
    assert!(input.is_send() && input.is_output());
    let outputs: Vec<_> = routes
        .iter()
        .filter(|route| matches!(route.kind, RouteKind::InsertOutput { .. }))
        .collect();
    assert_eq!(
        outputs[0].kind,
        RouteKind::InsertOutput { insert: 0, bus: 1 }
    );
    assert_eq!(
        outputs[1].kind,
        RouteKind::InsertOutput { insert: 0, bus: 10 }
    );
    assert_eq!(outputs[1].destination, "mix_master");
    assert!(outputs[0].is_output() && !outputs[0].is_send());
}
#[test]
fn cached_routes_refresh_with_accepted_watch_and_keep_reverse_inputs() {
    let mut engine = tests::engine();
    let old = engine.current.clone().unwrap();
    assert_eq!(
        mixer_model::strips(&old)[0].outputs[0].destination,
        "mix_master"
    );
    let mut s = routed();
    s.revision = 2;
    let frame = wire::decode(
        json!({"protocolVersion":"1.0","type":"snapshot","snapshot":s,
        "assetBaseDir":"/tmp","hash":"cd".repeat(32)}),
    )
    .unwrap();
    assert_eq!(engine.handle(frame)["revision"], "2");
    let new = engine.current.as_ref().unwrap();
    let strips = mixer_model::strips(new);
    let music = strips.iter().find(|s| s.id == "mix_music").unwrap();
    assert_eq!(music.inputs[0].source, "chn_keys");
    assert_eq!(music.sends().count(), 2);
    assert_eq!(music.relation("mix_room"), Some("OUT"));
    assert_eq!(music.relation("chn_keys"), Some("IN"));
    assert_eq!(
        strips
            .iter()
            .find(|s| s.id == "mix_master")
            .unwrap()
            .inputs
            .len(),
        3
    );
    assert_eq!(
        mixer_model::strips(&old)[0].outputs[0].destination,
        "mix_master"
    );
    let accepted = new.clone();
    let mut rejected = accepted.snapshot.clone();
    rejected.revision = 3;
    rejected.mixer_channels[0].sends[0].destination_id = "mix_missing".into();
    let frame = wire::decode(
        json!({"protocolVersion":"1.0","type":"snapshot","snapshot":rejected,
        "assetBaseDir":"/tmp","hash":"ef".repeat(32)}),
    )
    .unwrap();
    assert_eq!(engine.handle(frame)["type"], "rejected");
    assert!(std::sync::Arc::ptr_eq(
        &accepted,
        engine.current.as_ref().unwrap()
    ));
}
