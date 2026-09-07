//! RIFF/WAVE decoder: PCM 8/16/24/32-bit integer, 32/64-bit IEEE float,
//! WAVE_FORMAT_EXTENSIBLE, and `smpl` chunk loop points.

use oxitone_core::OxitoneError;

use crate::decode::{asset_err, unsupported};
use crate::types::{ChannelLayoutAction, DecodedSample, LoopPoints, SampleMetadata};

use oxitone_core::error::codes;

const TAG_PCM: u16 = 0x0001;
const TAG_FLOAT: u16 = 0x0003;
const TAG_EXTENSIBLE: u16 = 0xFFFE;
const GUID_TAIL: [u8; 14] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

struct Fmt {
    tag: u16,
    channels: usize,
    sample_rate: u32,
    bits: u16,
}

fn le_u16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn parse_fmt(body: &[u8]) -> Result<Fmt, OxitoneError> {
    if body.len() < 16 {
        return Err(asset_err(
            codes::ASSET_UNAVAILABLE,
            "truncated WAV fmt chunk",
        ));
    }
    let mut tag = le_u16(&body[0..2]);
    let channels = le_u16(&body[2..4]) as usize;
    let sample_rate = le_u32(&body[4..8]);
    let bits = le_u16(&body[14..16]);
    if channels == 0 || sample_rate == 0 {
        return Err(asset_err(
            codes::ASSET_UNAVAILABLE,
            "WAV fmt chunk with zero channels or sample rate",
        ));
    }
    if tag == TAG_EXTENSIBLE {
        if body.len() < 40 {
            return Err(asset_err(
                codes::ASSET_UNAVAILABLE,
                "truncated WAVE_FORMAT_EXTENSIBLE fmt chunk",
            ));
        }
        let valid_bits = le_u16(&body[18..20]);
        if valid_bits == 0 || valid_bits > bits {
            return Err(unsupported("invalid WAV valid-bit count"));
        }
        // Extensible PCM is left aligned within the container. Decode using
        // container width so both stride and normalization remain correct.
        let sub = &body[24..40];
        if sub[2..] != GUID_TAIL {
            return Err(unsupported("unsupported WAVE extensible sub-format GUID"));
        }
        tag = le_u16(&sub[0..2]);
    }
    match tag {
        TAG_PCM if matches!(bits, 8 | 16 | 24 | 32) => {}
        TAG_FLOAT if matches!(bits, 32 | 64) => {}
        TAG_PCM | TAG_FLOAT => {
            return Err(unsupported(format!(
                "unsupported WAV bit depth {bits} for format tag {tag}"
            )))
        }
        other => {
            return Err(unsupported(format!(
                "unsupported WAV format tag {other:#06x}"
            )))
        }
    }
    Ok(Fmt {
        tag,
        channels,
        sample_rate,
        bits,
    })
}

fn parse_smpl(body: &[u8], frames: u64) -> Option<LoopPoints> {
    if body.len() < 36 {
        return None;
    }
    let count = le_u32(&body[28..32]) as usize;
    for i in 0..count {
        let base = 36 + i * 24;
        if base + 24 > body.len() {
            break;
        }
        if le_u32(&body[base + 4..base + 8]) != 0 {
            continue;
        }
        let start = le_u32(&body[base + 8..base + 12]) as u64;
        // RIFF smpl stores an inclusive endpoint; internal loops are half-open.
        let end = (le_u32(&body[base + 12..base + 16]) as u64 + 1).min(frames);
        if start < end {
            return Some(LoopPoints {
                start_frame: start,
                end_frame: end,
            });
        }
    }
    None
}

fn decode_samples(fmt: &Fmt, data: &[u8]) -> Vec<Vec<f32>> {
    let bytes_per = (fmt.bits / 8) as usize;
    let frames = data.len() / (bytes_per * fmt.channels);
    let mut out = vec![vec![0.0f32; frames]; fmt.channels];
    for frame in 0..frames {
        for (ch, plane) in out.iter_mut().enumerate() {
            let base = (frame * fmt.channels + ch) * bytes_per;
            let b = &data[base..base + bytes_per];
            plane[frame] = match (fmt.tag, fmt.bits) {
                (TAG_PCM, 8) => (b[0] as i32 - 128) as f32 / 128.0,
                (TAG_PCM, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
                (TAG_PCM, 24) => {
                    let v = i32::from_le_bytes([
                        b[0],
                        b[1],
                        b[2],
                        if b[2] & 0x80 != 0 { 0xFF } else { 0 },
                    ]);
                    v as f32 / 8_388_608.0
                }
                (TAG_PCM, 32) => {
                    i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0
                }
                (TAG_FLOAT, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                (TAG_FLOAT, 64) => {
                    f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32
                }
                _ => unreachable!("fmt validated in parse_fmt"),
            };
        }
    }
    out
}

pub(crate) fn decode_wav(bytes: &[u8]) -> Result<DecodedSample, OxitoneError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(unsupported("not a RIFF/WAVE file"));
    }
    let mut fmt: Option<Fmt> = None;
    let mut data: Option<&[u8]> = None;
    let mut smpl: Option<&[u8]> = None;
    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = le_u32(&bytes[offset + 4..offset + 8]) as usize;
        let end = offset + 8 + size;
        if end > bytes.len() {
            return Err(asset_err(
                codes::ASSET_UNAVAILABLE,
                format!(
                    "truncated WAV chunk {:?} at offset {offset}",
                    String::from_utf8_lossy(id)
                ),
            ));
        }
        match id {
            b"fmt " => fmt = Some(parse_fmt(&bytes[offset + 8..end])?),
            b"data" => data = Some(&bytes[offset + 8..end]),
            b"smpl" => smpl = Some(&bytes[offset + 8..end]),
            _ => {}
        }
        offset = end + (size & 1);
    }
    let fmt =
        fmt.ok_or_else(|| asset_err(codes::ASSET_UNAVAILABLE, "WAV file without fmt chunk"))?;
    let data =
        data.ok_or_else(|| asset_err(codes::ASSET_UNAVAILABLE, "WAV file without data chunk"))?;
    let channels = decode_samples(&fmt, data);
    let frames = channels[0].len() as u64;
    Ok(DecodedSample {
        channels,
        sample_rate: fmt.sample_rate,
        loop_points: smpl.and_then(|body| parse_smpl(body, frames)),
        metadata: SampleMetadata {
            sha256: String::new(),
            format: oxitone_core::wire::SampleFormat::Wav,
            source_sample_rate: fmt.sample_rate,
            source_channels: fmt.channels as u8,
            source_bit_depth: Some(fmt.bits as u8),
            channel_layout_action: ChannelLayoutAction::Kept,
            decoder: None,
        },
    })
}
