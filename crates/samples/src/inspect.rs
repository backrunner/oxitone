//! Read-only file inspection on the control thread. Reuses the prepare decoders.
use std::path::Path;

use oxitone_core::wire::{
    InspectSampleRequest, SampleChannelLayoutAction, SampleFormat, SampleInfo,
};
use oxitone_core::{codes, version::check_protocol_version, OxitoneError, PROTOCOL_VERSION};

use crate::{decode_bytes, ChannelLayoutAction, DecodedSample};

/// Inspect an asset without creating a graph, writing a cache, or returning PCM.
pub fn inspect_sample(request: &InspectSampleRequest) -> Result<SampleInfo, OxitoneError> {
    check_protocol_version(&request.protocol_version)?;
    validate_path(&request.path, "path")?;
    let decoded = read_sample(Path::new(&request.path))?;
    Ok(sample_info(&decoded))
}

pub(crate) fn validate_path(value: &str, field: &str) -> Result<(), OxitoneError> {
    if value.is_empty() || value.contains('\0') {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "sample path must be nonempty and contain no NUL",
            field,
        ));
    }
    Ok(())
}

pub(crate) fn read_sample(path: &Path) -> Result<DecodedSample, OxitoneError> {
    let bytes = std::fs::read(path).map_err(|error| {
        OxitoneError::with_path(
            codes::ASSET_UNAVAILABLE,
            format!("failed to read sample: {error}"),
            path.display().to_string(),
        )
    })?;
    decode_detected(&bytes, path).map_err(|mut error| {
        error.path = Some(path.display().to_string());
        error
    })
}

fn decode_detected(bytes: &[u8], path: &Path) -> Result<DecodedSample, OxitoneError> {
    let format = detect_format(bytes, path).ok_or_else(|| {
        OxitoneError::new(
            codes::SAMPLE_FORMAT_UNSUPPORTED,
            "unrecognized sample container (WAV/AIFF/FLAC/MP3/MP4/M4A required)",
        )
    })?;
    let decoded = decode_bytes(bytes, format).map_err(|error| {
        OxitoneError::new(
            codes::SAMPLE_FORMAT_UNSUPPORTED,
            format!("failed to decode {:?} sample: {}", format, error.message),
        )
    })?;
    if decoded.frames() == 0
        || decoded.sample_rate == 0
        || ![1, 2].contains(&decoded.channels.len())
    {
        return Err(OxitoneError::new(
            codes::SAMPLE_FORMAT_UNSUPPORTED,
            "sample has no usable audio frames",
        ));
    }
    Ok(decoded)
}

pub(crate) fn sample_info(decoded: &DecodedSample) -> SampleInfo {
    let format = decoded.metadata.format;
    let decoder = decoded
        .metadata
        .decoder
        .clone()
        .unwrap_or_else(|| match format {
            SampleFormat::Wav => "oxitone-wav-v1".into(),
            SampleFormat::Aiff => "oxitone-aiff-v1".into(),
            _ => "symphonia-0.5".into(),
        });
    SampleInfo {
        protocol_version: PROTOCOL_VERSION.into(),
        sha256: decoded.metadata.sha256.clone(),
        format,
        sample_rate: decoded.sample_rate,
        channels: decoded.channels.len() as u8,
        frames: decoded.frames(),
        source_channels: decoded.metadata.source_channels,
        source_bit_depth: decoded.metadata.source_bit_depth,
        decoder,
        channel_layout_action: match decoded.metadata.channel_layout_action {
            ChannelLayoutAction::Kept => SampleChannelLayoutAction::Kept,
            ChannelLayoutAction::DownmixedToStereo => SampleChannelLayoutAction::DownmixedToStereo,
        },
    }
}

#[cfg(test)]
fn inspect_bytes(bytes: &[u8], path: &Path) -> Result<SampleInfo, OxitoneError> {
    decode_detected(bytes, path).map(|decoded| sample_info(&decoded))
}

fn detect_format(bytes: &[u8], path: &Path) -> Option<SampleFormat> {
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        Some(SampleFormat::Wav)
    } else if bytes.starts_with(b"FORM") && matches!(bytes.get(8..12), Some(b"AIFF" | b"AIFC")) {
        Some(SampleFormat::Aiff)
    } else if bytes.starts_with(b"fLaC") {
        Some(SampleFormat::Flac)
    } else if bytes.get(4..8) == Some(b"ftyp") {
        let m4a = path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("m4a"))
            || bytes.get(8..12) == Some(b"M4A ");
        Some(if m4a {
            SampleFormat::M4a
        } else {
            SampleFormat::Mp4
        })
    } else if bytes.starts_with(b"ID3")
        || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
    {
        Some(SampleFormat::Mp3)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
