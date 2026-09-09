use crate::{playback_controls::frame, shortcuts, tests::engine, timeline_input, wire};
use oxitone_core::Beat;
use serde_json::json;

#[test]
fn pointer_coordinates_respect_scroll_tempo_repeats_and_clip_truncation() {
    assert!((timeline_input::playlist_beat(221.5, 100., -50., 20., 64.) - 8.575).abs() < 1e-6);
    assert_eq!(timeline_input::playlist_beat(0., 100., 0., 20., 64.), 0.);
    let mut engine = engine();
    let project = std::sync::Arc::get_mut(engine.current.as_mut().unwrap()).unwrap();
    project.snapshot.pattern_clips[0].last_beat = Some(Beat::new(12, 1).unwrap());
    assert_eq!(
        timeline_input::piano_beat(project, "pcl_keys", 1.25, frame(project, 26.)),
        Some(29.)
    );
    assert_eq!(
        timeline_input::piano_beat(project, "pcl_keys", 1.25, 0),
        Some(13.)
    );
    project.snapshot.pattern_clips[0].last_beat = Some(Beat::new(13, 2).unwrap());
    assert_eq!(
        timeline_input::piano_beat(project, "pcl_keys", 3., 0),
        Some(18.)
    );
}

#[test]
fn fractional_positions_and_shortcuts_follow_musical_time() {
    let mut engine = engine();
    let p = std::sync::Arc::get_mut(engine.current.as_mut().unwrap()).unwrap();
    assert_eq!(shortcuts::step(p, 192000, 1, true), 228000); // 3/4 bar at 240 BPM
    assert_eq!(shortcuts::step(p, 168000, 1, false), 192000); // crosses the tempo change
    p.snapshot.markers = serde_json::from_value(json!([
        {"id":"mrk_late","startBeat":{"numerator":12,"denominator":1}},
        {"id":"mrk_early","startBeat":{"numerator":4,"denominator":1}}
    ]))
    .unwrap();
    assert_eq!(shortcuts::marker(p, frame(p, 4.), 1), frame(p, 12.));
    assert_eq!(shortcuts::marker(p, frame(p, 12.), -1), frame(p, 4.));
    use shortcuts::PlaybackShortcut::*;
    assert_eq!(
        shortcuts::playback(&gpui::Keystroke::parse("alt-shift-right").unwrap()),
        Some(Step(1, true))
    );
    for key in ["right", "home", "cmd-space", "cmd-l", "alt-enter"] {
        assert_eq!(
            shortcuts::playback(&gpui::Keystroke::parse(key).unwrap()),
            None,
            "{key}"
        );
    }
}

#[test]
fn native_transport_preserves_precise_cue_through_watch_and_pause() {
    let mut engine = engine();
    let command = |engine: &mut crate::engine::Engine, command| {
        assert_eq!(
            engine.handle(wire::Frame::Transport { command })["type"],
            "state"
        );
    };
    command(&mut engine, json!({"command":"seek","frame":"123456"}));
    let mut snapshot = engine.current.as_ref().unwrap().snapshot.clone();
    snapshot.revision = 2;
    let update = wire::decode(
        json!({"protocolVersion":"1.0","type":"snapshot","snapshot":snapshot,
        "assetBaseDir":"/tmp","hash":"cd".repeat(32)}),
    )
    .unwrap();
    assert_eq!(engine.handle(update)["cursor"], "123456");
    command(&mut engine, json!({"command":"play"}));
    wait(&engine, |p| p.playing && p.cursor > 123456);
    command(&mut engine, json!({"command":"pause"}));
    wait(&engine, |p| !p.playing);
    let paused = engine.playback();
    assert_eq!(
        paused.audible, paused.cursor,
        "paused cursor is not shifted by output latency"
    );
    command(&mut engine, json!({"command":"stop"}));
    command(&mut engine, json!({"command":"seek","frame":"123456"}));
    wait(&engine, |p| !p.playing && p.cursor == 123456);
    command(&mut engine, json!({"command":"play","frame":"250001"}));
    wait(&engine, |p| p.playing && p.cursor >= 250001);
}
fn wait(engine: &crate::engine::Engine, ready: impl Fn(&crate::model::PlaybackStatus) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !ready(&engine.playback()) {
        assert!(
            std::time::Instant::now() < deadline,
            "native transport must converge"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
