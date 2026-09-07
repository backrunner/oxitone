//! Deterministic wav-f32-v1 encoding, preserving decoded PCM and forward loop points.
use crate::DecodedSample;
use oxitone_core::{codes, OxitoneError};
use std::io::{self, Write};

pub(crate) fn header(decoded: &DecodedSample) -> Result<Vec<u8>, OxitoneError> {
    let frames = decoded.frames();
    let channels = decoded.channels.len();
    if frames == 0
        || ![1, 2].contains(&channels)
        || decoded.sample_rate == 0
        || decoded
            .channels
            .iter()
            .any(|plane| plane.len() as u64 != frames || plane.iter().any(|v| !v.is_finite()))
    {
        return Err(OxitoneError::new(
            codes::SAMPLE_FORMAT_UNSUPPORTED,
            "cache needs finite, nonempty mono/stereo PCM",
        ));
    }
    // 12 RIFF + 24 fmt + 12 fact + optional 68 smpl + 8 data.
    let header_size = 56 + if decoded.loop_points.is_some() { 68 } else { 0 };
    let data_bytes = frames
        .checked_mul(channels as u64 * 4)
        .filter(|bytes| *bytes <= u32::MAX as u64 - (header_size - 8))
        .ok_or_else(|| {
            OxitoneError::new(
                codes::WAV_TOO_LARGE,
                "normalized sample exceeds RIFF size limit",
            )
        })?;
    let byte_rate = decoded
        .sample_rate
        .checked_mul(channels as u32 * 4)
        .ok_or_else(|| {
            OxitoneError::new(
                codes::SAMPLE_FORMAT_UNSUPPORTED,
                "sample rate exceeds WAV byte-rate limit",
            )
        })?;
    let mut out = Vec::with_capacity(header_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((header_size - 8 + data_bytes) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&(channels as u16).to_le_bytes());
    out.extend_from_slice(&decoded.sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&(channels as u16 * 4).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"fact");
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&(frames as u32).to_le_bytes());
    if let Some(points) = decoded.loop_points {
        if points.start_frame >= points.end_frame || points.end_frame > frames {
            return Err(OxitoneError::new(
                codes::SAMPLE_FORMAT_UNSUPPORTED,
                "invalid sample loop points",
            ));
        }
        out.extend_from_slice(b"smpl");
        out.extend_from_slice(&60u32.to_le_bytes());
        let mut body = [0u8; 60];
        body[8..12].copy_from_slice(&(1_000_000_000 / decoded.sample_rate).to_le_bytes());
        body[12..16].copy_from_slice(&60u32.to_le_bytes());
        body[28..32].copy_from_slice(&1u32.to_le_bytes());
        body[44..48].copy_from_slice(&(points.start_frame as u32).to_le_bytes());
        body[48..52].copy_from_slice(&((points.end_frame - 1) as u32).to_le_bytes());
        out.extend_from_slice(&body);
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    Ok(out)
}

pub(crate) fn write(
    writer: &mut dyn Write,
    decoded: &DecodedSample,
    header: &[u8],
) -> io::Result<()> {
    writer.write_all(header)?;
    let mut block = [0u8; 8192];
    let channels = decoded.channels.len();
    let block_frames = block.len() / (channels * 4);
    for start in (0..decoded.frames() as usize).step_by(block_frames) {
        let count = block_frames.min(decoded.frames() as usize - start);
        for frame in 0..count {
            for (ch, plane) in decoded.channels.iter().enumerate() {
                let offset = (frame * channels + ch) * 4;
                block[offset..offset + 4].copy_from_slice(&plane[start + frame].to_le_bytes());
            }
        }
        writer.write_all(&block[..count * channels * 4])?;
    }
    Ok(())
}
