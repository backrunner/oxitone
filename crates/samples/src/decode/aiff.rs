//! AIFF/AIFC decoder: PCM 8/16/24/32-bit big-endian (`AIFC` also accepts
//! `sowt` little-endian), with 80-bit IEEE 754 extended sample rates.

use oxitone_core::error::codes;
use oxitone_core::wire::SampleFormat;
use oxitone_core::OxitoneError;

use crate::decode::{asset_err, unsupported};
use crate::types::{ChannelLayoutAction, DecodedSample, SampleMetadata};

fn be_u16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

fn be_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// 80-bit extended: 1 sign bit, 15-bit biased exponent, 64-bit mantissa with
/// explicit integer bit.
pub(crate) fn parse_extended80(b: &[u8]) -> f64 {
    let sign = if b[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    let exponent = (((b[0] & 0x7F) as u32) << 8) | b[1] as u32;
    let mantissa = u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]);
    if exponent == 0 && mantissa == 0 {
        return 0.0;
    }
    sign * (mantissa as f64) * 2f64.powi(exponent as i32 - 16383 - 63)
}

struct Comm {
    channels: usize,
    frames: u64,
    bits: u16,
    sample_rate: u32,
    little_endian: bool,
}

fn parse_comm(body: &[u8], aifc: bool) -> Result<Comm, OxitoneError> {
    if body.len() < 18 {
        return Err(asset_err(
            codes::ASSET_UNAVAILABLE,
            "truncated AIFF COMM chunk",
        ));
    }
    let channels = be_u16(&body[0..2]) as usize;
    let frames = be_u32(&body[2..6]) as u64;
    let bits = be_u16(&body[6..8]);
    let rate = parse_extended80(&body[8..18]);
    if channels == 0 || !(rate.is_finite() && rate > 0.0) {
        return Err(asset_err(
            codes::ASSET_UNAVAILABLE,
            "AIFF COMM chunk with zero channels or invalid sample rate",
        ));
    }
    if !matches!(bits, 8 | 16 | 24 | 32) {
        return Err(unsupported(format!("unsupported AIFF bit depth {bits}")));
    }
    let mut little_endian = false;
    if aifc {
        if body.len() < 22 {
            return Err(asset_err(
                codes::ASSET_UNAVAILABLE,
                "truncated AIFC COMM chunk",
            ));
        }
        match &body[18..22] {
            b"NONE" => {}
            b"sowt" => little_endian = true,
            other => {
                return Err(unsupported(format!(
                    "unsupported AIFC compression {:?}",
                    String::from_utf8_lossy(other)
                )))
            }
        }
    }
    Ok(Comm {
        channels,
        frames,
        bits,
        sample_rate: rate.round() as u32,
        little_endian,
    })
}

fn decode_samples(comm: &Comm, data: &[u8]) -> Vec<Vec<f32>> {
    let bytes_per = (comm.bits / 8) as usize;
    let frames = (data.len() / (bytes_per * comm.channels)).min(comm.frames as usize);
    let read = |b: &[u8]| -> i32 {
        let v = match bytes_per {
            1 => b[0] as i8 as i32,
            2..=4 => {
                let mut raw = [0u8; 4];
                for i in 0..bytes_per {
                    raw[i] = if comm.little_endian {
                        b[bytes_per - 1 - i]
                    } else {
                        b[i]
                    };
                }
                i32::from_be_bytes(raw) >> (8 * (4 - bytes_per))
            }
            _ => unreachable!("bits validated in parse_comm"),
        };
        v
    };
    let scale = (1u64 << (comm.bits - 1)) as f32;
    let mut out = vec![vec![0.0f32; frames]; comm.channels];
    for frame in 0..frames {
        for (ch, plane) in out.iter_mut().enumerate() {
            let base = (frame * comm.channels + ch) * bytes_per;
            plane[frame] = read(&data[base..base + bytes_per]) as f32 / scale;
        }
    }
    out
}

pub(crate) fn decode_aiff(bytes: &[u8]) -> Result<DecodedSample, OxitoneError> {
    if bytes.len() < 12 || &bytes[0..4] != b"FORM" {
        return Err(unsupported("not an AIFF file"));
    }
    let aifc = match &bytes[8..12] {
        b"AIFF" => false,
        b"AIFC" => true,
        _ => return Err(unsupported("not an AIFF file")),
    };
    let mut comm: Option<Comm> = None;
    let mut ssnd: Option<&[u8]> = None;
    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = be_u32(&bytes[offset + 4..offset + 8]) as usize;
        let end = offset + 8 + size;
        if end > bytes.len() {
            return Err(asset_err(
                codes::ASSET_UNAVAILABLE,
                format!(
                    "truncated AIFF chunk {:?} at offset {offset}",
                    String::from_utf8_lossy(id)
                ),
            ));
        }
        match id {
            b"COMM" => comm = Some(parse_comm(&bytes[offset + 8..end], aifc)?),
            b"SSND" => {
                if size < 8 {
                    return Err(asset_err(
                        codes::ASSET_UNAVAILABLE,
                        "truncated AIFF SSND chunk",
                    ));
                }
                let skip = 8 + be_u32(&bytes[offset + 8..offset + 12]) as usize;
                if offset + 8 + skip > end {
                    return Err(asset_err(
                        codes::ASSET_UNAVAILABLE,
                        "AIFF SSND offset out of range",
                    ));
                }
                ssnd = Some(&bytes[offset + 8 + skip..end]);
            }
            _ => {}
        }
        offset = end + (size & 1);
    }
    let comm =
        comm.ok_or_else(|| asset_err(codes::ASSET_UNAVAILABLE, "AIFF file without COMM chunk"))?;
    let ssnd =
        ssnd.ok_or_else(|| asset_err(codes::ASSET_UNAVAILABLE, "AIFF file without SSND chunk"))?;
    Ok(DecodedSample {
        channels: decode_samples(&comm, ssnd),
        sample_rate: comm.sample_rate,
        loop_points: None,
        metadata: SampleMetadata {
            sha256: String::new(),
            format: SampleFormat::Aiff,
            source_sample_rate: comm.sample_rate,
            source_channels: comm.channels as u8,
            source_bit_depth: Some(comm.bits as u8),
            channel_layout_action: ChannelLayoutAction::Kept,
            decoder: None,
        },
    })
}
