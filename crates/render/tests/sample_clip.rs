//! SampleClip golden tests (02-domain-spec.md §tempoSync,
//! 03-audio-runtime-spec.md §tempoSync 的运行时规则): off/stretch/repitch
//! beat placement under a tempo step and a tempo ramp, loop boundaries, and
//! the `SampleStretchRange` compile error.
//!
//! Content: an 8-beat click train (one click per beat at the native 120
//! BPM). With `durationBeats: 8` the content beats must land on project
//! beats. `off` is sample-accurate (±2 frames). `repitch` updates its rate
//! at control rate (per block) through a smoother, so a tempo step/ramp
//! lands within a few frames (±16 declared here). `stretch` is quantized by
//! the WSOLA hop grid, so its tolerance is one hop (±320 frames) — see the
//! module docs of `player/stretch.rs`.

mod common;

use common::*;
use oxitone_core::wire::{LoopSpec, TempoCurve, TempoSegment, TempoSync};
use oxitone_render::{builtin_registry, render_wav, RenderOptions, SampleStore};

const SR: usize = 48_000;
const BEAT: usize = SR / 2; // 120 BPM
const BEATS: usize = 8;

/// Snapshot with one sample clip (8-beat train) on one track. `stretch`
/// uses sine bursts (WSOLA-friendly); the other modes use impulses.
fn clip_snapshot(
    dir: &std::path::Path,
    tempo_sync: TempoSync,
) -> oxitone_core::wire::ProjectSnapshot {
    let content = if tempo_sync == TempoSync::Stretch {
        burst_train(BEATS * BEAT, BEAT)
    } else {
        click_train(BEATS * BEAT, BEAT)
    };
    let sample = sample_asset(dir, "smp_clicks", &content, (8, 1));
    let mut snapshot = base_snapshot();
    // Step at beat 4: 120 → 240 BPM.
    snapshot.tempo_map = vec![
        TempoSegment {
            start_beat: beat(0, 1),
            bpm: 120.0,
            curve: Some(TempoCurve::Step),
        },
        TempoSegment {
            start_beat: beat(4, 1),
            bpm: 240.0,
            curve: Some(TempoCurve::Step),
        },
    ];
    snapshot.samples.push(sample);
    snapshot
        .tracks
        .push(track("trk_clips", &["chn_clips"], &[], &["scl_0001"]));
    snapshot.channels.push(channel(
        "chn_clips",
        "mix_clips",
        wavetable_ref(&[]),
        vec![],
    ));
    snapshot
        .mixer_channels
        .push(mixer_channel("mix_clips", vec![], vec![]));
    let mut clip = sample_clip("scl_0001", "smp_clicks", "trk_clips", Some((8, 1)));
    clip.tempo_sync = Some(tempo_sync);
    snapshot.sample_clips.push(clip);
    snapshot
}

/// Expected click frames: project beats 0..8 under the 120→240 step.
fn expected_step_frames() -> Vec<usize> {
    (0..4)
        .map(|k| k * BEAT)
        .chain((4..BEATS).map(|k| 4 * BEAT + (k - 4) * BEAT / 2))
        .collect()
}

fn assert_onsets_match(left: &[f32], expected: &[usize], tolerance: usize, mode: &str) {
    assert_onsets_match_threshold(left, expected, tolerance, 0.2, mode)
}

fn assert_onsets_match_threshold(
    left: &[f32],
    expected: &[usize],
    tolerance: usize,
    threshold: f32,
    mode: &str,
) {
    let found = onset_frames(left, threshold, BEAT / 4);
    assert_eq!(
        found.len(),
        expected.len(),
        "{mode}: expected {} onsets, got {found:?}",
        expected.len()
    );
    for (&got, &want) in found.iter().zip(expected.iter()) {
        let delta = got.abs_diff(want);
        assert!(
            delta <= tolerance,
            "{mode}: onset at {got}, expected {want} (delta {delta} > {tolerance})"
        );
    }
}

#[test]
fn off_mode_is_tempo_independent() {
    let dir = out_dir("clip-off");
    let snapshot = clip_snapshot(&dir, TempoSync::Off);
    let (left, _, latency) = render_frames(&snapshot, BEATS * BEAT);
    // Content plays at its native rate regardless of the tempo step; the
    // clip spans 144000 frames, so six native clicks fit.
    let expected: Vec<usize> = (0..6).map(|k| k * BEAT + latency as usize).collect();
    assert_onsets_match(&left, &expected, 2, "off");
}

#[test]
fn repitch_tracks_tempo_step_sample_accurate() {
    let dir = out_dir("clip-repitch");
    let snapshot = clip_snapshot(&dir, TempoSync::Repitch);
    let (left, _, latency) = render_frames(&snapshot, BEATS * BEAT);
    let expected: Vec<usize> = expected_step_frames()
        .into_iter()
        .map(|f| f + latency as usize)
        .collect();
    assert_onsets_match(&left, &expected, 16, "repitch");
}

