//! Offline decode of compressed assets (FLAC, MP3, MP4/M4A audio tracks) via
//! symphonia. This is the "import → cached PCM" path of
//! `06-format-and-export.md` §Sample 导入策略; the realtime engine never sees
//! compressed bytes.

use std::io::Cursor;

use oxitone_core::error::codes;
use oxitone_core::OxitoneError;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::decode::{asset_err, deinterleave, unsupported};
use crate::types::{ChannelLayoutAction, DecodedSample, SampleMetadata};

/// Pinned symphonia version recorded in sample metadata; keep in sync with
/// the dependency in `crates/samples/Cargo.toml`.
const SYMPHONIA_VERSION: &str = "0.5";

fn corrupt(message: impl Into<String>) -> OxitoneError {
    asset_err(codes::ASSET_UNAVAILABLE, message)
}

pub(crate) fn decode_compressed(bytes: &[u8]) -> Result<DecodedSample, OxitoneError> {
    let source = Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| unsupported(format!("unrecognized compressed audio container: {e}")))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| corrupt("container has no audio track"))?;
    let track_id = track.id;
    let codec = track.codec_params.codec;
    let decoder_name = symphonia::default::get_codecs()
        .get_codec(codec)
        .map(|d| d.short_name)
        .unwrap_or("unknown");
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| unsupported(format!("unsupported codec in container: {e}")))?;

    let mut interleaved: Vec<f32> = Vec::new();
    let mut channels = 0usize;
    let mut sample_rate = 0u32;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(SymphoniaError::IoError(_)) => break,
            Err(e) => return Err(corrupt(format!("container demux failed: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(audio) => audio,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(corrupt(format!("codec decode failed: {e}"))),
        };
        let spec = *audio.spec();
        if channels == 0 {
            channels = spec.channels.count();
            sample_rate = spec.rate;
        } else if spec.channels.count() != channels || spec.rate != sample_rate {
            return Err(corrupt(
                "stream changes channel count or sample rate mid-file",
            ));
        }
        let mut buffer = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
        buffer.copy_interleaved_ref(audio);
        interleaved.extend_from_slice(buffer.samples());
    }
    if channels == 0 {
        return Err(corrupt("compressed asset decoded to zero frames"));
    }
    Ok(DecodedSample {
        channels: deinterleave(&interleaved, channels),
        sample_rate,
        loop_points: None,
        metadata: SampleMetadata {
            sha256: String::new(),
            format: oxitone_core::wire::SampleFormat::Flac,
            source_sample_rate: sample_rate,
            source_channels: channels as u8,
            source_bit_depth: None,
            channel_layout_action: ChannelLayoutAction::Kept,
            decoder: Some(format!("symphonia {SYMPHONIA_VERSION}/{decoder_name}")),
        },
    })
}
