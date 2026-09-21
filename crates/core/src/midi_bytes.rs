//! Owned arenas and checked ranges for bounded MIDI SysEx data.
use serde::{Deserialize, Serialize};

pub const MAX_SYSEX_BYTES: usize = 4096;
pub const MAX_MIDI_PAYLOAD_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiBytes {
    pub offset: u32,
    pub length: u32,
}
impl MidiBytes {
    pub fn get(self, payload: &[u8]) -> Option<&[u8]> {
        let start = self.offset as usize;
        let end = start.checked_add(self.length as usize)?;
        let bytes = payload.get(start..end)?;
        valid_sysex(bytes).then_some(bytes)
    }
}

/// Complete MIDI 1.0 messages, including F0/F7. Embedded status/realtime bytes are rejected.
pub fn valid_sysex(bytes: &[u8]) -> bool {
    (2..=MAX_SYSEX_BYTES).contains(&bytes.len())
        && bytes[0] == 0xf0
        && bytes[bytes.len() - 1] == 0xf7
        && bytes[1..bytes.len() - 1].iter().all(|b| *b < 128)
}

/// Append only to prepared storage; failure neither allocates nor changes the arena.
pub fn append_sysex(payload: &mut Vec<u8>, bytes: &[u8]) -> Option<MidiBytes> {
    if !valid_sysex(bytes)
        || payload.len() + bytes.len() > MAX_MIDI_PAYLOAD_BYTES
        || payload.len() + bytes.len() > payload.capacity()
    {
        return None;
    }
    let range = MidiBytes {
        offset: payload.len() as u32,
        length: bytes.len() as u32,
    };
    payload.extend_from_slice(bytes);
    Some(range)
}
