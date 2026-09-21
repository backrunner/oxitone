use super::{
    tests::{echo, prepare, process},
    *,
};
use crate::stream::tests::{allocations, shared};

#[test]
fn pcm_only_scheduler_rejects_enabled_midi_capture() {
    let mut shared = shared();
    std::sync::Arc::get_mut(&mut shared).unwrap().midi_output = true;
    let port = RealtimePort {
        shared,
        next_sequence: 0,
        output_event_count: 0,
        output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
        output_events: [Event::Midi {
            frame: 0,
            message: [0x80, 0, 0],
        }; crate::stream_wire::MAX_EVENTS],
    };
    let result = ScheduledPort::prepare(
        port,
        Schedule {
            schedule_version: 1,
            epoch: 0,
            latency_blocks: 2,
        },
    );
    assert_eq!(result.err().unwrap().code, "PluginCapabilityUnsupported");
}

#[test]
fn missed_deadline_latches_silence_even_when_old_results_arrive_later() {
    let shared = shared();
    let mut port = prepare(&shared, 1);
    let measured = allocations::count(|| {
        assert_eq!(process(&mut port, 128).unwrap(), [0.; 128]);
        assert_eq!(process(&mut port, 128).unwrap(), [0.; 128]);
        assert_eq!(process(&mut port, 128), Err(Fault::DeadlineMissed));
        echo(&shared);
        let mut left = [9.; 128];
        let mut right = [9.; 128];
        assert_eq!(
            port.process(
                Input {
                    payload: &[],
                    epoch: 1,
                    frame: 256,
                    left: &[0.; 128],
                    right: &[0.; 128],
                    events: &[]
                },
                &mut left,
                &mut right
            ),
            Err(Fault::DeadlineMissed)
        );
        assert_eq!(left, [0.; 128]);
        assert_eq!(right, [0.; 128]);
        port.invalidate();
    });
    assert_eq!(measured, (0, 0));
    assert_eq!(port.fault(), Some(Fault::DeadlineMissed));
    assert_eq!(shared.status(), Status::DeadlineMissed);
}

#[test]
fn seek_epoch_and_frame_discontinuities_discard_partial_input_and_buffered_audio() {
    for (epoch, frame) in [(2, 145), (1, 128), (1, 146)] {
        let old = shared();
        let mut port = prepare(&old, 1);
        process(&mut port, 128).unwrap();
        echo(&old);
        process(&mut port, 17).unwrap();
        let mut left = [1.; 128];
        let mut right = [1.; 128];
        assert_eq!(
            port.process(
                Input {
                    payload: &[],
                    epoch,
                    frame,
                    left: &[0.; 128],
                    right: &[0.; 128],
                    events: &[]
                },
                &mut left,
                &mut right
            ),
            Err(Fault::Discontinuity)
        );
        assert_eq!(left, [0.; 128]);
        assert_eq!(old.status(), Status::Invalidated);
        let fresh = shared();
        let mut replacement = prepare(&fresh, 2);
        assert_eq!(process(&mut replacement, 128).unwrap(), [0.; 128]);
        assert_eq!(replacement.frame(), 128);
        assert_eq!(replacement.fault(), None);
    }
}

#[test]
fn failure_in_second_segment_silences_the_entire_call_and_latches_first_fault() {
    let shared = shared();
    let mut port = prepare(&shared, 0);
    process(&mut port, 128).unwrap();
    echo(&shared); // Only block zero will ever arrive.
    process(&mut port, 128).unwrap();
    assert_eq!(process(&mut port, 111).unwrap()[..111], [0.25; 111]);
    let mut left = [1.; 128];
    let mut right = [1.; 128];
    assert_eq!(
        port.process(
            Input {
                payload: &[],
                epoch: 0,
                frame: 367,
                left: &[0.; 128],
                right: &[0.; 128],
                events: &[]
            },
            &mut left,
            &mut right
        ),
        Err(Fault::DeadlineMissed)
    );
    assert_eq!(left, [0.; 128]);
    shared.stop(Status::Crashed);
    assert_eq!(process(&mut port, 1), Err(Fault::DeadlineMissed));
}

#[test]
fn accumulated_events_overflow_without_submitting_a_partial_block() {
    let shared = shared();
    let mut port = prepare(&shared, 0);
    let events = [Event::Parameter {
        frame: 0,
        parameter_id: 9,
        value: 0.5,
    }; 256];
    let mut left = [0.; 17];
    let mut right = [0.; 17];
    port.process(
        Input {
            payload: &[],
            epoch: 0,
            frame: 0,
            left: &[0.; 17],
            right: &[0.; 17],
            events: &events,
        },
        &mut left,
        &mut right,
    )
    .unwrap();
    let measured = allocations::count(|| {
        assert_eq!(
            port.process(
                Input {
                    payload: &[],
                    epoch: 0,
                    frame: 17,
                    left: &[0.; 17],
                    right: &[0.; 17],
                    events: &events[..1]
                },
                &mut left,
                &mut right
            ),
            Err(Fault::EventOverflow)
        );
    });
    assert_eq!(measured, (0, 0));
    assert_eq!(port.frame(), 17);
    assert!(shared.pending.is_empty());
}

#[test]
fn invalid_buffers_events_samples_and_frame_overflow_fail_closed() {
    for fault_kind in 0..6 {
        let shared = shared();
        let mut port = prepare(&shared, 0);
        let mut input = [0.; 128];
        if fault_kind == 0 {
            input[127] = f32::NAN;
        }
        if fault_kind == 4 {
            port.frame = u64::MAX - 127;
        }
        let events = [Event::Parameter {
            frame: 128,
            parameter_id: 9,
            value: 0.5,
        }];
        let frames = if fault_kind == 1 { 0 } else { 128 };
        let mut left = [1.; 129];
        let mut right = [1.; 128];
        let out_frames = if fault_kind == 2 { 129 } else { 128 };
        if fault_kind == 5 {
            shared.stop(Status::PluginFault);
        }
        let expected = match fault_kind {
            4 => Fault::FrameOverflow,
            5 => Fault::Stream(Status::PluginFault),
            _ => Fault::InvalidBlock,
        };
        let measured = allocations::count(|| {
            assert_eq!(
                port.process(
                    Input {
                        payload: &[],
                        epoch: 0,
                        frame: port.frame(),
                        left: &input[..frames],
                        right: &input[..frames],
                        events: if fault_kind == 3 { &events } else { &[] }
                    },
                    &mut left[..out_frames],
                    &mut right
                ),
                Err(expected)
            );
        });
        assert_eq!(measured, (0, 0));
        assert_eq!(left[..128], [0.; 128]);
        assert_eq!(left[128], 1.);
        assert_eq!(right, [0.; 128]);
    }
}

#[test]
fn corrupt_completion_identity_or_length_never_reaches_audio() {
    for wrong_sequence in [false, true] {
        let shared = shared();
        let mut port = prepare(&shared, 0);
        process(&mut port, 128).unwrap();
        let mut block = shared.pending.pop().unwrap();
        if wrong_sequence {
            block.sequence = 2;
        } else {
            block.frames = 17;
        }
        shared.recycle(block, &shared.completed);
        assert_eq!(process(&mut port, 128), Err(Fault::InvalidResponse));
    }
}
