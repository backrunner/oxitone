// Included by sample_clip.rs; shared click-train fixtures stay in that suite.
use super::*;

#[test]
fn stretch_reset_is_allocation_free_and_replays_identical_audio() {
    let dir = out_dir("stretch-reset");
    let snapshot = clip_snapshot(&dir, TempoSync::Stretch);
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let plan =
        oxitone_graph::compile_plan(&snapshot, &registry, &store, &Default::default()).unwrap();
    let mut player =
        oxitone_render::player::SampleClipPlayer::new(&plan.sample_clips[0], SR as f64, 128);
    let mut expected = vec![0.0; 8192];
    let mut actual = vec![0.0; 8192];
    let mut right = [0.0; 128];
    for chunk in expected.chunks_mut(128) {
        player.render(chunk.len(), chunk, &mut right);
    }
    let (allocated, freed) = allocations::count(|| {
        player.reset();
        for chunk in actual.chunks_mut(128) {
            player.render(chunk.len(), chunk, &mut right);
        }
    });
    assert_eq!((allocated, freed), (0, 0));
    assert_eq!(actual, expected);
    assert!(actual.iter().any(|x| x.abs() > 0.01));
}

#[test]
fn repitch_fills_explicit_shorter_and_longer_durations_under_ramps() {
    for (name, duration, curve) in [
        ("short", 4, TempoCurve::Step),
        ("long", 16, TempoCurve::Step),
        ("linear", 4, TempoCurve::Linear),
        ("exponential", 4, TempoCurve::Exponential),
    ] {
        let dir = out_dir(&format!("clip-fit-{name}"));
        let mut snapshot = clip_snapshot(&dir, TempoSync::Repitch);
        snapshot.tempo_map[0].curve = Some(curve);
        snapshot.tempo_map[1].start_beat = beat(duration, 1);
        snapshot.sample_clips[0].duration_beats = Some(beat(duration, 1));
        let tempo =
            oxitone_transport::tempo::TempoMap::compile(&snapshot.tempo_map, SR as u32).unwrap();
        let expected: Vec<usize> = (0..BEATS)
            .map(|k| tempo.beat_to_frame(beat(k as i64 * duration, BEATS as u32)) as usize)
            .collect();
        let (left, _, latency) = render_frames(
            &snapshot,
            tempo.beat_to_frame(beat(duration, 1)) as usize + 128,
        );
        let expected: Vec<usize> = expected
            .into_iter()
            .map(|frame| frame + latency as usize)
            .collect();
        assert_onsets_match(&left, &expected, 64, name);
    }
}
