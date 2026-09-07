use crate::{analysis::NodeAnalysis, engine::Engine, position, wire};
use oxitone_render::preview::NoteEcho;
use serde_json::json;

fn engine() -> Engine {
    let (events, _) = std::sync::mpsc::channel();
    let mut engine = Engine::new(true, events);
    let beat = |n| json!({"numerator":n,"denominator":1});
    let snapshot = json!({"protocolVersion":"1.0","revision":"1","id":"prj_view","sampleRate":48000,"blockSize":128,"seed":1,
        "tempoMap":[{"startBeat":beat(0),"bpm":120},{"startBeat":beat(8),"bpm":240}],
        "timeSignatureMap":[{"startBar":1,"numerator":4,"denominator":4},{"startBar":3,"numerator":3,"denominator":4}],
        "markers":[],"samples":[],"sampleClips":[],"automation":[],"mixerChannels":[],
        "tracks":[{"id":"trk_keys","channelIds":["chn_keys"],"patternClipIds":["pcl_keys"],"sampleClipIds":[],"tempo":60}],
        "channels":[{"id":"chn_keys","instrument":{"pluginId":"oxitone.wavetable","pluginVersion":"1.0.0","parameters":{}},"effectChain":[],"level":1,"pan":0,"mixerChannelId":"mix_master"}],
        "patterns":[{"id":"pat_keys","lengthBeats":beat(4),"notes":[{"pitch":60,"start":beat(0),"duration":beat(1),"velocity":0.7}]}],
        "patternClips":[{"id":"pcl_keys","trackId":"trk_keys","patternId":"pat_keys","startBeat":beat(4),"lastBeat":beat(8)}]});
    let frame=wire::decode(json!({"protocolVersion":"1.0","type":"snapshot","snapshot":snapshot,"assetBaseDir":"/tmp","hash":"ab".repeat(32)})).unwrap();
    assert_eq!(engine.handle(frame)["revision"], "1");
    engine
}

#[test]
fn layout_uses_compiled_clock_local_clip_boundaries_and_time_signature_positions() {
    let engine = engine();
    let project = engine.current.unwrap();
    let mut clip = project.snapshot.pattern_clips[0].clone();
    assert_eq!(project.clip_bounds(&clip), (8., 24.));
    assert_eq!(project.global_to_local("trk_keys", 24.), 8.);
    assert_eq!(project.beat(48000 * 8), 24.);
    clip.last_beat = None;
    clip.loop_count = Some(3);
    assert_eq!(project.clip_bounds(&clip), (8., 56.));
    clip.loop_count = None;
    clip.duration_beats = Some(oxitone_core::Beat::new(2, 1).unwrap());
    assert_eq!(project.clip_bounds(&clip), (8., 16.));
    assert_eq!(position::resolve(&project, "3.1").unwrap(), 192000);
    assert_eq!(position::resolve(&project, "4.1").unwrap(), 228000);
    assert_eq!(position::resolve(&project, "0:08").unwrap(), 384000);
    assert_eq!(position::resolve(&project, "8s").unwrap(), 384000);
    for invalid in ["0.1", "1.0", "0:60", "NaNs", "-1s", ""] {
        assert!(position::resolve(&project, invalid).is_err());
    }
}

#[test]
fn note_feedback_waits_for_audible_frame_and_clears_on_seek_or_pause() {
    let engine = engine();
    let project = engine.current.unwrap();
    let node = &project.telemetry.channels[0];
    let mut analysis = NodeAnalysis::default();
    node.notes
        .push(NoteEcho {
            frame: 500,
            epoch: 0,
            pitch: 60,
            on: true,
        })
        .unwrap();
    node.notes
        .push(NoteEcho {
            frame: 700,
            epoch: 0,
            pitch: 60,
            on: false,
        })
        .unwrap();
    analysis.update(node, 499, 0, true);
    assert!(!analysis.sounding(60));
    analysis.update(node, 500, 0, true);
    assert!(analysis.sounding(60));
    analysis.update(node, 700, 0, true);
    assert!(!analysis.sounding(60));
    node.notes
        .push(NoteEcho {
            frame: 800,
            epoch: 0,
            pitch: 60,
            on: true,
        })
        .unwrap();
    analysis.update(node, 800, 0, true);
    assert!(analysis.sounding(60));
    analysis.update(node, 800, 1, true);
    assert!(!analysis.sounding(60));
    node.notes
        .push(NoteEcho {
            frame: 900,
            epoch: 1,
            pitch: 62,
            on: true,
        })
        .unwrap();
    analysis.update(node, 900, 1, true);
    assert!(analysis.sounding(62));
    analysis.update(node, 900, 1, false);
    assert!(!analysis.sounding(62));
}
