use super::*;
use crate::stream::{
    tests::{allocations, shared, shared_with_depth},
    Shared,
};
use std::sync::Arc;

pub(super) fn prepare(shared: &Arc<Shared>, epoch: u64) -> ScheduledPort {
    ScheduledPort::prepare(
        RealtimePort {
            shared: shared.clone(),
            next_sequence: 0,
            output_events: [crate::wire::Event::Midi {
                frame: 0,
                message: [0x80, 0, 0],
            }; crate::stream_wire::MAX_EVENTS],
            output_event_count: 0,
            output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
        },
        Schedule {
            schedule_version: 1,
            epoch,
            latency_blocks: 2,
        },
    )
    .unwrap()
}
pub(super) fn echo(shared: &Shared) {
    while let Some(block) = shared.pending.pop() {
        shared.recycle(block, &shared.completed);
    }
}
pub(super) fn process(port: &mut ScheduledPort, frames: usize) -> Result<[f32; 128], Fault> {
    let input = [0.25; 128];
    let mut left = [1.; 128];
    let mut right = [1.; 128];
    port.process(
        Input {
            payload: &[],
            epoch: port.epoch(),
            frame: port.frame(),
            left: &input[..frames],
            right: &input[..frames],
            events: &[],
        },
        &mut left[..frames],
        &mut right[..frames],
    )?;
    assert_eq!(left, right);
    Ok(left)
}

#[test]
fn variable_segments_preserve_pcm_and_intrinsic_latency_at_exact_sample_frames() {
    for depth in [2, 4, 16] {
        let mut shared = shared_with_depth(depth);
        Arc::get_mut(&mut shared).unwrap().info.latency_frames = 7;
        let mut port = ScheduledPort::prepare(
            RealtimePort {
                shared: shared.clone(),
                next_sequence: 0,
                output_events: [crate::wire::Event::Midi {
                    frame: 0,
                    message: [0x80, 0, 0],
                }; crate::stream_wire::MAX_EVENTS],
                output_event_count: 0,
                output_payload: Vec::with_capacity(
                    oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES,
                ),
            },
            Schedule {
                schedule_version: 1,
                epoch: 9,
                latency_blocks: depth,
            },
        )
        .unwrap();
        let delay = (128 * depth + 7) as u64;
        assert_eq!(port.latency_frames(), delay as u32);
        assert_eq!(port.buffering_latency_frames(), (128 * depth) as u32);
        let mut plugin_delay = [[0.; 7]; 2];
        let mut delay_cursor = 0;
        for frames in [1, 17, 111, 128, 3, 95, 127].into_iter().cycle().take(400) {
            let frame = port.frame();
            let l = std::array::from_fn::<_, 128, _>(|i| signal(frame + i as u64));
            let r = l.map(|v| -v);
            let mut out_l = [1.; 128];
            let mut out_r = [1.; 128];
            port.process(
                Input {
                    payload: &[],
                    epoch: 9,
                    frame,
                    left: &l[..frames],
                    right: &r[..frames],
                    events: &[],
                },
                &mut out_l[..frames],
                &mut out_r[..frames],
            )
            .unwrap();
            for i in 0..frames {
                let expected = (frame + i as u64)
                    .checked_sub(delay)
                    .map(signal)
                    .unwrap_or(0.);
                assert_eq!(
                    out_l[i],
                    expected,
                    "depth {depth}, frame {}",
                    frame + i as u64
                );
                assert_eq!(out_r[i], -expected);
            }
            while let Some(mut block) = shared.pending.pop() {
                assert_eq!(block.frames, 128);
                for i in 0..128 {
                    for (channel, samples) in plugin_delay.iter_mut().enumerate() {
                        std::mem::swap(
                            &mut samples[delay_cursor],
                            &mut block.audio[channel * 128 + i],
                        );
                    }
                    delay_cursor = (delay_cursor + 1) % 7;
                }
                shared.recycle(block, &shared.completed);
            }
        }
    }
}
fn signal(frame: u64) -> f32 {
    (frame % 257) as f32 / 256. - 0.5
}

#[test]
fn segment_events_are_rebased_without_reordering_or_crossing_block_boundaries() {
    let shared = shared();
    let mut port = prepare(&shared, 0);
    process(&mut port, 111).unwrap();
    let mut events = [Event::Parameter {
        frame: 16,
        parameter_id: 9,
        value: 0.3,
    }; 3];
    events[1] = Event::NoteOff {
        frame: 16,
        channel: 15,
        pitch: 127,
        velocity: 0.,
    };
    events[2] = Event::NoteOn {
        frame: 17,
        channel: 0,
        pitch: 60,
        velocity: 1.,
    };
    let mut left = [0.; 128];
    let mut right = [0.; 128];
    port.process(
        Input {
            payload: &[],
            epoch: 0,
            frame: 111,
            left: &[0.; 128],
            right: &[0.; 128],
            events: &events,
        },
        &mut left,
        &mut right,
    )
    .unwrap();
    let first = shared.pending.pop().unwrap();
    assert_eq!(first.event_count, 2);
    assert!(matches!(
        first.events[0],
        Event::Parameter { frame: 127, .. }
    ));
    assert!(matches!(first.events[1], Event::NoteOff { frame: 127, .. }));
    shared.recycle(first, &shared.completed);
    process(&mut port, 17).unwrap();
    let second = shared.pending.pop().unwrap();
    assert_eq!(second.event_count, 1);
    assert!(matches!(second.events[0], Event::NoteOn { frame: 0, .. }));
    shared.recycle(second, &shared.completed);
}

#[test]
fn scheduled_processing_and_invalidation_never_allocate_or_free() {
    let shared = shared();
    let mut port = prepare(&shared, 2);
    let measured = allocations::count(|| {
        for _ in 0..10_000 {
            for frames in [17, 111] {
                process(&mut port, frames).unwrap();
                echo(&shared);
            }
        }
        port.invalidate();
        assert_eq!(process(&mut port, 128), Err(Fault::Invalidated));
    });
    assert_eq!(measured, (0, 0));
    assert_eq!(shared.status(), Status::Invalidated);
}

#[test]
fn preparation_rejects_insufficient_capacity_and_used_ports() {
    let shared = shared();
    let port = RealtimePort {
        shared,
        next_sequence: 0,
        output_events: [crate::wire::Event::Midi {
            frame: 0,
            message: [0x80, 0, 0],
        }; crate::stream_wire::MAX_EVENTS],
        output_event_count: 0,
        output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
    };
    assert_eq!(
        ScheduledPort::prepare(
            port,
            Schedule {
                schedule_version: 1,
                epoch: 0,
                latency_blocks: 3
            }
        )
        .err()
        .unwrap()
        .code,
        "PluginConfigInvalid"
    );
    let shared = shared_with_depth(2);
    let mut port = RealtimePort {
        shared,
        next_sequence: 0,
        output_events: [crate::wire::Event::Midi {
            frame: 0,
            message: [0x80, 0, 0],
        }; crate::stream_wire::MAX_EVENTS],
        output_event_count: 0,
        output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
    };
    port.submit(&[0.; 128], &[0.; 128], &[]).unwrap();
    assert_eq!(
        ScheduledPort::prepare(
            port,
            Schedule {
                schedule_version: 1,
                epoch: 0,
                latency_blocks: 2
            }
        )
        .err()
        .unwrap()
        .code,
        "PluginConfigInvalid"
    );
}
