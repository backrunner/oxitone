use super::*;
use crate::{
    stream_codec,
    stream_wire::{Parameter, STREAM_VERSION},
};
#[path = "../../render/tests/common/allocations.rs"]
pub(super) mod allocations;

pub(super) fn shared() -> Arc<Shared> {
    shared_with_depth(2)
}
pub(super) fn shared_with_depth(depth: usize) -> Arc<Shared> {
    Shared::new(
        Ready {
            stream_protocol_version: STREAM_VERSION,
            sample_rate: 48000,
            block_size: 128,
            class_id: "1".repeat(32),
            sha256: "a".repeat(64),
            input_channels: 2,
            output_channels: 2,
            audio_buses: crate::bus_wire::AudioBuses::stereo(2, 2),
            category: "Fx".into(),
            note_input: true,
            note_output: false,
            latency_frames: 0,
            tail_frames: 0,
            helper_time_constraint: false,
            parameters: vec![Parameter {
                id: 9,
                writable: true,
                automatable: true,
            }],
        },
        128,
        depth,
        false,
    )
}
#[test]
fn queue_bounds_invalid_inputs_and_no_stale_output() {
    let shared = shared();
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
    let input = [0.25; 128];
    assert_eq!(port.submit(&[], &[], &[]), Err(PortError::InvalidBlock));
    assert_eq!(
        port.submit(
            &input,
            &input,
            &[Event::Parameter {
                frame: 128,
                parameter_id: 9,
                value: 0.2
            }]
        ),
        Err(PortError::InvalidBlock)
    );
    assert_eq!(
        port.submit(
            &input,
            &input,
            &[Event::Parameter {
                frame: 0,
                parameter_id: 99,
                value: 0.2
            }]
        ),
        Err(PortError::InvalidBlock)
    );
    assert_eq!(port.submit(&input, &input, &[]), Ok(0));
    assert_eq!(port.submit(&input, &input, &[]), Ok(1));
    assert_eq!(port.submit(&input, &input, &[]), Err(PortError::Full));
    let mut left = [1.; 128];
    let mut right = [1.; 128];
    assert_eq!(port.receive(&mut left, &mut right), Ok(None));
    assert_eq!(left, [0.; 128]);
    let block = shared.pending.pop().unwrap();
    shared.recycle(block, &shared.completed);
    assert_eq!(
        port.receive(&mut left, &mut right)
            .unwrap()
            .unwrap()
            .sequence,
        0
    );
    assert_eq!(left, input);
    shared.stop(Status::TimedOut);
    assert_eq!(port.receive(&mut left, &mut right), Err(PortError::Closed));
    assert_eq!(left, [0.; 128]);
    assert_eq!(port.status(), Status::TimedOut);
}
#[test]
fn binary_round_trip_preserves_events_and_partial_blocks_and_rejects_bad_headers() {
    let mut block = Block::new(128);
    block.frames = 17;
    block.sequence = 7;
    block.audio[..34].fill(0.25);
    block.events[0] = Event::NoteOn {
        frame: 16,
        channel: 15,
        pitch: 127,
        velocity: 0.7,
    };
    block.event_count = 1;
    let mut bytes = Vec::new();
    stream_codec::write_block(&mut bytes, &block, false).unwrap();
    let mut decoded = Block::new(128);
    stream_codec::read_block(&mut &bytes[..], &mut decoded, false).unwrap();
    assert_eq!(decoded.frames, 17);
    assert_eq!(decoded.sequence, 7);
    assert_eq!(decoded.audio[..34], block.audio[..34]);
    assert!(
        matches!(decoded.events[0], Event::NoteOn { frame: 16, channel: 15, pitch: 127, velocity } if velocity == 0.7)
    );
    for (offset, value) in [(4, 1), (16, 0), (20, 255), (24, 2), (28, 1)] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = value;
        assert!(stream_codec::read_block(&mut &corrupt[..], &mut decoded, false).is_err());
    }
    for size in [0, 31, 40, bytes.len() - 1] {
        assert!(stream_codec::read_block(&mut &bytes[..size], &mut decoded, false).is_err());
    }
}

#[test]
fn realtime_port_success_saturation_and_fault_paths_allocate_and_free_nothing() {
    let shared = shared();
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
    let input = [0.25; 128];
    let mut left = [0.; 128];
    let mut right = [0.; 128];
    let events = [Event::Parameter {
        frame: 127,
        parameter_id: 9,
        value: 0.5,
    }];
    let measured = allocations::count(|| {
        for sequence in 0..10_000 {
            let result = if sequence % 2 == 0 {
                port.submit_reset_at(&input, &input, &events, crate::transport_tests::position())
            } else {
                port.submit_at(&input, &input, &events, crate::transport_tests::position())
            };
            assert_eq!(result, Ok(sequence));
            let mut block = shared.pending.pop().unwrap();
            assert_eq!(block.reset, sequence % 2 == 0);
            assert_eq!(block.transport, Some(crate::transport_tests::position()));
            assert!(!block.restart_required);
            block.restart_required = sequence % 3 == 0;
            block.event_count = if block.restart_required { 0 } else { 1 };
            block.events[0] = Event::Midi {
                frame: 127,
                message: [0x8f, 60, 0],
            };
            if block.restart_required {
                block.audio.fill(0.);
            }
            shared.recycle(block, &shared.completed);
            let received = port.receive(&mut left, &mut right).unwrap().unwrap();
            assert_eq!(received.sequence, sequence);
            assert_eq!(received.restart_required, sequence % 3 == 0);
            assert!(!port.restart_required());
            assert_eq!(
                port.output_events().len(),
                if received.restart_required { 0 } else { 1 }
            );
            if !received.restart_required {
                assert!(matches!(
                    port.output_events()[0],
                    Event::Midi {
                        frame: 127,
                        message: [0x8f, 60, 0]
                    }
                ));
            }
            assert_eq!(port.receive(&mut left, &mut right), Ok(None));
            assert!(port.output_events().is_empty());
        }
        port.submit(&input, &input, &events).unwrap();
        port.submit(&input, &input, &events).unwrap();
        assert_eq!(port.submit(&input, &input, &events), Err(PortError::Full));
        shared.stop(Status::Crashed);
        assert_eq!(port.receive(&mut left, &mut right), Err(PortError::Closed));
        assert_eq!(port.submit(&input, &input, &events), Err(PortError::Closed));
    });
    assert_eq!(measured, (0, 0));
    assert_eq!(left, [0.; 128]);
}
