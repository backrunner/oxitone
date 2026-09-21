//! The same event vocabulary with owned authoring data or ranges into a prepared stream arena.
use oxitone_core::midi_bytes::{MidiBytes, MAX_MIDI_PAYLOAD_BYTES};
use serde::{Deserialize, Serialize};

pub type Event = EventData<MidiBytes>;
pub type OwnedEvent = EventData<Vec<u8>>;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum EventData<P> {
    Midi {
        frame: u64,
        message: [u8; 3],
    },
    SysEx {
        frame: u64,
        data: P,
    },
    Parameter {
        frame: u64,
        #[serde(rename = "parameterId")]
        parameter_id: u32,
        value: f64,
    },
    NoteOn {
        frame: u64,
        channel: u8,
        pitch: u8,
        velocity: f64,
    },
    NoteOff {
        frame: u64,
        channel: u8,
        pitch: u8,
        velocity: f64,
    },
}
impl<P> EventData<P> {
    pub fn frame(&self) -> u64 {
        match self {
            Self::Midi { frame, .. }
            | Self::SysEx { frame, .. }
            | Self::Parameter { frame, .. }
            | Self::NoteOn { frame, .. }
            | Self::NoteOff { frame, .. } => *frame,
        }
    }
    pub fn priority(&self) -> u8 {
        match self {
            Self::Midi { message, .. } => match message[0] & 0xf0 {
                0x80 => 0,
                0x90 if message[2] == 0 => 0,
                0x90 => 2,
                _ => 1,
            },
            Self::NoteOff { .. } => 0,
            Self::Parameter { .. } | Self::SysEx { .. } => 1,
            Self::NoteOn { .. } => 2,
        }
    }
    #[cfg(feature = "host")]
    fn map_payload<Q>(self, map: impl FnOnce(P) -> Q) -> EventData<Q> {
        match self {
            Self::SysEx { frame, data } => EventData::SysEx {
                frame,
                data: map(data),
            },
            Self::Midi { frame, message } => EventData::Midi { frame, message },
            Self::Parameter {
                frame,
                parameter_id,
                value,
            } => EventData::Parameter {
                frame,
                parameter_id,
                value,
            },
            Self::NoteOn {
                frame,
                channel,
                pitch,
                velocity,
            } => EventData::NoteOn {
                frame,
                channel,
                pitch,
                velocity,
            },
            Self::NoteOff {
                frame,
                channel,
                pitch,
                velocity,
            } => EventData::NoteOff {
                frame,
                channel,
                pitch,
                velocity,
            },
        }
    }
}

/// Flatten a validated offline request once, on the helper control thread.
#[cfg(feature = "host")]
pub(crate) fn flatten(events: Vec<OwnedEvent>) -> (Vec<Event>, Vec<u8>) {
    let mut payload = Vec::new();
    let events = events
        .into_iter()
        .map(|event| {
            event.map_payload(|bytes| {
                let range = MidiBytes {
                    offset: payload.len() as u32,
                    length: bytes.len() as u32,
                };
                payload.extend_from_slice(&bytes);
                range
            })
        })
        .collect();
    (events, payload)
}

pub(crate) fn valid_payloads(events: &[Event], payload: &[u8]) -> bool {
    let mut total = 0usize;
    for event in events {
        if let Event::SysEx { data, .. } = event {
            if data.get(payload).is_none() {
                return false;
            }
            total += data.length as usize;
            if total > MAX_MIDI_PAYLOAD_BYTES {
                return false;
            }
        }
    }
    true
}
