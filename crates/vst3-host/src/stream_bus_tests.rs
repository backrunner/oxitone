use super::*;
use crate::{
    bus_wire::{AudioBus, AudioBuses},
    stream_wire::MAX_EVENTS,
};

#[test]
fn midi_only_port_has_no_output_storage_and_never_replays_events_or_allocates() {
    let mut info: Ready =
        serde_json::from_value(serde_json::to_value(&tests::shared().info).unwrap()).unwrap();
    info.audio_buses.inputs.clear();
    info.audio_buses.outputs.clear();
    info.input_channels = 0;
    info.output_channels = 0;
    info.note_output = true;
    let shared = Shared::new(info, 128, 2, true);
    let event = Event::Midi {
        frame: 16,
        message: [0x80, 60, 0],
    };
    let mut port = RealtimePort {
        shared: shared.clone(),
        next_sequence: 0,
        output_events: [event; MAX_EVENTS],
        output_event_count: 0,
        output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
    };
    let silent = [0.; 17];
    let (mut left, mut right) = ([1.; 128], [1.; 128]);
    let counts = tests::allocations::count(|| {
        assert_eq!(port.submit(&silent, &silent, &[]), Ok(0));
        let mut block = shared.pending.pop().unwrap();
        block.bus_count = 0;
        block.events[0] = event;
        block.event_count = 1;
        shared.recycle(block, &shared.completed);
        assert_eq!(
            port.receive(&mut left, &mut right),
            Err(PortError::InvalidBlock)
        );
        assert_eq!(port.receive_buses(&mut []).unwrap().unwrap().frames, 17);
        assert!(matches!(
            port.output_events(),
            [Event::Midi {
                frame: 16,
                message: [0x80, 60, 0]
            }]
        ));
        assert_eq!(port.receive_buses(&mut []), Ok(None));
        assert!(port.output_events().is_empty());
    });
    assert_eq!(counts, (0, 0));
    assert_eq!(left, [0.; 128]);
    assert_eq!(left, right);
}

#[test]
fn multibus_port_preserves_slots_short_blocks_and_allocates_nothing() {
    let mut info: Ready =
        serde_json::from_value(serde_json::to_value(&tests::shared().info).unwrap()).unwrap();
    info.audio_buses = AudioBuses {
        inputs: vec![
            AudioBus {
                channels: 2,
                active: true,
            },
            AudioBus {
                channels: 1,
                active: false,
            },
            AudioBus {
                channels: 2,
                active: true,
            },
        ],
        outputs: vec![
            AudioBus {
                channels: 2,
                active: true
            };
            3
        ],
    };
    let shared = Shared::new(info, 128, 2, false);
    let mut port = RealtimePort {
        shared: shared.clone(),
        next_sequence: 0,
        output_events: [crate::wire::Event::Midi {
            frame: 0,
            message: [0x80, 0, 0],
        }; crate::stream_wire::MAX_EVENTS],
        output_event_count: 0,
        output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
    };
    let zero = [0.; 128];
    let first = [0.125; 128];
    let last = [-0.25; 128];
    let mut out = [[0.; 128]; 6];
    let [a, b, c, d, e, f] = &mut out;
    let mut outputs = [
        [&mut a[..], &mut b[..]],
        [&mut c[..], &mut d[..]],
        [&mut e[..], &mut f[..]],
    ];
    let allocation_count = tests::allocations::count(|| {
        assert_eq!(
            port.submit(&first, &first, &[]),
            Err(PortError::InvalidBlock)
        );
        assert_eq!(
            port.submit_buses(&[[&first[..], &first[..]]; 3], &[], BlockContext::default()),
            Err(PortError::InvalidBlock)
        );
        for (sequence, frames) in [1, 128, 17, 111, 7, 128].into_iter().enumerate() {
            let inputs = [
                [&first[..frames]; 2],
                [&zero[..frames]; 2],
                [&last[..frames]; 2],
            ];
            assert_eq!(
                port.submit_buses(&inputs, &[], BlockContext::default()),
                Ok(sequence as u64)
            );
            let block = shared.pending.pop().unwrap();
            assert_eq!(block.bus_count, 3);
            assert!(block.audio[..2 * frames].iter().all(|v| *v == first[0]));
            shared.recycle(block, &shared.completed);
            // Wrong output count cannot consume the queued completion.
            assert_eq!(
                port.receive_buses(&mut outputs[..1]),
                Err(PortError::InvalidBlock)
            );
            let result = port.receive_buses(&mut outputs).unwrap().unwrap();
            assert_eq!(result.frames, frames);
            for (bus, expected) in outputs.iter().zip([0.125, 0., -0.25]) {
                for channel in bus {
                    assert!(channel[..frames].iter().all(|v| *v == expected));
                    assert!(channel[frames..].iter().all(|v| *v == 0.));
                }
            }
        }
        let events = [Event::Parameter {
            frame: 0,
            parameter_id: 9,
            value: 0.5,
        }; MAX_EVENTS + 1];
        assert_eq!(
            port.submit_buses(&[[&zero[..]; 2]; 3], &events, BlockContext::default()),
            Err(PortError::InvalidBlock)
        );
        shared.stop(Status::PluginFault);
        assert_eq!(port.receive_buses(&mut outputs), Err(PortError::Closed));
        assert!(outputs
            .iter()
            .flatten()
            .flat_map(|c| c.iter())
            .all(|v| *v == 0.));
    });
    assert_eq!(allocation_count, (0, 0));
}
