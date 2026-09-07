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
