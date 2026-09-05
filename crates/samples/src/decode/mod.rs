//! Format dispatch, content-hash verification, and channel-layout handling.

mod aiff;
mod compressed;
#[cfg(test)]
mod tests;
mod wav;

use std::path::Path;

use oxitone_core::error::codes;
use oxitone_core::wire::{SampleFormat, SampleRef};
use oxitone_core::OxitoneError;
use sha2::{Digest, Sha256};

use crate::types::{ChannelLayoutAction, DecodedSample};
use crate::SAMPLE_FORMAT_UNSUPPORTED;

pub(crate) fn asset_err(code: &'static str, message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(code, message)
}

pub(crate) fn unsupported(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(SAMPLE_FORMAT_UNSUPPORTED, message)
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Decode `bytes` as `format` into non-interleaved `f32`. No hash check; use
/// [`decode_asset`] when a [`SampleRef`] with a declared hash is available.
///
/// Sources with more than two channels are downmixed to stereo (see
/// [`ChannelLayoutAction`]); the `channels: 1|2` contract is enforced here.
pub fn decode_bytes(bytes: &[u8], format: SampleFormat) -> Result<DecodedSample, OxitoneError> {
    decode_impl(bytes, format)
}

/// Verify the SHA-256 content hash declared by `sample_ref`, then decode.
/// A mismatch reports `AssetUnavailable` with the asset URI and format.
pub fn decode_asset(bytes: &[u8], sample_ref: &SampleRef) -> Result<DecodedSample, OxitoneError> {
    let actual = sha256_hex(bytes);
    if !actual.eq_ignore_ascii_case(&sample_ref.sha256) {
        return Err(OxitoneError::with_path(
            codes::ASSET_UNAVAILABLE,
            format!(
                "content hash mismatch for {:?} asset: expected {}, got {actual}",
                sample_ref.format, sample_ref.sha256
            ),
            sample_ref.asset_uri.clone(),
        ));
    }
    decode_impl(bytes, sample_ref.format)
        .map_err(|e| OxitoneError::with_path(e.code, e.message, sample_ref.asset_uri.clone()))
}

/// Read `path` and decode against `sample_ref`. I/O failure reports
/// `AssetUnavailable` with the asset path.
pub fn load_asset(path: &Path, sample_ref: &SampleRef) -> Result<DecodedSample, OxitoneError> {
    let bytes = std::fs::read(path).map_err(|e| {
        OxitoneError::with_path(
            codes::ASSET_UNAVAILABLE,
            format!("failed to read asset: {e}"),
            path.display().to_string(),
        )
    })?;
    decode_asset(&bytes, sample_ref)
}

fn decode_impl(bytes: &[u8], format: SampleFormat) -> Result<DecodedSample, OxitoneError> {
    let mut decoded = match format {
        SampleFormat::Wav => wav::decode_wav(bytes)?,
        SampleFormat::Aiff => aiff::decode_aiff(bytes)?,
        SampleFormat::Flac | SampleFormat::Mp3 | SampleFormat::Mp4 | SampleFormat::M4a => {
            compressed::decode_compressed(bytes)?
        }
    };
    decoded.metadata.sha256 = sha256_hex(bytes);
    decoded.metadata.format = format;
    let channels = decoded.channels.len();
    if channels > 2 {
        decoded.channels = downmix_to_stereo(&decoded.channels);
        decoded.metadata.channel_layout_action = ChannelLayoutAction::DownmixedToStereo;
    }
    Ok(decoded)
}

/// Fold `channels` (> 2, interleaved-planar) down to stereo: channels 0/1 map
/// to L/R at unity gain, remaining channels fold alternately into L/R at
/// 1/√2 (center and surrounds, standard import downmix).
pub(crate) fn downmix_to_stereo(channels: &[Vec<f32>]) -> Vec<Vec<f32>> {
    const FOLD: f32 = 0.707_106_77;
    let frames = channels[0].len();
    let mut left = channels[0].clone();
    let mut right = channels
        .get(1)
        .cloned()
        .unwrap_or_else(|| vec![0.0; frames]);
    for (index, channel) in channels.iter().enumerate().skip(2) {
        let target = if index % 2 == 0 {
            &mut left
        } else {
            &mut right
        };
        for (dst, &src) in target.iter_mut().zip(channel.iter()) {
            *dst += src * FOLD;
        }
    }
    vec![left, right]
}

/// Split interleaved samples into per-channel planes.
pub(crate) fn deinterleave(interleaved: &[f32], channels: usize) -> Vec<Vec<f32>> {
    let frames = interleaved.len() / channels;
    let mut out = vec![vec![0.0f32; frames]; channels];
    for (i, &sample) in interleaved[..frames * channels].iter().enumerate() {
        out[i % channels][i / channels] = sample;
    }
    out
}
