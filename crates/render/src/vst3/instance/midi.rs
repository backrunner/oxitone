//! Preallocated MIDI input/output storage, owned by one isolated graph instance.
use oxitone_core::midi_bytes::{append_sysex, MAX_MIDI_PAYLOAD_BYTES};
use oxitone_core::OxitoneError;
use oxitone_graph::midi::{MidiEvent, MidiMessage, MAX_MIDI_EVENTS};
use oxitone_vst3_host::wire::Event;

pub(super) struct MidiState {
    pub input: Vec<MidiEvent>,
    pub output: Vec<MidiEvent>,
    pub input_payload: Vec<u8>,
    pub output_payload: Vec<u8>,
    // IMidiMapping is processor-owned. Once MIDI can change parameters, authored values must
    // reach the processor even when equal to the host's last requested value.
    pub controls_parameters: bool,
}
impl Default for MidiState {
    fn default() -> Self {
        Self {
            input: Vec::with_capacity(MAX_MIDI_EVENTS),
            output: Vec::with_capacity(MAX_MIDI_EVENTS),
            input_payload: Vec::with_capacity(MAX_MIDI_PAYLOAD_BYTES),
            output_payload: Vec::with_capacity(MAX_MIDI_PAYLOAD_BYTES),
            controls_parameters: false,
        }
    }
}
impl MidiState {
    pub fn clear(&mut self) {
        self.input.clear();
        self.output.clear();
        self.input_payload.clear();
        self.output_payload.clear();
    }
    pub fn stage(
        &mut self,
        events: &[MidiEvent],
        payload: &[u8],
        frames: usize,
    ) -> Result<(), OxitoneError> {
        if self.input.len() + events.len() > MAX_MIDI_EVENTS {
            return Err(OxitoneError::new(
                "BudgetExceeded",
                "MIDI route fan-in exceeds 256 events",
            ));
        }
        if payload.len() > MAX_MIDI_PAYLOAD_BYTES
            || events.iter().any(|e| !e.valid(frames, payload))
        {
            return Err(super::invalid("invalid routed MIDI event"));
        }
        let bytes: usize = events
            .iter()
            .map(|event| match event.message {
                MidiMessage::SysEx(data) => data.length as usize,
                _ => 0,
            })
            .sum();
        if self.input_payload.len() + bytes > MAX_MIDI_PAYLOAD_BYTES {
            return Err(OxitoneError::new(
                "BudgetExceeded",
                "MIDI route fan-in exceeds 16 KiB of SysEx",
            ));
        }
        self.controls_parameters |= events.iter().any(|event| match event.message {
            MidiMessage::Channel(message) => matches!(message[0] & 0xf0, 0xb0..=0xe0),
            MidiMessage::SysEx(_) => true,
        });
        for event in events {
            let mut event = *event;
            if let MidiMessage::SysEx(data) = event.message {
                event.message = MidiMessage::SysEx(
                    append_sysex(&mut self.input_payload, data.get(payload).unwrap()).unwrap(),
                );
            }
            self.input.push(event);
        }
        Ok(())
    }
    pub fn receive(&mut self, events: &[Event], payload: &[u8]) -> Result<(), OxitoneError> {
        self.output.clear();
        self.output_payload.clear();
        if events.len() > MAX_MIDI_EVENTS || payload.len() > MAX_MIDI_PAYLOAD_BYTES {
            return Err(super::invalid("unbounded MIDI output"));
        }
        for event in events {
            let (frame, message) = match event {
                Event::Midi { frame, message } => (*frame, MidiMessage::Channel(*message)),
                Event::SysEx { frame, data } if data.get(payload).is_some() => {
                    (*frame, MidiMessage::SysEx(*data))
                }
                _ => return Err(super::invalid("unexpected MIDI output kind")),
            };
            self.output.push(MidiEvent {
                frame_offset: frame as u32,
                message,
            });
        }
        self.output_payload.extend_from_slice(payload);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sysex_fan_in_owns_each_payload_and_rejects_overflow_before_mutation() {
        let mut state = MidiState::default();
        let mut bytes = [0x7d; 4096];
        bytes[0] = 0xf0;
        bytes[4095] = 0xf7;
        let data = oxitone_core::midi_bytes::MidiBytes {
            offset: 0,
            length: 4096,
        };
        let event = MidiEvent {
            frame_offset: 13,
            message: MidiMessage::SysEx(data),
        };
        for _ in 0..4 {
            state.stage(&[event], &bytes, 128).unwrap();
        }
        assert!(state.controls_parameters);
        for (i, event) in state.input.iter().enumerate() {
            let MidiMessage::SysEx(data) = event.message else {
                panic!("lost SysEx")
            };
            assert_eq!(data.offset, i as u32 * 4096);
            assert_eq!(data.get(&state.input_payload).unwrap(), bytes);
        }
        assert_eq!(
            state.stage(&[event], &bytes, 128).unwrap_err().code,
            "BudgetExceeded"
        );
        assert_eq!(state.input.len(), 4);
        assert_eq!(state.input_payload.len(), MAX_MIDI_PAYLOAD_BYTES);
        state
            .receive(&[Event::SysEx { frame: 13, data }], &bytes)
            .unwrap();
        state.clear();
        assert!(state.input_payload.is_empty() && state.output_payload.is_empty());
        bytes[1] = 0xff;
        assert!(state.stage(&[event], &bytes, 128).is_err());
        assert!(state.input.is_empty() && state.input_payload.is_empty());
    }
    #[test]
    fn fan_in_is_atomic_and_reset_removes_stale_output() {
        let mut state = MidiState::default();
        let event = MidiEvent {
            frame_offset: 127,
            message: MidiMessage::Channel([0x80, 60, 0]),
        };
        state.stage(&[event; MAX_MIDI_EVENTS], &[], 128).unwrap();
        assert_eq!(
            state.stage(&[event], &[], 128).unwrap_err().code,
            "BudgetExceeded"
        );
        assert_eq!(state.input.len(), MAX_MIDI_EVENTS);
        assert_eq!(state.input.capacity(), MAX_MIDI_EVENTS);
        state
            .receive(
                &[Event::Midi {
                    frame: 127,
                    message: [0x80, 60, 0],
                }],
                &[],
            )
            .unwrap();
        assert_eq!(state.output, [event]);
        state.clear();
        assert!(state.input.is_empty() && state.output.is_empty());
        assert!(state.stage(&[event], &[], 127).is_err());
        assert!(state.input.is_empty());
    }
}
