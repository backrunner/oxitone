//! Persistent import cache. All decoding and filesystem work is control-thread only.
use std::path::Path;

use oxitone_core::wire::{
    CacheSampleRequest, CachedSampleInfo, SampleCacheEncoding, SampleFormat, SampleProvenance,
};
use oxitone_core::{version::check_protocol_version, OxitoneError, PROTOCOL_VERSION};

use crate::inspect::{read_sample, sample_info, validate_path};

/// Publish normalized PCM under its WAV content hash, leaving the source untouched.
pub fn cache_sample(request: &CacheSampleRequest) -> Result<CachedSampleInfo, OxitoneError> {
    check_protocol_version(&request.protocol_version)?;
    validate_path(&request.path, "path")?;
    validate_path(&request.cache_dir, "cacheDir")?;
    let decoded = read_sample(Path::new(&request.path))?;
    let info = sample_info(&decoded);
    let header = super::cache_wav::header(&decoded).map_err(|mut error| {
        error.path = Some(request.path.clone());
        error
    })?;
    let (path, sha256) = super::cache_file::publish(Path::new(&request.cache_dir), |writer| {
        super::cache_wav::write(writer, &decoded, &header)
    })?;
    Ok(CachedSampleInfo {
        protocol_version: PROTOCOL_VERSION.into(),
        path: path.display().to_string(),
        sha256,
        format: SampleFormat::Wav,
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        provenance: SampleProvenance {
            source_sha256: info.sha256,
            source_format: info.format,
            source_sample_rate: decoded.metadata.source_sample_rate,
            source_channels: info.source_channels,
            source_bit_depth: info.source_bit_depth,
            decoder: info.decoder,
            channel_layout_action: info.channel_layout_action,
            cache_encoding: Some(SampleCacheEncoding::WavF32V1),
        },
    })
}

#[cfg(test)]
mod tests;