#[test]
fn stretch_tracks_tempo_step_within_one_hop() {
    let dir = out_dir("clip-stretch");
    let snapshot = clip_snapshot(&dir, TempoSync::Stretch);
    let (left, _, latency) = render_frames(&snapshot, BEATS * BEAT);
    let expected: Vec<usize> = expected_step_frames()
        .into_iter()
        .map(|f| f + latency as usize)
        .collect();
    assert_onsets_match_threshold(&left, &expected, 320, 0.05, "stretch");
}

#[test]
fn repitch_tracks_tempo_ramp_sample_accurate() {
    let dir = out_dir("clip-ramp");
    let mut snapshot = clip_snapshot(&dir, TempoSync::Repitch);
    // Linear ramp 120 → 240 over beats 0..8 (one segment, curve linear).
    snapshot.tempo_map = vec![
        TempoSegment {
            start_beat: beat(0, 1),
            bpm: 120.0,
            curve: Some(TempoCurve::Linear),
        },
        TempoSegment {
            start_beat: beat(8, 1),
            bpm: 240.0,
            curve: None,
        },
    ];
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let plan = oxitone_graph::compile_plan(
        &snapshot,
        &registry,
        &store,
        &oxitone_graph::CompileOptions::default(),
    )
    .unwrap();
    let expected: Vec<usize> = (0..BEATS)
        .map(|k| plan.tempo.beat_to_frame(beat(k as i64, 1)) as usize)
        .collect();
    let (left, _, latency) = render_frames(&snapshot, BEATS * BEAT);
    let expected: Vec<usize> = expected.into_iter().map(|f| f + latency as usize).collect();
    assert_onsets_match(&left, &expected, 32, "repitch-ramp");
}

#[test]
fn loop_repeats_content_with_crossfade() {
    let dir = out_dir("clip-loop");
    // One-beat content with the click mid-region (loop-boundary crossfade
    // runs over silence) plus a half-beat tail the crossfade reads into.
    let mut content = vec![0.0f32; BEAT + BEAT / 2];
    for i in 0..200 {
        content[BEAT / 2 + i] += 0.9 * (-(i as f32) / 40.0).exp();
    }
    let sample = sample_asset(&dir, "smp_loop", &content, (3, 2));
    let mut snapshot = base_snapshot();
    snapshot.samples.push(sample);
    snapshot
        .tracks
        .push(track("trk_clips", &["chn_clips"], &[], &["scl_loop"]));
    snapshot.channels.push(channel(
        "chn_clips",
        "mix_clips",
        wavetable_ref(&[]),
        vec![],
    ));
    snapshot
        .mixer_channels
        .push(mixer_channel("mix_clips", vec![], vec![]));
    let mut clip = sample_clip("scl_loop", "smp_loop", "trk_clips", Some((4, 1)));
    clip.loop_spec = Some(LoopSpec {
        start_beat: None,
        length_beats: beat(1, 1),
        count: None,
        last_beat: None,
    });
    snapshot.sample_clips.push(clip);
    let (left, _, latency) = render_frames(&snapshot, 4 * BEAT);
    let expected: Vec<usize> = (0..4)
        .map(|k| k * BEAT + BEAT / 2 + latency as usize)
        .collect();
    assert_onsets_match(&left, &expected, 2, "loop");
}

#[test]
fn disabled_clip_is_silent() {
    let dir = out_dir("clip-disabled");
    let mut snapshot = clip_snapshot(&dir, TempoSync::Off);
    snapshot.sample_clips[0].enabled = Some(false);
    let (left, _, _) = render_frames(&snapshot, 2 * BEAT);
    assert!(
        left.iter().all(|&x| x == 0.0),
        "disabled clip produced audio"
    );
}

#[test]
fn stretch_ratio_out_of_range_is_compile_error() {
    let dir = out_dir("clip-range");
    let content = click_train(BEATS * BEAT, BEAT);
    let sample = sample_asset(&dir, "smp_long", &content, (8, 1));
    let mut snapshot = base_snapshot();
    snapshot.samples.push(sample);
    snapshot
        .tracks
        .push(track("trk_clips", &["chn_clips"], &[], &["scl_bad"]));
    snapshot.channels.push(channel(
        "chn_clips",
        "mix_clips",
        wavetable_ref(&[]),
        vec![],
    ));
    snapshot
        .mixer_channels
        .push(mixer_channel("mix_clips", vec![], vec![]));
    // 8 content beats squeezed into 1 beat: ratio 0.125 < 0.25.
    let mut clip = sample_clip("scl_bad", "smp_long", "trk_clips", Some((1, 1)));
    clip.tempo_sync = Some(TempoSync::Stretch);
    snapshot.sample_clips.push(clip);
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(dir.join("bad.wav"));
    options.end = Some(oxitone_render::RenderPoint::Beat(4.0));
    let error = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap_err();
    assert_eq!(error.code, oxitone_core::codes::SAMPLE_STRETCH_RANGE);
}
