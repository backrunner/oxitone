use oxitone_core::error::codes;
use oxitone_core::wire::SampleFormat;

use crate::fixtures::{build_aiff, build_wav, sample_ref_for, sine_interleaved};
use crate::{decode_asset, decode_bytes, ChannelLayoutAction, SAMPLE_FORMAT_UNSUPPORTED};

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tolerance {tolerance})"
    );
}

#[test]
fn wav_pcm16_stereo_roundtrip() {
    let samples = sine_interleaved(128, 2, 44100, 440.0);
    let bytes = build_wav(2, 44100, 16, false, &samples, None);
    let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
    assert_eq!(decoded.sample_rate, 44100);
    assert_eq!(decoded.channels.len(), 2);
    assert_eq!(decoded.frames(), 128);
    assert_eq!(decoded.metadata.source_bit_depth, Some(16));
    assert_eq!(
        decoded.metadata.channel_layout_action,
        ChannelLayoutAction::Kept
    );
    for ch in 0..2 {
        for i in 0..128 {
            assert_close(
                decoded.channels[ch][i],
                samples[i * 2 + ch],
                1.0 / 32768.0 + 1e-7,
            );
        }
    }
}

#[test]
fn wav_pcm_depths_roundtrip() {
    let samples = sine_interleaved(64, 1, 48000, 220.0);
    let cases: [(u16, f32); 3] = [(8, 1.0 / 128.0), (24, 1.0 / 8_388_608.0), (32, 1e-6)];
    for (bits, tolerance) in cases {
        let bytes = build_wav(1, 48000, bits, false, &samples, None);
        let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
        assert_eq!(decoded.metadata.source_bit_depth, Some(bits as u8));
        for (i, &expected) in samples.iter().enumerate() {
            assert_close(decoded.channels[0][i], expected, tolerance + 1e-6);
        }
    }
}

#[test]
fn wav_float32_and_float64_roundtrip() {
    let samples = sine_interleaved(64, 1, 44100, 330.0);
    for bits in [32u16, 64] {
        let bytes = build_wav(1, 44100, bits, true, &samples, None);
        let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
        assert_eq!(decoded.metadata.source_bit_depth, Some(bits as u8));
        for (i, &expected) in samples.iter().enumerate() {
            assert_close(decoded.channels[0][i], expected, 1e-6);
        }
    }
}

#[test]
fn wav_smpl_loop_points() {
    let samples = sine_interleaved(512, 1, 44100, 440.0);
    let bytes = build_wav(1, 44100, 16, false, &samples, Some((64, 320)));
    let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
    let lp = decoded.loop_points.expect("loop points");
    assert_eq!(lp.start_frame, 64);
    assert_eq!(lp.end_frame, 320);
}

#[test]
fn wav_multichannel_downmixes_to_stereo() {
    let frames = 32;
    let mut samples = vec![0.0f32; frames * 4];
    for i in 0..frames {
        samples[i * 4] = 0.5; // L
        samples[i * 4 + 1] = -0.5; // R
        samples[i * 4 + 2] = 0.25; // C → folds into L
        samples[i * 4 + 3] = 0.25; // LFE → folds into R
    }
    let bytes = build_wav(4, 44100, 32, true, &samples, None);
    let decoded = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
    assert_eq!(decoded.channels.len(), 2);
    assert_eq!(decoded.metadata.source_channels, 4);
    assert_eq!(
        decoded.metadata.channel_layout_action,
        ChannelLayoutAction::DownmixedToStereo
    );
    let fold = 0.25 * std::f32::consts::FRAC_1_SQRT_2;
    assert_close(decoded.channels[0][0], 0.5 + fold, 1e-6);
    assert_close(decoded.channels[1][0], -0.5 + fold, 1e-6);
}

#[test]
fn wav_truncated_chunk_is_asset_unavailable() {
    let mut bytes = build_wav(
        1,
        44100,
        16,
        false,
        &sine_interleaved(16, 1, 44100, 440.0),
        None,
    );
    bytes.truncate(bytes.len() - 10);
    let err = decode_bytes(&bytes, SampleFormat::Wav).unwrap_err();
    assert_eq!(err.code, codes::ASSET_UNAVAILABLE);
}

#[test]
fn wav_unsupported_format_tag_is_rejected() {
    let mut bytes = build_wav(
        1,
        44100,
        16,
        false,
        &sine_interleaved(16, 1, 44100, 440.0),
        None,
    );
    bytes[20] = 6; // µ-law format tag
    let err = decode_bytes(&bytes, SampleFormat::Wav).unwrap_err();
    assert_eq!(err.code, SAMPLE_FORMAT_UNSUPPORTED);
}

#[test]
fn wav_garbage_is_rejected() {
    let err = decode_bytes(b"not audio at all", SampleFormat::Wav).unwrap_err();
    assert_eq!(err.code, SAMPLE_FORMAT_UNSUPPORTED);
}

