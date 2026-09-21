mod common;
use common::*;
use oxitone_graph::{midi, validate};
use std::collections::BTreeMap;

fn channels() -> Vec<oxitone_core::wire::ChannelSpec> {
    ["chn_z", "chn_a", "chn_m"]
        .iter()
        .map(|id| {
            let mut channel = base_snapshot().channels.remove(0);
            channel.id = (*id).into();
            channel.instrument.instance_id = Some(format!("ins_{id}"));
            channel
        })
        .collect()
}
#[test]
fn dependency_order_does_not_reindex_channel_bindings() {
    let mut c = channels();
    c[0].midi_routes = Some(BTreeMap::from([(
        "ins_chn_z".into(),
        vec!["chn_a".into(), "chn_m".into()],
    )]));
    let compiled = midi::compile(&c, None).unwrap().unwrap();
    assert_eq!(compiled.order, [2, 0, 1]);
    assert_eq!(compiled.routes[2][0].destinations, [0, 1]);
    assert_eq!(compiled.routes[2][0].insert, None);
    c[1].midi_routes = Some(BTreeMap::from([("ins_chn_a".into(), vec!["chn_z".into()])]));
    assert!(midi::compile(&c, None).is_err());
}
#[test]
fn dangling_owned_sources_and_invalid_targets_fail_before_loading_native_plugins() {
    for (instance, targets) in [
        ("ins_missing", vec!["chn_a"]),
        ("ins_chn_z", vec!["missing"]),
        ("ins_chn_z", vec!["chn_z"]),
        ("ins_chn_z", vec!["chn_a", "chn_a"]),
        ("ins_chn_z", vec![]),
    ] {
        let mut c = channels();
        c[0].midi_routes = Some(BTreeMap::from([(
            instance.into(),
            targets.into_iter().map(String::from).collect(),
        )]));
        assert!(midi::compile(&c, None).is_err());
    }
}
#[test]
fn requires_version_floor_and_output_capability() {
    let mut s = base_snapshot();
    s.channels = channels();
    s.tracks[0].channel_ids = vec![s.channels[0].id.clone()];
    s.channels[0].midi_routes = Some(BTreeMap::from([("ins_chn_z".into(), vec!["chn_a".into()])]));
    s.protocol_version = "1.6".into();
    assert_eq!(
        validate(&s, &base_registry()).unwrap_err().code,
        "ProtocolVersionUnsupported"
    );
    s.protocol_version = "1.7".into();
    assert_eq!(
        validate(&s, &base_registry()).unwrap_err().code,
        "PluginCapabilityUnsupported"
    );
}
