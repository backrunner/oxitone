//! Error-path tests: `WavTooLarge`, mixed render-range kinds, unknown
//! stretch algorithm, and the deterministic dither/size boundaries.

mod common;

use common::*;
use oxitone_core::codes;
use oxitone_render::wav::{check_wav_size, WavBitDepth};
use oxitone_render::{builtin_registry, render_wav, RenderOptions, RenderPoint, SampleStore};

#[test]
fn wav_size_limit_is_enforced_before_rendering() {
    // float32 stereo: limit is (u32::MAX - 44) / 8 frames.
    let max_frames = (u32::MAX as u64 - 44) / 8;
    assert!(check_wav_size(max_frames, 2, WavBitDepth::Float32).is_ok());
    let error = check_wav_size(max_frames + 1, 2, WavBitDepth::Float32).unwrap_err();
    assert_eq!(error.code, codes::WAV_TOO_LARGE);
    // 16-bit stereo packs 4 bytes/frame: twice the byte budget per frame.
    let max_frames_16 = (u32::MAX as u64 - 44) / 4;
    assert!(check_wav_size(max_frames_16, 2, WavBitDepth::Pcm16).is_ok());
    let error = check_wav_size(max_frames_16 + 1, 2, WavBitDepth::Pcm16).unwrap_err();
    assert_eq!(error.code, codes::WAV_TOO_LARGE);
    assert!(check_wav_size(max_frames_16, 2, WavBitDepth::Pcm24).is_err());
}

#[test]
fn mixed_start_end_kinds_are_rejected() {
    let snapshot = base_snapshot();
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(out_dir("errors").join("mixed.wav"));
    options.start = Some(RenderPoint::Bar(1));
    options.end = Some(RenderPoint::Beat(4.0));
    let error = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap_err();
    assert_eq!(error.code, codes::INVALID_PROJECT);
    assert!(
        error.message.contains("same range kind"),
        "{}",
        error.message
    );
}

#[test]
fn timecode_family_allows_seconds_and_frames_together() {
    // Seconds and frames are both the Timecode family: not a mix.
    let mut snapshot = base_snapshot();
    snapshot.tracks = vec![track("trk_n", &["chn_n"], &["pcl_n"], &[])];
    snapshot.patterns = vec![pattern(
        "pat_n",
        (4, 1),
        vec![note(60, (0, 1), (1, 2), 0.8)],
    )];
    snapshot.pattern_clips = vec![pattern_clip("pcl_n", "pat_n", "trk_n", (0, 1), (4, 1))];
    snapshot.channels = vec![channel("chn_n", "mix_n", wavetable_ref(&[]), vec![])];
    snapshot.mixer_channels = vec![mixer_channel("mix_n", vec![], vec![])];
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(out_dir("errors").join("timecode.wav"));
    options.start = Some(RenderPoint::Seconds(0.0));
    options.end = Some(RenderPoint::Frames(48_000));
    let report = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap();
    assert!((report.files[0].duration_seconds - 1.0).abs() < 0.001);
}

#[test]
fn unknown_stretch_algorithm_is_rejected() {
    let dir = out_dir("errors-stretch");
    let content = click_train(24_000, 24_000);
    let sample = sample_asset(&dir, "smp_one", &content, (1, 1));
    let mut snapshot = base_snapshot();
    snapshot.samples.push(sample);
    snapshot.tracks = vec![track("trk_c", &["chn_c"], &[], &["scl_c"])];
    snapshot.channels = vec![channel("chn_c", "mix_c", wavetable_ref(&[]), vec![])];
    snapshot.mixer_channels = vec![mixer_channel("mix_c", vec![], vec![])];
    let mut clip = sample_clip("scl_c", "smp_one", "trk_c", Some((1, 1)));
    clip.tempo_sync = Some(oxitone_core::wire::TempoSync::Stretch);
    clip.stretch_algorithm = Some("elastique-v9".into());
    snapshot.sample_clips = vec![clip];
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(dir.join("alg.wav"));
    options.end = Some(RenderPoint::Beat(1.0));
    let error = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap_err();
    assert_eq!(error.code, codes::INVALID_PROJECT);
    assert!(
        error.message.contains("stretchAlgorithm"),
        "{}",
        error.message
    );
}

#[test]
fn end_before_start_is_rejected() {
    let snapshot = base_snapshot();
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(out_dir("errors").join("order.wav"));
    options.start = Some(RenderPoint::Beat(8.0));
    options.end = Some(RenderPoint::Beat(4.0));
    let error = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap_err();
    assert_eq!(error.code, codes::INVALID_PROJECT);
}

#[test]
fn marker_range_renders_between_markers() {
    let mut snapshot = base_snapshot();
    snapshot.markers = vec![
        oxitone_core::wire::MarkerSpec {
            id: "mrk_a".into(),
            name: None,
            start_beat: beat(0, 1),
        },
        oxitone_core::wire::MarkerSpec {
            id: "mrk_b".into(),
            name: None,
            start_beat: beat(4, 1),
        },
    ];
    snapshot.tracks = vec![track("trk_n", &["chn_n"], &["pcl_n"], &[])];
    snapshot.patterns = vec![pattern(
        "pat_n",
        (4, 1),
        vec![note(60, (0, 1), (1, 2), 0.8)],
    )];
    snapshot.pattern_clips = vec![pattern_clip("pcl_n", "pat_n", "trk_n", (0, 1), (4, 1))];
    snapshot.channels = vec![channel("chn_n", "mix_n", wavetable_ref(&[]), vec![])];
    snapshot.mixer_channels = vec![mixer_channel("mix_n", vec![], vec![])];
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(out_dir("errors").join("markers.wav"));
    options.start = Some(RenderPoint::Marker("mrk_a".into()));
    options.end = Some(RenderPoint::Marker("mrk_b".into()));
    let report = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap();
    // 4 beats at 120 BPM = 2 s.
    assert!((report.files[0].duration_seconds - 2.0).abs() < 0.001);
    assert!(report.files[0].peak_dbfs.is_finite());
}
