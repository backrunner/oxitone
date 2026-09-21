//! Graph events retain segment-relative sample offsets. State retains the latest control values.
use oxitone_core::OxitoneError;
use oxitone_graph::{NoteEventKind, ProcessContext};
use oxitone_vst3_host::{stream_wire::MAX_EVENTS, wire::Event};
use std::collections::BTreeMap;
#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;

pub(super) fn translate(
    ctx: &ProcessContext<'_>,
    midi: &[oxitone_graph::midi::MidiEvent],
    payload: &[u8],
    controls_parameters: bool,
    parameters: &mut BTreeMap<String, f64>,
    events: &mut Vec<Event>,
) -> Result<(), OxitoneError> {
    events.clear();
    if ctx.note_events.len() + midi.len() > MAX_EVENTS {
        return Err(OxitoneError::new(
            "BudgetExceeded",
            "VST3 graph segment exceeds 256 events",
        ));
    }
    for event in midi {
        if !event.valid(ctx.frames, payload) {
            return Err(super::invalid("MIDI event outside graph segment"));
        }
        let frame = u64::from(event.frame_offset);
        events.push(match event.message {
            oxitone_graph::midi::MidiMessage::Channel(message) => Event::Midi { frame, message },
            oxitone_graph::midi::MidiMessage::SysEx(data) => Event::SysEx { frame, data },
        });
    }
    for parameter in ctx.parameter_events {
        let parameter_id = parameter
            .parameter_id
            .parse::<u32>()
            .map_err(|_| super::invalid("invalid VST3 ParamID"))?;
        // Compare with the last change in THIS segment before consulting the prior block.
        // A -> B -> A must send both changes even when the cached starting value was A.
        let latest = events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Parameter {
                    parameter_id: id,
                    value,
                    ..
                } if *id == parameter_id => Some(value),
                _ => None,
            })
            .or_else(|| parameters.get(parameter.parameter_id));
        if !controls_parameters && latest == Some(&parameter.value) {
            continue;
        }
        if events.len() + ctx.note_events.len() == MAX_EVENTS {
            return Err(OxitoneError::new(
                "BudgetExceeded",
                "VST3 graph segment exceeds 256 events",
            ));
        }
        events.push(Event::Parameter {
            frame: u64::from(parameter.frame_offset),
            parameter_id,
            value: parameter.value,
        });
    }
    for note in ctx.note_events {
        events.push(match note.kind {
            NoteEventKind::NoteOn => Event::NoteOn {
                frame: u64::from(note.frame_offset),
                channel: 0,
                pitch: note.pitch,
                velocity: f64::from(note.velocity),
            },
            NoteEventKind::NoteOff => Event::NoteOff {
                frame: u64::from(note.frame_offset),
                channel: 0,
                pitch: note.pitch,
                velocity: f64::from(note.velocity),
            },
        });
    }
    for parameter in ctx.parameter_events {
        if let Some(value) = parameters.get_mut(parameter.parameter_id) {
            *value = parameter.value;
        } else {
            parameters.insert(parameter.parameter_id.to_string(), parameter.value);
        }
    }
    Ok(())
}
