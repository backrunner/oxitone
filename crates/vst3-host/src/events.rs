use crate::{native, wire::Event, Result};
use vst3_host::{MidiChannel, MidiEvent, Plugin};

pub fn apply(
    plugin: &mut Plugin,
    events: &[Event],
    payload: &[u8],
    start: u64,
    frames: usize,
) -> Result<()> {
    let end = start.saturating_add(frames as u64);
    // Sorted once before rendering; no full-timeline scan/allocation for each block.
    let first = events.partition_point(|event| event.frame() < start);
    let last = events.partition_point(|event| event.frame() < end);
    if last - first > crate::stream_wire::MAX_EVENTS
        || !crate::event_wire::valid_payloads(&events[first..last], payload)
    {
        return Err(crate::Error::new(
            "BudgetExceeded",
            "VST3 block event or SysEx budget exceeded",
        ));
    }
    for event in &events[first..last] {
        let offset = (event.frame() - start) as i32;
        match event {
            Event::SysEx { data, .. } => plugin
                .send_sysex_at(
                    data.get(payload)
                        .ok_or_else(|| crate::invalid("invalid SysEx range"))?
                        .to_vec(),
                    offset,
                )
                .map_err(native)?,
            Event::Midi { message, .. } => plugin
                .send_midi_event_at(decode(*message)?, offset)
                .map_err(native)?,
            Event::Parameter {
                parameter_id,
                value,
                ..
            } => plugin
                .set_parameter_at(*parameter_id, *value, offset)
                .map_err(native)?,
            Event::NoteOn {
                channel,
                pitch,
                velocity,
                ..
            } => plugin
                .send_midi_event_at(
                    MidiEvent::NoteOn {
                        channel: MidiChannel::from_index(*channel)
                            .ok_or_else(|| crate::invalid("invalid MIDI channel"))?,
                        note: *pitch,
                        velocity: (*velocity * 127.0).round() as u8,
                    },
                    offset,
                )
                .map_err(native)?,
            Event::NoteOff {
                channel,
                pitch,
                velocity,
                ..
            } => plugin
                .send_midi_event_at(
                    MidiEvent::NoteOff {
                        channel: MidiChannel::from_index(*channel)
                            .ok_or_else(|| crate::invalid("invalid MIDI channel"))?,
                        note: *pitch,
                        velocity: (*velocity * 127.0).round() as u8,
                    },
                    offset,
                )
                .map_err(native)?,
        }
    }
    Ok(())
}

fn decode(message: [u8; 3]) -> Result<MidiEvent> {
    if !crate::wire::valid_midi(message) {
        return Err(crate::invalid("invalid MIDI message"));
    }
    let [status, a, b] = message;
    let channel = MidiChannel::from_index(status & 15).unwrap();
    Ok(match status & 0xf0 {
        0x80 => MidiEvent::NoteOff {
            channel,
            note: a,
            velocity: b,
        },
        0x90 => MidiEvent::NoteOn {
            channel,
            note: a,
            velocity: b,
        },
        0xa0 => MidiEvent::PolyAftertouch {
            channel,
            note: a,
            pressure: b,
        },
        0xb0 => MidiEvent::ControlChange {
            channel,
            controller: a,
            value: b,
        },
        0xc0 => MidiEvent::ProgramChange {
            channel,
            program: a,
        },
        0xd0 => MidiEvent::ChannelAftertouch {
            channel,
            pressure: a,
        },
        0xe0 => MidiEvent::PitchBend {
            channel,
            value: u16::from(a) | u16::from(b) << 7,
        },
        _ => unreachable!("validated channel message"),
    })
}
