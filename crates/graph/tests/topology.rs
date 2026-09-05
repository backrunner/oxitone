mod common;

use oxitone_core::codes;
use oxitone_core::wire::{MixerChannelSpec, SendSpec};
use oxitone_graph::{build_mixer_routing, MASTER_MIXER_CHANNEL_ID};

fn bus(id: &str, sends: Vec<SendSpec>) -> MixerChannelSpec {
    MixerChannelSpec {
        id: id.to_string(),
        name: None,
        level: 1.0,
        balance: 0.0,
        master_send_ratio: None,
        inserts: vec![],
        sends,
        mute: None,
        solo: None,
    }
}

fn send(destination: &str, sidechain: bool) -> SendSpec {
    SendSpec {
        destination_id: destination.to_string(),
        ratio: 0.5,
        pre_fader: None,
        sidechain: Some(sidechain),
    }
}

fn order(channels: &[MixerChannelSpec]) -> Vec<String> {
    build_mixer_routing(channels).unwrap().order
}

#[test]
fn empty_and_single_bus_mixers() {
    assert_eq!(order(&[]), [MASTER_MIXER_CHANNEL_ID]);
    assert_eq!(
        order(&[bus("mix_0001", vec![])]),
        ["mix_0001", MASTER_MIXER_CHANNEL_ID]
    );
}

#[test]
fn chains_and_diamonds_process_sources_first() {
    let channels = vec![
        bus("mix_a", vec![send("mix_b", false)]),
        bus("mix_b", vec![send("mix_c", false)]),
        bus("mix_c", vec![]),
    ];
    assert_eq!(
        order(&channels),
        ["mix_a", "mix_b", "mix_c", MASTER_MIXER_CHANNEL_ID]
    );

    let diamond = vec![
        bus("mix_a", vec![send("mix_b", false), send("mix_c", false)]),
        bus("mix_b", vec![send("mix_d", false)]),
        bus("mix_c", vec![send("mix_d", false)]),
        bus("mix_d", vec![]),
    ];
    assert_eq!(
        order(&diamond),
        ["mix_a", "mix_b", "mix_c", "mix_d", MASTER_MIXER_CHANNEL_ID]
    );
}

#[test]
fn ordering_is_deterministic_regardless_of_declaration_order() {
    let mut channels = vec![
        bus("mix_c", vec![send("mix_d", false)]),
        bus("mix_a", vec![send("mix_b", false), send("mix_c", false)]),
        bus("mix_d", vec![]),
        bus("mix_b", vec![send("mix_d", false)]),
    ];
    let first = build_mixer_routing(&channels).unwrap();
    let second = build_mixer_routing(&channels).unwrap();
    assert_eq!(first, second, "same input must yield the same routing");

    channels.reverse();
    let shuffled = build_mixer_routing(&channels).unwrap();
    assert_eq!(
        first, shuffled,
        "declaration order must not affect the result"
    );
}

#[test]
fn cycle_reports_the_full_path() {
    let channels = vec![
        bus("mix_a", vec![send("mix_b", false)]),
        bus("mix_b", vec![send("mix_c", false)]),
        bus("mix_c", vec![send("mix_a", false)]),
    ];
    let err = build_mixer_routing(&channels).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);
    assert_eq!(err.path.as_deref(), Some("$.mixerChannels"));
    assert!(
        err.message.contains("mix_a -> mix_b -> mix_c -> mix_a"),
        "{}",
        err.message
    );
}

#[test]
fn self_send_is_a_cycle() {
    let channels = vec![bus("mix_a", vec![send("mix_a", false)])];
    let err = build_mixer_routing(&channels).unwrap_err();
    assert!(err.message.contains("mix_a -> mix_a"), "{}", err.message);
}

#[test]
fn sidechain_detector_edges_join_the_topology() {
    // A sidechain-only send still closes a feedback loop.
    let channels = vec![
        bus("mix_a", vec![send("mix_b", false)]),
        bus("mix_b", vec![send("mix_a", true)]),
    ];
    let err = build_mixer_routing(&channels).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);
    assert!(err.message.contains("cycle"), "{}", err.message);

    // One-directional sidechain routing is fine and ordered.
    let channels = vec![
        bus("mix_a", vec![send("mix_b", true)]),
        bus("mix_b", vec![]),
    ];
    let routing = build_mixer_routing(&channels).unwrap();
    assert_eq!(routing.order, ["mix_a", "mix_b", MASTER_MIXER_CHANNEL_ID]);
    let detector = routing
        .edges
        .iter()
        .find(|e| e.destination == "mix_b")
        .unwrap();
    assert!(detector.sidechain);
}

#[test]
fn master_edges_are_listed_but_never_cycle() {
    let mut no_master_send = bus("mix_a", vec![]);
    no_master_send.master_send_ratio = Some(0.0);
    let routing = build_mixer_routing(&[no_master_send]).unwrap();
    assert!(routing
        .edges
        .iter()
        .all(|e| e.destination != MASTER_MIXER_CHANNEL_ID));
    assert_eq!(routing.order, ["mix_a", MASTER_MIXER_CHANNEL_ID]);

    let routing = build_mixer_routing(&[bus("mix_a", vec![])]).unwrap();
    let master_edge = routing
        .edges
        .iter()
        .find(|e| e.destination == MASTER_MIXER_CHANNEL_ID)
        .unwrap();
    assert_eq!(master_edge.source, "mix_a");
    assert!(!master_edge.sidechain);
}

#[test]
fn edges_are_sorted_and_dangling_dests_ignored() {
    let channels = vec![
        bus(
            "mix_b",
            vec![send("mix_a", false), send("mix_missing", false)],
        ),
        bus("mix_a", vec![]),
    ];
    let routing = build_mixer_routing(&channels).unwrap();
    let pairs: Vec<(&str, &str)> = routing
        .edges
        .iter()
        .map(|e| (e.source.as_str(), e.destination.as_str()))
        .collect();
    assert!(pairs.contains(&("mix_b", "mix_a")));
    assert!(!pairs.iter().any(|(_, d)| *d == "mix_missing"));
    let mut sorted = pairs.clone();
    sorted.sort();
    assert_eq!(pairs, sorted);
}
