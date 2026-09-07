mod common;

use common::*;
use oxitone_core::codes;
use oxitone_core::wire::{
    MixerChannelSpec, NoteSpec, PatternClipSpec, PatternSpec, SampleClipSpec, SampleRef, SendSpec,
};
use oxitone_graph::{validate, MASTER_MIXER_CHANNEL_ID};

fn err_code(
    snapshot: &oxitone_core::wire::ProjectSnapshot,
) -> (&'static str, String, Option<String>) {
    let registry = base_registry();
    let err = validate(snapshot, &registry).unwrap_err();
    (err.code, err.message, err.path)
}

#[test]
fn base_snapshot_is_valid() {
    assert!(validate(&base_snapshot(), &base_registry()).is_ok());
}

#[test]
fn duplicate_ids_are_rejected_globally() {
    let mut snapshot = base_snapshot();
    snapshot.markers.push(oxitone_core::wire::MarkerSpec {
        id: "chn_0001".to_string(),
        name: None,
        start_beat: beat(0, 1),
    });
    let (code, message, _) = err_code(&snapshot);
    assert_eq!(code, codes::INVALID_PROJECT);
    assert!(message.contains("duplicate"), "{message}");

    let mut snapshot = base_snapshot();
    snapshot.mixer_channels.push(MixerChannelSpec {
        id: "mix_0001".to_string(),
        ..snapshot.mixer_channels[0].clone()
    });
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);
}

#[test]
fn documented_prefixes_are_enforced() {
    let mut snapshot = base_snapshot();
    snapshot.tracks[0].id = "pat_0001".to_string();
    let (_, message, path) = err_code(&snapshot);
    assert!(message.contains("trk_"), "{message}");
    assert_eq!(path.as_deref(), Some("$.tracks[0].id"));

    let mut snapshot = base_snapshot();
    snapshot.channels[0].id = "channel1".to_string();
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);
}

#[test]
fn malformed_ids_are_rejected() {
    let mut snapshot = base_snapshot();
    snapshot.tracks[0].id = "trk_has space".to_string();
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);

    let mut snapshot = base_snapshot();
    snapshot
        .automation
        .push(oxitone_core::wire::AutomationLaneSpec {
            id: "".to_string(),
            target: oxitone_core::wire::AutomationTarget {
                entity_id: "chn_0001".to_string(),
                parameter_id: "level".to_string(),
            },
            source: oxitone_core::wire::AutomationSourceSpec::Constant { value: 0.5 },
            combine: None,
            loop_spec: None,
            last_beat: None,
        });
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);
}

fn add_pattern(snapshot: &mut oxitone_core::wire::ProjectSnapshot) {
    snapshot.patterns.push(PatternSpec {
        id: "pat_0001".to_string(),
        name: None,
        length_beats: beat(4, 1),
        notes: vec![NoteSpec {
            id: None,
            pitch: 60,
            start: beat(0, 1),
            duration: beat(1, 4),
            velocity: 0.8,
            off_velocity: None,
            chance: None,
            voice: None,
            tags: None,
        }],
    });
}

#[test]
fn dangling_clip_references_are_rejected() {
    let mut snapshot = base_snapshot();
    snapshot.pattern_clips.push(PatternClipSpec {
        id: "pcl_0001".to_string(),
        pattern_id: "pat_missing".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: None,
        loop_count: None,
        last_beat: None,
        transpose: None,
        velocity_scale: None,
        probability: None,
        enabled: None,
    });
    let (code, _, path) = err_code(&snapshot);
    assert_eq!(code, codes::INVALID_PROJECT);
    assert_eq!(path.as_deref(), Some("$.patternClips[0].patternId"));

    let mut snapshot = base_snapshot();
    add_pattern(&mut snapshot);
    snapshot.pattern_clips.push(PatternClipSpec {
        track_id: "trk_missing".to_string(),
        ..snapshot_pattern_clip()
    });
    assert_eq!(
        err_code(&snapshot).2.as_deref(),
        Some("$.patternClips[0].trackId")
    );

    let mut snapshot = base_snapshot();
    snapshot.pattern_clips.push(snapshot_pattern_clip());
    add_pattern(&mut snapshot);
    assert!(validate(&snapshot, &base_registry()).is_ok());
}

fn snapshot_pattern_clip() -> PatternClipSpec {
    PatternClipSpec {
        id: "pcl_0001".to_string(),
        pattern_id: "pat_0001".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: Some(beat(4, 1)),
        loop_count: None,
        last_beat: None,
        transpose: None,
        velocity_scale: None,
        probability: None,
        enabled: None,
    }
}

