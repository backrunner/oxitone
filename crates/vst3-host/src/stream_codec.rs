//! Framing is only used by helper/IO threads. Never by the realtime port.
use crate::bus_wire::MAX_BUSES;
use crate::transport_codec::{self, BYTES as TRANSPORT_BYTES};
use crate::{
    stream_wire::{Block, MAX_EVENTS, STREAM_VERSION},
    wire::{Event, MAX_REQUEST},
};
use oxitone_core::midi_bytes::{MidiBytes, MAX_MIDI_PAYLOAD_BYTES};
use std::io::{self, Read, Write};

#[cfg(test)]
#[path = "sysex_codec_tests.rs"]
mod sysex_tests;
#[cfg(test)]
#[path = "stream_codec_tests.rs"]
mod tests;

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid VST3 stream frame")
}
#[cfg(feature = "host")]
pub(crate) fn write_fault(writer: &mut impl Write) -> io::Result<()> {
    let mut header = [0u8; 40];
    header[..4].copy_from_slice(b"OXVF");
    header[4..8].copy_from_slice(&STREAM_VERSION.to_le_bytes());
    writer.write_all(&header)
}
pub(crate) fn write_json(writer: &mut impl Write, value: &impl serde::Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_REQUEST {
        return Err(invalid());
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)
}
pub(crate) fn read_json(reader: &mut impl Read) -> io::Result<serde_json::Value> {
    let mut size = [0; 4];
    reader.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_REQUEST {
        return Err(invalid());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

// Header: magic, version, sequence, frames, events, flags, processing micros, bus count, payload bytes (40 LE bytes).
// Direction is 1=request or 2=response; bit 2 requests/acknowledges a processing-state reset.
// Processing duration is helper wall time for replies; requests require zero.
// Event: kind, offset, parameter id, packed MIDI channel/pitch, value (24 LE bytes).
pub(crate) fn write_block(
    writer: &mut impl Write,
    block: &Block,
    response: bool,
) -> io::Result<()> {
    if block.frames == 0
        || block.frames > 4096
        || block.frames > block.max_frames
        || block.bus_count > MAX_BUSES
        || (!response && block.bus_count == 0)
        || block.frames * block.bus_count * 2 > block.audio.len()
        || block.event_count > MAX_EVENTS
        || block.payload.len() > MAX_MIDI_PAYLOAD_BYTES
        || !crate::event_wire::valid_payloads(
            &block.events[..block.event_count.min(MAX_EVENTS)],
            &block.payload,
        )
        || (block.restart_required
            && (!response
                || block.audio[..2 * block.bus_count * block.frames]
                    .iter()
                    .any(|v| *v != 0.)))
    {
        return Err(invalid());
    }
    let events = block.event_count;
    if response
        && (block.restart_required && (events != 0 || !block.payload.is_empty())
            || block.events[..events]
                .iter()
                .any(|e| !matches!(e, Event::Midi { .. } | Event::SysEx { .. })))
    {
        return Err(invalid());
    }
    // One bounded frame/write, including the worst-case automation/MIDI burst.
    let mut bytes = [0u8; 40
        + TRANSPORT_BYTES
        + MAX_BUSES * 4096 * 8
        + MAX_EVENTS * 24
        + MAX_MIDI_PAYLOAD_BYTES];
    let header = &mut bytes[..40];
    header[..4].copy_from_slice(b"OXVB");
    header[4..8].copy_from_slice(&STREAM_VERSION.to_le_bytes());
    header[8..16].copy_from_slice(&block.sequence.to_le_bytes());
    header[16..20].copy_from_slice(&(block.frames as u32).to_le_bytes());
    header[20..24].copy_from_slice(&(events as u32).to_le_bytes());
    let flags = (if response { 2u32 } else { 1u32 })
        | (u32::from(block.reset) << 2)
        | (u32::from(block.restart_required) << 3);
    header[24..28].copy_from_slice(&flags.to_le_bytes());
    if response {
        header[28..32].copy_from_slice(&block.processing_micros.to_le_bytes());
    }
    header[32..36].copy_from_slice(&(block.bus_count as u32).to_le_bytes());
    header[36..40].copy_from_slice(&(block.payload.len() as u32).to_le_bytes());
    transport_codec::encode(block.transport, &mut bytes[40..40 + TRANSPORT_BYTES])?;
    let audio_start = 40 + TRANSPORT_BYTES;
    let audio_end = audio_start + block.bus_count * block.frames * 8;
    for (sample, target) in block.audio[..2 * block.bus_count * block.frames]
        .iter()
        .zip(bytes[audio_start..audio_end].chunks_exact_mut(4))
    {
        target.copy_from_slice(&sample.to_le_bytes());
    }
    for (event, bytes) in block.events[..events]
        .iter()
        .zip(bytes[audio_end..].chunks_exact_mut(24))
    {
        let (kind, id, midi, value) = match *event {
            Event::SysEx { data, .. } => (5, data.offset, data.length, 0.),
            Event::Midi {
                message: [status, a, b],
                ..
            } => (
                4,
                0,
                u32::from(status) | (u32::from(a) << 8) | (u32::from(b) << 16),
                0.,
            ),
            Event::Parameter {
                parameter_id,
                value,
                ..
            } => (1u32, parameter_id, 0u32, value),
            Event::NoteOn {
                channel,
                pitch,
                velocity,
                ..
            } => (2, 0, channel as u32 | ((pitch as u32) << 8), velocity),
            Event::NoteOff {
                channel,
                pitch,
                velocity,
                ..
            } => (3, 0, channel as u32 | ((pitch as u32) << 8), velocity),
        };
        bytes[..4].copy_from_slice(&kind.to_le_bytes());
        bytes[4..8].copy_from_slice(&(event.frame() as u32).to_le_bytes());
        bytes[8..12].copy_from_slice(&id.to_le_bytes());
        bytes[12..16].copy_from_slice(&midi.to_le_bytes());
        bytes[16..24].copy_from_slice(&value.to_le_bytes());
    }
    let payload_start = audio_end + events * 24;
    bytes[payload_start..payload_start + block.payload.len()].copy_from_slice(&block.payload);
    writer.write_all(&bytes[..payload_start + block.payload.len()])
}
pub(crate) fn read_block(
    reader: &mut impl Read,
    block: &mut Block,
    response: bool,
) -> io::Result<()> {
    let mut header = [0u8; 40];
    reader.read_exact(&mut header)?;
    if response
        && &header[..4] == b"OXVF"
        && header[4..8] == STREAM_VERSION.to_le_bytes()
        && header[8..].iter().all(|v| *v == 0)
    {
        return Err(io::Error::other("VST3 processor fault"));
    }
    let u32_at = |n| u32::from_le_bytes(header[n..n + 4].try_into().unwrap());
    let frames = u32_at(16) as usize;
    let events = u32_at(20) as usize;
    let buses = u32_at(32) as usize;
    let payload_bytes = u32_at(36) as usize;
    if &header[..4] != b"OXVB"
        || u32_at(4) != STREAM_VERSION
        || (!response && u32_at(28) != 0)
        || u32_at(24) & !(if response { 12 } else { 4 }) != if response { 2 } else { 1 }
        || frames == 0
        || frames > 4096
        || frames > block.max_frames
        || buses > MAX_BUSES
        || (!response && buses == 0)
        || payload_bytes > MAX_MIDI_PAYLOAD_BYTES
        || frames * buses * 2 > block.audio.len()
        || events > MAX_EVENTS
        || (u32_at(24) & 8 != 0 && (events != 0 || payload_bytes != 0))
    {
        return Err(invalid());
    }
    block.frames = frames;
    block.bus_count = buses;
    block.reset = u32_at(24) & 4 != 0;
    block.restart_required = u32_at(24) & 8 != 0;
    block.sequence = u64::from_le_bytes(header[8..16].try_into().unwrap());
    block.event_count = events;
    block.processing_micros = u32_at(28);
    let mut bytes =
        [0u8; TRANSPORT_BYTES + MAX_BUSES * 4096 * 8 + MAX_EVENTS * 24 + MAX_MIDI_PAYLOAD_BYTES];
    reader.read_exact(
        &mut bytes[..TRANSPORT_BYTES + buses * frames * 8 + events * 24 + payload_bytes],
    )?;
    block.transport = transport_codec::decode(&bytes[..TRANSPORT_BYTES])?;
    let bytes = &bytes[TRANSPORT_BYTES..];
    for (target, bytes) in block.audio[..buses * frames * 2]
        .iter_mut()
        .zip(bytes[..buses * frames * 8].chunks_exact(4))
    {
        *target = f32::from_le_bytes(bytes.try_into().unwrap());
        if !target.is_finite() || (block.restart_required && *target != 0.) {
            return Err(invalid());
        }
    }
    for (target, bytes) in block.events[..events]
        .iter_mut()
        .zip(bytes[buses * frames * 8..].chunks_exact(24))
    {
        let word = |n| u32::from_le_bytes(bytes[n..n + 4].try_into().unwrap());
        let frame = word(4) as u64;
        let value = f64::from_le_bytes(bytes[16..24].try_into().unwrap());
        if frame >= frames as u64 || !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(invalid());
        }
        *target = match word(0) {
            5 if value.to_bits() == 0 => Event::SysEx {
                frame,
                data: MidiBytes {
                    offset: word(8),
                    length: word(12),
                },
            },
            1 if !response && word(12) == 0 => Event::Parameter {
                frame,
                parameter_id: word(8),
                value,
            },
            kind @ (2 | 3) if !response && word(8) == 0 && word(12) & !0x7f0f == 0 => {
                let channel = word(12) as u8;
                let pitch = (word(12) >> 8) as u8;
                if kind == 2 {
                    Event::NoteOn {
                        frame,
                        channel,
                        pitch,
                        velocity: value,
                    }
                } else {
                    Event::NoteOff {
                        frame,
                        channel,
                        pitch,
                        velocity: value,
                    }
                }
            }
            4 if word(8) == 0 && word(12) >> 24 == 0 && value.to_bits() == 0 => {
                let message = [
                    word(12) as u8,
                    (word(12) >> 8) as u8,
                    (word(12) >> 16) as u8,
                ];
                if !crate::wire::valid_midi(message) {
                    return Err(invalid());
                }
                Event::Midi { frame, message }
            }
            _ => return Err(invalid()),
        };
    }
    let payload_start = buses * frames * 8 + events * 24;
    block.payload.clear();
    block
        .payload
        .extend_from_slice(&bytes[payload_start..payload_start + payload_bytes]);
    if !crate::event_wire::valid_payloads(&block.events[..events], &block.payload) {
        return Err(invalid());
    }
    Ok(())
}
