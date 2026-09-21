use super::*;
use crate::{
    manager_wire::ManagerOptions,
    schedule_wire::Schedule,
    stream::{
        tests::{allocations, shared},
        RealtimePort,
    },
};

fn manager(capacity: usize) -> (Controller, AudioSlot) {
    Controller::new(ManagerOptions {
        manager_version: 1,
        sample_rate: 48000,
        block_size: 128,
        capacity,
    })
    .unwrap()
}
fn prepared(epoch: u64) -> (Box<ScheduledPort>, Arc<crate::stream::Shared>) {
    let shared = shared();
    let port = RealtimePort {
        shared: shared.clone(),
        next_sequence: 0,
        output_events: [crate::wire::Event::Midi {
            frame: 0,
            message: [0x80, 0, 0],
        }; crate::stream_wire::MAX_EVENTS],
        output_event_count: 0,
        output_payload: Vec::with_capacity(oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES),
    };
    (
        Box::new(
            ScheduledPort::prepare(
                port,
                Schedule {
                    schedule_version: 1,
                    epoch,
                    latency_blocks: 2,
                },
            )
            .unwrap(),
        ),
        shared,
    )
}
fn input(epoch: u64, frame: u64) -> Input<'static> {
    Input {
        payload: &[],
        epoch,
        frame,
        left: &[0.25; 128],
        right: &[0.25; 128],
        events: &[],
    }
}

#[test]
fn bounded_fifo_swaps_and_retirement_never_allocate_or_destroy_buffers_on_audio() {
    let (mut controller, mut audio) = manager(16);
    let mut shared_ports = Vec::new();
    for epoch in 0..16 {
        let (port, shared) = prepared(epoch);
        assert!(audio.shared.pending.push(port).is_ok());
        shared_ports.push(shared);
    }
    let measured = allocations::count(|| {
        for epoch in 0..16 {
            assert_eq!(
                audio.activate_next(),
                Ok(Some(ActiveInfo {
                    epoch,
                    latency_frames: 256
                }))
            );
            assert_eq!(audio.active().unwrap().epoch, epoch);
            assert_eq!(audio.shared.retired.len(), epoch as usize);
            if epoch > 0 {
                assert_eq!(
                    shared_ports[epoch as usize - 1].status(),
                    Status::Invalidated
                );
            }
        }
        assert_eq!(audio.activate_next(), Ok(None));
        audio.invalidate();
    });
    assert_eq!(measured, (0, 0));
    assert_eq!(audio.fault(), Some(Fault::Invalidated));
    assert_eq!(audio.shared.retired.len(), 15);
    let reclaimed = allocations::count(|| {
        controller.reclaim();
    });
    assert_eq!(reclaimed.0, 0);
    assert!(
        reclaimed.1 > 0,
        "retired buffers must actually be freed on control"
    );
    assert_eq!(audio.shared.retired.len(), 0);
}

#[test]
fn failed_prepared_port_keeps_current_audio_and_returns_failed_storage_for_reclamation() {
    let (_controller, mut audio) = manager(2);
    let (first, shared) = prepared(0);
    assert!(audio.shared.pending.push(first).is_ok());
    audio.activate_next().unwrap();
    let (failed, failed_shared) = prepared(1);
    failed_shared.stop(Status::Crashed);
    assert!(audio.shared.pending.push(failed).is_ok());
    let mut left = [0.; 128];
    let mut right = [0.; 128];
    let measured = allocations::count(|| {
        assert_eq!(
            audio.activate_next(),
            Err(ActivateError::PreparedFault {
                epoch: 1,
                status: Status::Crashed
            })
        );
        assert_eq!(audio.active().unwrap().epoch, 0);
        for frame in (0..1280).step_by(128) {
            audio
                .process_at(
                    input(0, frame),
                    crate::transport_tests::position(),
                    &mut left,
                    &mut right,
                )
                .unwrap();
            let block = shared.pending.pop().unwrap();
            assert_eq!(block.transport, Some(crate::transport_tests::position()));
            shared.recycle(block, &shared.completed);
            assert_eq!(left, if frame < 256 { [0.; 128] } else { [0.25; 128] });
            assert_eq!(left, right);
        }
    });
    assert_eq!(measured, (0, 0));
    assert_eq!(audio.shared.retired.len(), 1);
    assert_eq!(failed_shared.status(), Status::Crashed);
}