#[test]
fn aiff_pcm16_roundtrip() {
    let samples = sine_interleaved(96, 2, 48000, 440.0);
    let bytes = build_aiff(2, 48000, 16, &samples);
    let decoded = decode_bytes(&bytes, SampleFormat::Aiff).unwrap();
    assert_eq!(decoded.sample_rate, 48000);
    assert_eq!(decoded.channels.len(), 2);
    assert_eq!(decoded.frames(), 96);
    for ch in 0..2 {
        for i in 0..96 {
            assert_close(
                decoded.channels[ch][i],
                samples[i * 2 + ch],
                1.0 / 32767.0 + 1e-7,
            );
        }
    }
}

#[test]
fn aiff_pcm24_mono_roundtrip() {
    let samples = sine_interleaved(64, 1, 44100, 110.0);
    let bytes = build_aiff(1, 44100, 24, &samples);
    let decoded = decode_bytes(&bytes, SampleFormat::Aiff).unwrap();
    assert_eq!(decoded.sample_rate, 44100);
    for (i, &expected) in samples.iter().enumerate() {
        assert_close(decoded.channels[0][i], expected, 1.0 / 8_388_607.0 + 1e-6);
    }
}

#[test]
fn extended80_roundtrip_common_rates() {
    for rate in [8000.0, 22050.0, 44100.0, 48000.0, 96000.0] {
        let parsed = super::aiff::parse_extended80(&crate::fixtures::extended80(rate));
        assert_close(parsed as f32, rate as f32, 0.5);
    }
}

#[test]
fn aiff_wrong_magic_is_rejected() {
    let bytes = build_wav(
        1,
        44100,
        16,
        false,
        &sine_interleaved(16, 1, 44100, 440.0),
        None,
    );
    let err = decode_bytes(&bytes, SampleFormat::Aiff).unwrap_err();
    assert_eq!(err.code, SAMPLE_FORMAT_UNSUPPORTED);
}

#[test]
fn decode_asset_verifies_sha256() {
    let samples = sine_interleaved(64, 1, 44100, 440.0);
    let bytes = build_wav(1, 44100, 16, false, &samples, None);
    let reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 64);
    let decoded = decode_asset(&bytes, &reference).unwrap();
    assert_eq!(decoded.metadata.sha256, reference.sha256);
    assert_eq!(decoded.metadata.format, SampleFormat::Wav);

    let mut bad = reference.clone();
    bad.sha256 = "00".repeat(32);
    let err = decode_asset(&bytes, &bad).unwrap_err();
    assert_eq!(err.code, codes::ASSET_UNAVAILABLE);
    assert_eq!(err.path.as_deref(), Some(bad.asset_uri.as_str()));
    assert!(err.message.contains(&reference.sha256));
}

#[test]
fn compressed_garbage_is_rejected_with_stable_code() {
    let err = decode_bytes(b"definitely not flac", SampleFormat::Flac).unwrap_err();
    assert_eq!(err.code, SAMPLE_FORMAT_UNSUPPORTED);
    let err = decode_bytes(b"definitely not mp4", SampleFormat::M4a).unwrap_err();
    assert_eq!(err.code, SAMPLE_FORMAT_UNSUPPORTED);
}

#[test]
fn aac_m4a_roundtrip_via_afconvert() {
    if !std::path::Path::new("/usr/bin/afconvert").exists() {
        eprintln!("afconvert unavailable; skipping m4a fixture test");
        return;
    }
    let dir = std::env::temp_dir().join(format!("oxitone-samples-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav_path = dir.join("fixture.wav");
    let m4a_path = dir.join("fixture.m4a");
    let samples = sine_interleaved(8192, 2, 44100, 440.0);
    std::fs::write(&wav_path, build_wav(2, 44100, 16, false, &samples, None)).unwrap();
    let status = std::process::Command::new("afconvert")
        .args(["-f", "m4af", "-d", "aac", "-b", "128000"])
        .arg(&wav_path)
        .arg(&m4a_path)
        .status()
        .unwrap();
    assert!(status.success(), "afconvert failed");
    let bytes = std::fs::read(&m4a_path).unwrap();
    let decoded = decode_bytes(&bytes, SampleFormat::M4a).unwrap();
    assert_eq!(decoded.sample_rate, 44100);
    assert_eq!(decoded.channels.len(), 2);
    assert!(decoded.frames() >= 8192, "AAC padding should not shorten");
    let decoder = decoded.metadata.decoder.expect("decoder recorded");
    assert!(decoder.contains("aac"), "unexpected decoder {decoder}");
    assert!(decoder.starts_with("symphonia 0.5/"));
    assert_eq!(decoded.metadata.sha256, crate::sha256_hex(&bytes));

    let skip = 2048 + 64; // encoder priming + decoder delay
    let plane = &decoded.channels[0];
    let rms: f32 = plane
        .iter()
        .skip(skip)
        .take(4096)
        .map(|s| s * s)
        .sum::<f32>()
        / 4096.0;
    assert!(
        rms.sqrt() > 0.2,
        "decoded AAC too quiet: rms {}",
        rms.sqrt()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
