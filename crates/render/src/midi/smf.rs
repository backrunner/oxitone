//! SMF Type 1 byte writer: header chunk, track chunks, delta-time varlen
//! encoding, and same-tick ordering (note-off before note-on; a note's own
//! off always follows its on). No running status — every event writes its
//! full status byte so the output is trivially deterministic.

use super::expand::ExpandedNote;

/// One encodable track event. `data` is the complete event bytes including
/// the status/meta byte. Sort key is `(tick, rank, pitch, sequence)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackEvent {
    pub tick: u64,
    pub rank: u8,
    pub pitch: u8,
    pub sequence: u64,
    pub data: Vec<u8>,
}

pub(crate) const RANK_NOTE_OFF: u8 = 10;
pub(crate) const RANK_NOTE_ON: u8 = 11;
/// Rank for a note-off whose tick equals its own note-on (zero-length note
/// after tick rounding): it must follow the note-on.
const RANK_NOTE_OFF_SAME_TICK: u8 = 12;

/// Expand one note into its on/off event pair on `channel` (1..=16).
pub(crate) fn note_events(note: &ExpandedNote, channel: u8) -> [TrackEvent; 2] {
    let ch = channel - 1;
    let same_tick = note.tick_off == note.tick_on;
    [
        TrackEvent {
            tick: note.tick_on,
            rank: RANK_NOTE_ON,
            pitch: note.pitch,
            sequence: note.sequence,
            data: vec![0x90 | ch, note.pitch, note.on_velocity],
        },
        TrackEvent {
            tick: note.tick_off,
            rank: if same_tick {
                RANK_NOTE_OFF_SAME_TICK
            } else {
                RANK_NOTE_OFF
            },
            pitch: note.pitch,
            sequence: note.sequence,
            data: vec![0x80 | ch, note.pitch, note.off_velocity],
        },
    ]
}

/// MIDI variable-length quantity.
pub(crate) fn write_varlen(mut value: u64, out: &mut Vec<u8>) {
    let mut bytes = [0u8; 10];
    let mut n = 0;
    loop {
        bytes[n] = (value & 0x7F) as u8;
        value >>= 7;
        n += 1;
        if value == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        out.push(bytes[i] | if i == 0 { 0 } else { 0x80 });
    }
}

fn write_track(name: &str, events: &[TrackEvent], out: &mut Vec<u8>) {
    let mut body = Vec::new();
    // Track name meta event at tick 0, always first.
    body.extend_from_slice(&[0x00, 0xFF, 0x03]);
    write_varlen(name.len() as u64, &mut body);
    body.extend_from_slice(name.as_bytes());

    let mut sorted: Vec<&TrackEvent> = events.iter().collect();
    sorted.sort_by_key(|e| (e.tick, e.rank, e.pitch, e.sequence));
    let mut last_tick = 0_u64;
    for event in sorted {
        write_varlen(event.tick - last_tick, &mut body);
        body.extend_from_slice(&event.data);
        last_tick = event.tick;
    }
    body.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);

    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
}

/// Serialize a complete SMF Type 1 file. `tracks` is `(name, events)` per
/// track; the first track is the conductor track.
pub(crate) fn write_smf(ppq: u16, tracks: &[(String, Vec<TrackEvent>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6_u32.to_be_bytes());
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&(tracks.len() as u16).to_be_bytes());
    out.extend_from_slice(&ppq.to_be_bytes());
    for (name, events) in tracks {
        write_track(name, events, &mut out);
    }
    out
}