#[test]
fn empty_closed_and_wrong_epoch_paths_clear_audio_without_allocating() {
    let (mut controller, mut audio) = manager(2);
    let mut left = [9.; 129];
    let mut right = [9.; 129];
    assert_eq!(
        allocations::count(|| {
            assert_eq!(
                audio.process(input(0, 0), &mut left, &mut right),
                Err(ProcessError::NotPrepared)
            );
            assert_eq!(audio.activate_next(), Ok(None));
        }),
        (0, 0)
    );
    assert_eq!(left[..128], [0.; 128]);
    assert_eq!(left[128], 9.);
    let (port, _) = prepared(8);
    assert!(audio.shared.pending.push(port).is_ok());
    audio.activate_next().unwrap();
    assert_eq!(
        allocations::count(|| {
            assert_eq!(
                audio.process(input(7, 0), &mut left[..128], &mut right[..128]),
                Err(ProcessError::Schedule(Fault::Discontinuity))
            );
        }),
        (0, 0)
    );
    controller.shutdown();
    assert_eq!(
        allocations::count(|| {
            assert_eq!(audio.activate_next(), Err(ActivateError::Closed));
            assert_eq!(
                audio.process(input(8, 0), &mut left, &mut right),
                Err(ProcessError::Closed)
            );
            audio.invalidate();
        }),
        (0, 0)
    );
    assert_eq!(left[..128], [0.; 128]);
    assert_eq!(right[..128], [0.; 128]);
}

#[test]
fn replacement_never_replays_retired_partial_input_or_completed_audio() {
    let (_controller, mut audio) = manager(2);
    let (old, old_shared) = prepared(0);
    let (new, new_shared) = prepared(1);
    assert!(audio.shared.pending.push(old).is_ok());
    assert!(audio.shared.pending.push(new).is_ok());
    audio.activate_next().unwrap();
    let mut left = [0.; 128];
    let mut right = [0.; 128];
    audio.process(input(0, 0), &mut left, &mut right).unwrap();
    let block = old_shared.pending.pop().unwrap();
    old_shared.recycle(block, &old_shared.completed);
    let mut partial = input(0, 128);
    partial.left = &[0.9; 17];
    partial.right = &[0.9; 17];
    audio
        .process(partial, &mut left[..17], &mut right[..17])
        .unwrap();
    assert_eq!(
        allocations::count(|| {
            audio.activate_next().unwrap();
            for frame in [0, 128, 256] {
                audio
                    .process(input(1, frame), &mut left, &mut right)
                    .unwrap();
                let block = new_shared.pending.pop().unwrap();
                new_shared.recycle(block, &new_shared.completed);
                assert_eq!(left, if frame < 256 { [0.; 128] } else { [0.25; 128] });
            }
        }),
        (0, 0)
    );
    assert_eq!(old_shared.status(), Status::Invalidated);
}

#[test]
fn required_epoch_skips_all_queued_stale_preparations_without_freeing_them() {
    let (_controller, mut audio) = manager(16);
    for epoch in 0..16 {
        assert!(audio.shared.pending.push(prepared(epoch).0).is_ok());
    }
    audio.activate_next().unwrap();
    assert_eq!(
        allocations::count(|| {
            audio.require_epoch(15).unwrap();
            assert_eq!(audio.fault(), Some(Fault::Invalidated));
            assert_eq!(audio.activate_next().unwrap().unwrap().epoch, 15);
            assert_eq!(audio.shared.retired.len(), 15);
            assert_eq!(audio.require_epoch(14), Err(ActivateError::InvalidEpoch));
            assert_eq!(
                audio.require_epoch(u64::MAX),
                Err(ActivateError::InvalidEpoch)
            );
            audio.require_epoch(15).unwrap();
            assert_eq!(audio.fault(), None);
        }),
        (0, 0)
    );
}
