mod common;
use common::{base_registry, base_snapshot};
use oxitone_core::{
    codes,
    wire::{SampleProvenance, SampleRef},
};

#[test]
fn provenance_is_preserved_and_invalid_metadata_is_rejected() {
    let mut snapshot = base_snapshot();
    let reference: SampleRef = serde_json::from_value(serde_json::json!({
        "id": "smp_imported", "assetUri": "assets/test.wav", "sha256": "12".repeat(32),
        "format": "wav", "sampleRate": 48000, "channels": 2, "frames": "128",
        "provenance": {
            "sourceSha256": "ab".repeat(32), "sourceFormat": "m4a", "sourceSampleRate": 44100,
            "sourceChannels": 6, "decoder": "symphonia 0.5/aac",
            "channelLayoutAction": "downmixed-to-stereo", "cacheEncoding": "wav-f32-v1"
        }
    }))
    .unwrap();
    snapshot.samples.push(reference);
    oxitone_graph::validate(&snapshot, &base_registry()).unwrap();
    let encoded = oxitone_core::canonical::to_canonical_json(&snapshot).unwrap();
    let restored: oxitone_core::wire::ProjectSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(snapshot, restored);
    let valid = snapshot.samples[0].provenance.clone().unwrap();
    let mutations: [fn(&mut SampleProvenance); 6] = [
        |p| p.source_sha256 = "AB".repeat(32),
        |p| p.source_sha256 = "a".repeat(63),
        |p| p.source_sample_rate = 0,
        |p| p.source_channels = 0,
        |p| p.source_bit_depth = Some(0),
        |p| p.decoder.clear(),
    ];
    for mutate in mutations {
        let mut provenance = valid.clone();
        mutate(&mut provenance);
        snapshot.samples[0].provenance = Some(provenance);
        let error = oxitone_graph::validate(&snapshot, &base_registry()).unwrap_err();
        assert_eq!(error.code, codes::INVALID_PROJECT);
        assert!(error.path.unwrap().ends_with(".provenance"));
    }
}