#[test]
fn dangling_sample_clip_and_track_membership_references() {
    let mut snapshot = base_snapshot();
    snapshot.sample_clips.push(SampleClipSpec {
        id: "scl_0001".to_string(),
        sample_id: "smp_missing".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: None,
        gain: None,
        pan: None,
        rate: None,
        loop_spec: None,
        tempo_sync: None,
        stretch_algorithm: None,
        enabled: None,
    });
    assert_eq!(
        err_code(&snapshot).2.as_deref(),
        Some("$.sampleClips[0].sampleId")
    );

    let mut snapshot = base_snapshot();
    snapshot.tracks[0]
        .channel_ids
        .push("chn_missing".to_string());
    assert_eq!(
        err_code(&snapshot).2.as_deref(),
        Some("$.tracks[0].channelIds[1]")
    );

    let mut snapshot = base_snapshot();
    snapshot.tracks[0]
        .pattern_clip_ids
        .push("pcl_missing".to_string());
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);
}

#[test]
fn channel_must_route_to_a_known_bus() {
    let mut snapshot = base_snapshot();
    snapshot.channels[0].mixer_channel_id = "mix_missing".to_string();
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);

    // Routing a channel directly at the implicit Master is allowed.
    let mut snapshot = base_snapshot();
    snapshot.channels[0].mixer_channel_id = MASTER_MIXER_CHANNEL_ID.to_string();
    assert!(validate(&snapshot, &base_registry()).is_ok());
}

fn send(destination: &str, ratio: f64, sidechain: bool) -> SendSpec {
    SendSpec {
        destination_id: destination.to_string(),
        ratio,
        pre_fader: None,
        sidechain: Some(sidechain),
    }
}

#[test]
fn send_reference_rules() {
    let mut snapshot = base_snapshot();
    snapshot.mixer_channels[0]
        .sends
        .push(send("mix_missing", 0.5, false));
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);

    // destination must never be Master
    let mut snapshot = base_snapshot();
    snapshot.mixer_channels[0]
        .sends
        .push(send(MASTER_MIXER_CHANNEL_ID, 0.5, false));
    let (_, message, _) = err_code(&snapshot);
    assert!(message.contains("Master"), "{message}");

    // duplicate destination on the same bus
    let mut snapshot = base_snapshot();
    snapshot.mixer_channels.push(MixerChannelSpec {
        id: "mix_0002".to_string(),
        ..snapshot.mixer_channels[0].clone()
    });
    snapshot.mixer_channels[0].sends =
        vec![send("mix_0002", 0.5, false), send("mix_0002", 0.25, true)];
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);

    // valid send passes
    let mut snapshot = base_snapshot();
    snapshot.mixer_channels.push(MixerChannelSpec {
        id: "mix_0002".to_string(),
        ..snapshot.mixer_channels[0].clone()
    });
    snapshot.mixer_channels[0].sends = vec![send("mix_0002", 0.5, true)];
    assert!(validate(&snapshot, &base_registry()).is_ok());
}

#[test]
fn master_cannot_be_re_routed() {
    let master = || MixerChannelSpec {
        id: MASTER_MIXER_CHANNEL_ID.to_string(),
        name: None,
        level: 1.0,
        balance: 0.0,
        master_send_ratio: None,
        inserts: vec![],
        sends: vec![],
        mute: None,
        solo: None,
    };

    let mut snapshot = base_snapshot();
    snapshot.mixer_channels.push(MixerChannelSpec {
        sends: vec![send("mix_0001", 0.5, false)],
        ..master()
    });
    let (_, message, _) = err_code(&snapshot);
    assert!(message.contains("Master"), "{message}");

    let mut snapshot = base_snapshot();
    snapshot.mixer_channels.push(MixerChannelSpec {
        master_send_ratio: Some(1.0),
        ..master()
    });
    assert_eq!(err_code(&snapshot).0, codes::INVALID_PROJECT);

    // An explicit Master without sends is fine.
    let mut snapshot = base_snapshot();
    snapshot.mixer_channels.push(master());
    assert!(validate(&snapshot, &base_registry()).is_ok());
}

#[test]
fn samples_validate_with_clips() {
    let mut snapshot = base_snapshot();
    snapshot.samples.push(SampleRef {
        id: "smp_0001".to_string(),
        asset_uri: "assets/a.wav".to_string(),
        sha256: "ab".repeat(32),
        format: oxitone_core::wire::SampleFormat::Wav,
        sample_rate: 48_000,
        channels: 2,
        frames: 192_000,
        edits: None,
        musical_length_beats: None,
        provenance: None,
    });
    snapshot.sample_clips.push(SampleClipSpec {
        id: "scl_0001".to_string(),
        sample_id: "smp_0001".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: Some(beat(8, 1)),
        gain: Some(1.0),
        pan: None,
        rate: None,
        loop_spec: None,
        tempo_sync: None,
        stretch_algorithm: None,
        enabled: None,
    });
    assert!(validate(&snapshot, &base_registry()).is_ok());
}
