use super::*;

fn note() -> PluginEvent {
    PluginEvent {
        bus_index: 0,
        sample_offset: 31,
        ppq_position: 0.,
        flags: 0,
        data: PluginEventData::NoteOn {
            channel: 15,
            pitch: 60,
            tuning: 0.,
            velocity: 1.,
            length: 0,
            note_id: 4,
        },
    }
}
#[test]
fn capture_preserves_channel_and_sample_position_and_rejects_unsupported_output() {
    assert!(matches!(
        convert(&note(), 32).unwrap(),
        Event::Midi {
            frame: 31,
            message: [0x9f, 60, 127]
        }
    ));
    assert!(convert(&note(), 31).is_err());
    let mut event = note();
    event.sample_offset = -1;
    assert!(convert(&event, 32).is_err());
    event = note();
    event.bus_index = 1;
    assert_eq!(
        convert(&event, 32).unwrap_err().code,
        "PluginCapabilityUnsupported"
    );
    event = PluginEvent::sysex(vec![0xf0, 0xf7]);
    assert_eq!(
        convert(&event, 32).unwrap_err().code,
        "PluginCapabilityUnsupported"
    );
    event = note();
    if let PluginEventData::NoteOn { velocity, .. } = &mut event.data {
        *velocity = f32::NAN;
    }
    assert!(convert(&event, 32).is_err());
}
#[test]
fn captures_controller_pressure_program_and_pitch_bend_without_clamping() {
    for (control, a, b, expected) in [
        (7, 100, 0, [0xbf, 7, 100]),
        (128, 91, 0, [0xdf, 91, 0]),
        (129, 5, 64, [0xef, 5, 64]),
        (130, 12, 0, [0xcf, 12, 0]),
    ] {
        let mut event = note();
        event.data = PluginEventData::LegacyMidiCcOut {
            channel: 15,
            control_number: control,
            value: a,
            value2: b,
        };
        assert!(
            matches!(convert(&event,32).unwrap(), Event::Midi { message, .. } if message == expected)
        );
        if let PluginEventData::LegacyMidiCcOut { value, .. } = &mut event.data {
            *value = 128;
        }
        assert!(convert(&event, 32).is_err());
    }
}
