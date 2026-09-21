use super::*;
use crate::stream::tests::{allocations, shared};
use oxitone_core::midi_bytes::MidiBytes;

fn put(
    port: &mut ScheduledPort,
    frames: usize,
    events: &[Event],
    payload: &[u8],
) -> Result<(), Fault> {
    let audio = [0.; 128];
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    port.process(
        Input {
            epoch: port.epoch(),
            frame: port.frame(),
            left: &audio[..frames],
            right: &audio[..frames],
            events,
            payload,
        },
        &mut left[..frames],
        &mut right[..frames],
    )
}

#[test]
fn scheduled_sysex_rebases_ranges_across_partial_blocks_without_allocating() {
    let shared = shared();
    let mut port = tests::prepare(&shared, 1);
    let payload = [0xf0, 1, 0xf7];
    let event = |frame| Event::SysEx {
        frame,
        data: MidiBytes {
            offset: 0,
            length: 3,
        },
    };
    let counts = allocations::count(|| {
        put(&mut port, 64, &[event(63)], &payload).unwrap();
        put(&mut port, 128, &[event(0), event(127)], &payload).unwrap();
        let mut block = shared.pending.pop().unwrap();
        assert_eq!(block.event_count, 2);
        assert_eq!(block.payload, [0xf0, 1, 0xf7, 0xf0, 1, 0xf7]);
        for (i, event) in block.events[..2].iter().enumerate() {
            let Event::SysEx { frame, data } = event else {
                panic!("lost SysEx")
            };
            assert_eq!(*frame, [63, 64][i]);
            assert_eq!(data.offset, (i * 3) as u32);
            assert_eq!(data.get(&block.payload).unwrap(), payload);
        }
        block.payload.clear();
        block.event_count = 0;
        shared.recycle(block, &shared.completed);
        put(&mut port, 64, &[], &[]).unwrap();
        let block = shared.pending.pop().unwrap();
        assert_eq!(block.event_count, 1);
        assert!(matches!(
            block.events[0],
            Event::SysEx {
                frame: 63,
                data: MidiBytes {
                    offset: 0,
                    length: 3
                }
            }
        ));
        assert_eq!(block.payload, payload);
        shared.recycle(block, &shared.free);
        port.invalidate();
    });
    assert_eq!(counts, (0, 0));
}

#[test]
fn aggregate_payload_overflow_rejects_before_accepting_a_partial_call() {
    let shared = shared();
    let mut port = tests::prepare(&shared, 1);
    let mut payload = [0x7d; 4096];
    payload[0] = 0xf0;
    payload[4095] = 0xf7;
    let event = Event::SysEx {
        frame: 0,
        data: MidiBytes {
            offset: 0,
            length: 4096,
        },
    };
    put(&mut port, 64, &[event; 4], &payload).unwrap();
    assert_eq!(
        put(&mut port, 64, &[event], &payload),
        Err(Fault::EventOverflow)
    );
    assert_eq!(port.frame(), 64);
    assert_eq!(port.input.event_count, 4);
    assert_eq!(port.input.payload.len(), 16384);
}
