//! Cross-language golden tests: the Rust decoder/encoder must reproduce the
//! canonical fixtures produced by `@oxitone/protocol` byte for byte, and the
//! pcg32-v1/hash64-v1 vectors must match bit for bit.

use std::fs;
use std::path::PathBuf;

use oxitone_core::canonical::{to_canonical_json, write_canonical};
use oxitone_core::error::codes;
use oxitone_core::pcg32::{hash64, Hash64Part, Pcg32};
use oxitone_core::wire::{decode_project_snapshot, encode_project_snapshot, AutomationSourceSpec};

fn fixture(rel: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../schemas/fixtures");
    path.push(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn project_snapshot_round_trips_byte_exact() {
    let text = fixture("project-snapshot.canonical.json");
    let snapshot = decode_project_snapshot(&text).expect("decode fixture snapshot");
    let reencoded = encode_project_snapshot(&snapshot).expect("encode snapshot");
    assert_eq!(
        reencoded, text,
        "Rust re-encode must match the canonical fixture bytes"
    );
}

#[test]
fn project_snapshot_ignores_unknown_fields() {
    let text = fixture("project-snapshot.canonical.json");
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["futureField"] = serde_json::json!({"nested": [1, 2, 3]});
    let snapshot = decode_project_snapshot(&serde_json::to_string(&value).unwrap()).unwrap();
    assert_eq!(encode_project_snapshot(&snapshot).unwrap(), text);
}

#[test]
fn project_snapshot_rejects_unknown_major_version() {
    let text = fixture("project-snapshot.canonical.json");
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["protocolVersion"] = serde_json::json!("2.0");
    let err = decode_project_snapshot(&serde_json::to_string(&value).unwrap()).unwrap_err();
    assert_eq!(err.code, codes::PROTOCOL_VERSION_UNSUPPORTED);
}

#[test]
fn automation_fixtures_round_trip_byte_exact() {
    for name in ["gate", "wave", "chance", "curve", "nested"] {
        let text = fixture(&format!("automation/{name}.canonical.json"));
        let source: AutomationSourceSpec =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("decode {name}: {e}"));
        source
            .validate()
            .unwrap_or_else(|e| panic!("validate {name}: {e}"));
        let reencoded = to_canonical_json(&source).expect("encode source");
        assert_eq!(reencoded, text, "{name} re-encode must match fixture bytes");
    }
}

#[test]
fn pcg32_v1_vectors_match() {
    let text = fixture("pcg32-v1.json");
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["algorithm"], "pcg32-v1");
    assert_eq!(
        file["multiplier"].as_str().unwrap(),
        oxitone_core::PCG32_MULTIPLIER.to_string()
    );
    assert_eq!(
        file["increment"].as_str().unwrap(),
        oxitone_core::PCG32_INCREMENT.to_string()
    );
    for vector in file["vectors"].as_array().unwrap() {
        let seed: u64 = vector["seed"].as_str().unwrap().parse().unwrap();
        let mut rng = Pcg32::new(seed);
        for expected in vector["outputs"].as_array().unwrap() {
            let expected: u32 = expected.as_str().unwrap().parse().unwrap();
            assert_eq!(rng.next_u32(), expected, "seed {seed} u32 stream");
        }
        for expected in vector["floats"].as_array().unwrap() {
            let expected: f64 = expected.as_str().unwrap().parse().unwrap();
            assert_eq!(
                rng.next_f64().to_bits(),
                expected.to_bits(),
                "seed {seed} float stream"
            );
        }
    }
}

#[test]
fn hash64_v1_vectors_match() {
    let text = fixture("hash64.json");
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["algorithm"], "hash64-v1");
    for vector in file["vectors"].as_array().unwrap() {
        let input = vector["input"].as_str().unwrap();
        let parts: Vec<Hash64Part> = input
            .split('|')
            .map(|part| {
                let (tag, raw) = part.split_at(2);
                match tag {
                    "n:" => Hash64Part::Int(raw.parse().unwrap()),
                    "s:" => Hash64Part::Str(raw.to_owned()),
                    other => panic!("unknown hash64 part tag {other}"),
                }
                .clone()
            })
            .collect();
        let expected: u64 = vector["hash"].as_str().unwrap().parse().unwrap();
        assert_eq!(oxitone_core::pcg32::hash64_input(&parts), input);
        assert_eq!(hash64(&parts), expected, "hash64({input})");
    }
}

#[test]
fn vector_files_are_canonical_themselves() {
    for rel in ["pcg32-v1.json", "hash64.json"] {
        let text = fixture(rel);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            write_canonical(&value).unwrap(),
            text,
            "{rel} must be canonical"
        );
    }
}
