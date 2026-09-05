use super::*;
use crate::fixtures::{build_aiff, build_wav};

#[test]
fn detects_container_without_trusting_extension_and_reports_original_hash() {
    for (bytes, format, decoder) in [
        (
            build_wav(1, 44100, 24, false, &[0.0, 0.5, -0.5], None),
            SampleFormat::Wav,
            "oxitone-wav-v1",
        ),
        (
            build_aiff(1, 44100, 24, &[0.0, 0.5, -0.5]),
            SampleFormat::Aiff,
            "oxitone-aiff-v1",
        ),
    ] {
        let info = inspect_bytes(&bytes, Path::new("wrong.mp3")).unwrap();
        assert_eq!(info.format, format);
        assert_eq!(info.sha256, crate::sha256_hex(&bytes));
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.frames, 3);
        assert_eq!(info.channels, 1);
        assert_eq!(info.source_channels, 1);
        assert_eq!(info.source_bit_depth, Some(24));
        assert_eq!(info.decoder, decoder);
        assert_eq!(info.channel_layout_action, SampleChannelLayoutAction::Kept);
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["frames"], "3");
        assert_eq!(json["protocolVersion"], PROTOCOL_VERSION);
    }
}

#[test]
fn reports_downmixed_dimensions_and_source_provenance() {
    let bytes = build_wav(6, 48000, 32, true, &[0.25; 60], None);
    let info = inspect_bytes(&bytes, Path::new("surround.wav")).unwrap();
    assert_eq!(info.channels, 2);
    assert_eq!(info.source_channels, 6);
    assert_eq!(info.frames, 10);
    assert_eq!(info.source_bit_depth, Some(32));
    assert_eq!(
        info.channel_layout_action,
        SampleChannelLayoutAction::DownmixedToStereo
    );
}

#[test]
fn rejects_unknown_truncated_and_empty_audio() {
    for bytes in [
        b"not an audio file".to_vec(),
        b"RIFF\0\0\0\0WAVE".to_vec(),
        build_wav(1, 48000, 16, false, &[], None),
        b"fLaC".to_vec(),
        b"ID3".to_vec(),
        b"\0\0\0\x14ftypM4A \0\0\0\0M4A ".to_vec(),
    ] {
        assert_eq!(
            inspect_bytes(&bytes, Path::new("bad.wav"))
                .unwrap_err()
                .code,
            codes::SAMPLE_FORMAT_UNSUPPORTED
        );
    }
}

#[test]
fn checks_protocol_and_path_before_io() {
    let mut request = InspectSampleRequest {
        protocol_version: "99.0".into(),
        path: String::new(),
    };
    assert_eq!(
        inspect_sample(&request).unwrap_err().code,
        codes::PROTOCOL_VERSION_UNSUPPORTED
    );
    request.protocol_version = PROTOCOL_VERSION.into();
    for path in ["", "bad\0path"] {
        request.path = path.into();
        let error = inspect_sample(&request).unwrap_err();
        assert_eq!(error.code, codes::INVALID_PROJECT);
        assert_eq!(error.path.as_deref(), Some("path"));
    }
    request.path = std::env::temp_dir()
        .join(format!("oxitone-missing-{}/asset.wav", std::process::id()))
        .display()
        .to_string();
    let error = inspect_sample(&request).unwrap_err();
    assert_eq!(error.code, codes::ASSET_UNAVAILABLE);
    assert_eq!(error.path.as_ref(), Some(&request.path));
}

#[test]
fn detects_compressed_container_signatures() {
    for (bytes, name, expected) in [
        (&b"fLaC"[..], "asset", SampleFormat::Flac),
        (&b"ID3"[..], "asset", SampleFormat::Mp3),
        (&b"\xff\xfb"[..], "asset", SampleFormat::Mp3),
        (&b"\0\0\0\x14ftypM4A "[..], "asset", SampleFormat::M4a),
        (&b"\0\0\0\x14ftypisom"[..], "asset.M4A", SampleFormat::M4a),
        (&b"\0\0\0\x14ftypisom"[..], "asset", SampleFormat::Mp4),
    ] {
        assert_eq!(detect_format(bytes, Path::new(name)), Some(expected));
    }
}
