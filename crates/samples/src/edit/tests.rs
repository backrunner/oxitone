use oxitone_core::error::codes;
use oxitone_core::wire::{FadeCurve, FadeSpec, NormalizeSpec, SampleEditSpec, SampleFormat};
use oxitone_core::Beat;

use crate::fixtures::{build_wav, sample_ref_for};
use crate::{decode_bytes, prepare, DecodedSample};

fn constant_decoded(frames: usize, value: f32, rate: u32) -> (Vec<u8>, DecodedSample) {
    let bytes = build_wav(1, rate, 32, true, &vec![value; frames], None);
    let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
    (bytes, decoded)
}

fn edits() -> SampleEditSpec {
    SampleEditSpec {
        start_frame: None,
        end_frame: None,
        level: None,
        tone: None,
        normalize: None,
        fade_in: None,
        fade_out: None,
        crossfade: None,
    }
}

#[test]
fn trim_uses_half_open_range() {
    let (bytes, decoded) = constant_decoded(100, 0.5, 44100);
    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 100);
    let mut spec = edits();
    spec.start_frame = Some(10);
    spec.end_frame = Some(30);
    reference.edits = Some(spec);
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    assert_eq!(prepared.frames(), 20);
    assert!(prepared.channels[0].iter().all(|&s| s == 0.5));
    assert_eq!(prepared.sample_rate, 44100);
}

#[test]
fn invalid_trim_is_invalid_project() {
    let (bytes, decoded) = constant_decoded(100, 0.5, 44100);
    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 100);
    let mut spec = edits();
    spec.start_frame = Some(50);
    spec.end_frame = Some(50);
    reference.edits = Some(spec.clone());
    let err = prepare(&reference, &decoded, 44100).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);
    assert_eq!(err.path.as_deref(), Some("$.edits.startFrame"));
    spec.start_frame = Some(0);
    spec.end_frame = Some(101);
    reference.edits = Some(spec);
    assert_eq!(
        prepare(&reference, &decoded, 44100).unwrap_err().code,
        codes::INVALID_PROJECT
    );
}

#[test]
fn level_scales_and_is_range_checked() {
    let (bytes, decoded) = constant_decoded(16, 0.25, 44100);
    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 16);
    let mut spec = edits();
    spec.level = Some(2.0);
    reference.edits = Some(spec.clone());
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    assert!(prepared.channels[0].iter().all(|&s| (s - 0.5).abs() < 1e-7));
    spec.level = Some(2.5);
    reference.edits = Some(spec);
    assert_eq!(
        prepare(&reference, &decoded, 44100).unwrap_err().code,
        codes::INVALID_PROJECT
    );
}

#[test]
fn normalize_hits_target_peak_db() {
    let (bytes, decoded) = constant_decoded(32, 0.25, 44100);
    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 32);
    let mut spec = edits();
    spec.normalize = Some(NormalizeSpec { peak_db: -1.0 });
    reference.edits = Some(spec.clone());
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    let expected = 10f64.powf(-1.0 / 20.0) as f32;
    assert!(prepared.channels[0]
        .iter()
        .all(|&s| (s - expected).abs() < 1e-6));

    spec.normalize = Some(NormalizeSpec { peak_db: 0.5 });
    reference.edits = Some(spec);
    let err = prepare(&reference, &decoded, 44100).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);
}

#[test]
fn fade_curves_have_expected_shape() {
    let frames = 20;
    for (curve, mid_gain) in [
        (FadeCurve::Linear, 0.5f32),
        (
            FadeCurve::EqualPower,
            (std::f64::consts::FRAC_PI_4).sin() as f32,
        ),
        (FadeCurve::Exponential, 0.25),
    ] {
        let (bytes, decoded) = constant_decoded(frames, 1.0, 44100);
        let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, frames as u64);
        let mut spec = edits();
        spec.fade_in = Some(FadeSpec {
            length_frames: 10,
            curve: Some(curve),
        });
        spec.fade_out = Some(FadeSpec {
            length_frames: 10,
            curve: Some(curve),
        });
        reference.edits = Some(spec);
        let prepared = prepare(&reference, &decoded, 44100).unwrap();
        let plane = &prepared.channels[0];
        assert_eq!(plane[0], 0.0, "fade-in starts at zero ({curve:?})");
        assert!((plane[5] - mid_gain).abs() < 1e-6, "{curve:?} fade-in mid");
        assert!(
            (plane[frames - 1]).abs() < 1e-7,
            "{curve:?} fade-out ends at zero"
        );
        assert!(
            (plane[frames - 6] - mid_gain).abs() < 1e-6,
            "{curve:?} fade-out mid"
        );
    }
}

#[test]
fn fade_defaults_to_linear_curve() {
    let (bytes, decoded) = constant_decoded(20, 1.0, 44100);
    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 20);
    let mut spec = edits();
    spec.fade_in = Some(FadeSpec {
        length_frames: 10,
        curve: None,
    });
    reference.edits = Some(spec);
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    assert!((prepared.channels[0][5] - 0.5).abs() < 1e-6);
}

#[test]
fn src_converts_rate_and_frame_count() {
    let (bytes, decoded) = constant_decoded(4800, 0.5, 48000);
    let reference = sample_ref_for(&bytes, SampleFormat::Wav, 48000, 1, 4800);
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    assert_eq!(prepared.sample_rate, 44100);
    assert_eq!(prepared.frames(), 4410);
    let mid = &prepared.channels[0][100..4310];
    assert!(mid.iter().all(|&s| (s - 0.5).abs() < 0.01));
}

#[test]
fn loop_points_remap_through_trim_and_src() {
    let frames = 4800;
    let samples: Vec<f32> = (0..frames)
        .map(|i| (i as f32 / frames as f32) - 0.5)
        .collect();
    let bytes = build_wav(1, 48000, 32, true, &samples, Some((1200, 3600)));
    let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
    assert!(decoded.loop_points.is_some());

    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 48000, 1, 4800);
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    let lp = prepared.loop_points.unwrap();
    assert_eq!(lp.start_frame, 1103); // 1200 * 44100/48000 rounded
    assert_eq!(lp.end_frame, 3308);

    let mut spec = edits();
    spec.start_frame = Some(1200);
    spec.end_frame = Some(3600);
    reference.edits = Some(spec);
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    let lp = prepared.loop_points.unwrap();
    assert_eq!(lp.start_frame, 0);
    assert_eq!(lp.end_frame, 2205);

    let mut spec = edits();
    spec.start_frame = Some(3600);
    spec.end_frame = Some(4800);
    reference.edits = Some(spec);
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    assert!(
        prepared.loop_points.is_none(),
        "loop fully trimmed must drop"
    );
}

#[test]
fn musical_length_beats_passes_through() {
    let (bytes, decoded) = constant_decoded(16, 0.5, 44100);
    let mut reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 16);
    reference.musical_length_beats = Some(Beat::new(4, 1).unwrap());
    let prepared = prepare(&reference, &decoded, 44100).unwrap();
    assert_eq!(
        prepared.musical_length_beats,
        Some(Beat::new(4, 1).unwrap())
    );
}
