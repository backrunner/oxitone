use super::*;
use oxitone_graph::ParameterEvent;
fn translate_values(
    parameters: &mut BTreeMap<String, f64>,
    values: &[ParameterEvent<'_>],
) -> Result<Vec<Event>, OxitoneError> {
    let mut events = Vec::with_capacity(MAX_EVENTS);
    let mut outputs: [&mut [f32]; 0] = [];
    let context = ProcessContext {
        frames: 128,
        sample_rate: 48000.,
        inputs: &[],
        outputs: &mut outputs,
        note_events: &[],
        parameter_events: values,
        sidechain: None,
    };
    translate(&context, &[], &[], false, parameters, &mut events)?;
    Ok(events)
}
#[test]
fn same_segment_return_to_cached_value_reaches_the_processor() {
    let mut state = BTreeMap::from([("0".into(), 0.2)]);
    let values = [0.2, 0.75, 0.75, 0.2, 0.2]
        .iter()
        .enumerate()
        .map(|(frame, value)| ParameterEvent {
            parameter_id: "0",
            value: *value,
            frame_offset: frame as u32,
        })
        .collect::<Vec<_>>();
    let events = translate_values(&mut state, &values).unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        events[0],
        Event::Parameter {
            frame: 1,
            value: 0.75,
            ..
        }
    ));
    assert!(matches!(
        events[1],
        Event::Parameter {
            frame: 3,
            value: 0.2,
            ..
        }
    ));
    assert_eq!(state["0"], 0.2);
    assert!(translate_values(&mut state, &values[..1])
        .unwrap()
        .is_empty());
}
#[test]
fn alternating_values_cannot_bypass_budget_and_failure_preserves_cached_state() {
    let mut state = BTreeMap::from([("0".into(), 0.2)]);
    let values = (0..257)
        .map(|frame| ParameterEvent {
            parameter_id: "0",
            value: if frame % 2 == 0 { 0.75 } else { 0.2 },
            frame_offset: 0,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        translate_values(&mut state, &values).unwrap_err().code,
        "BudgetExceeded"
    );
    assert_eq!(state["0"], 0.2);
}

#[test]
fn midi_parameter_mapping_cannot_hide_an_authored_return_to_the_cached_value() {
    let mut state = BTreeMap::from([("0".into(), 0.2)]);
    let mut outputs: [&mut [f32]; 0] = [];
    let parameters = [
        ParameterEvent {
            frame_offset: 0,
            parameter_id: "0",
            value: 0.2,
        },
        ParameterEvent {
            frame_offset: 16,
            parameter_id: "0",
            value: 0.2,
        },
    ];
    let context = ProcessContext {
        frames: 128,
        sample_rate: 48000.,
        inputs: &[],
        outputs: &mut outputs,
        note_events: &[],
        parameter_events: &parameters,
        sidechain: None,
    };
    let midi = [oxitone_graph::midi::MidiEvent {
        frame_offset: 8,
        message: oxitone_graph::midi::MidiMessage::Channel([0xb0, 7, 100]),
    }];
    let mut events = Vec::with_capacity(MAX_EVENTS);
    translate(&context, &midi, &[], true, &mut state, &mut events).unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[2],
        Event::Parameter {
            frame: 16,
            value: 0.2,
            ..
        }
    ));
    translate(&context, &[], &[], true, &mut state, &mut events).unwrap();
    assert_eq!(
        events.len(),
        2,
        "later explicit restores must still be sent after MIDI changed the processor"
    );
}
