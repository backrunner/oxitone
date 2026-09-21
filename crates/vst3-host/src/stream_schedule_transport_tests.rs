use super::*;
use crate::stream::{
    schedule::tests::{echo, prepare},
    tests::{allocations, shared},
};

#[test]
fn aligned_transport_is_bounded_and_not_replayed_into_later_implicit_blocks() {
    let shared = shared();
    let mut port = prepare(&shared, 0);
    let audio = [0.25; 128];
    let mut left = [1.0; 128];
    let mut right = [1.0; 128];
    let measured = allocations::count(|| {
        for sequence in 0..8 {
            port.process_at(
                Input {
                    payload: &[],
                    epoch: 0,
                    frame: sequence * 128,
                    left: &audio,
                    right: &audio,
                    events: &[],
                },
                crate::transport_tests::position(),
                &mut left,
                &mut right,
            )
            .unwrap();
            let block = shared.pending.pop().unwrap();
            assert_eq!(block.transport, Some(crate::transport_tests::position()));
            shared.recycle(block, &shared.completed);
            assert_eq!(left, [if sequence < 2 { 0.0 } else { 0.25 }; 128]);
        }
        port.process(
            Input {
                payload: &[],
                epoch: 0,
                frame: 1024,
                left: &audio,
                right: &audio,
                events: &[],
            },
            &mut left,
            &mut right,
        )
        .unwrap();
        let block = shared.pending.pop().unwrap();
        assert_eq!(block.transport, None);
        shared.recycle(block, &shared.completed);
    });
    assert_eq!(measured, (0, 0));
    echo(&shared);
}

#[test]
fn transport_cannot_silently_replace_context_in_a_partially_assembled_block() {
    let shared = shared();
    let mut port = prepare(&shared, 0);
    let audio = [0.0; 128];
    let mut left = [0.0; 128];
    let mut right = [0.0; 128];
    port.process(
        Input {
            payload: &[],
            epoch: 0,
            frame: 0,
            left: &audio[..17],
            right: &audio[..17],
            events: &[],
        },
        &mut left[..17],
        &mut right[..17],
    )
    .unwrap();
    assert_eq!(
        port.process_at(
            Input {
                payload: &[],
                epoch: 0,
                frame: 17,
                left: &audio,
                right: &audio,
                events: &[]
            },
            crate::transport_tests::position(),
            &mut left,
            &mut right
        ),
        Err(Fault::InvalidBlock)
    );
    assert_eq!(left, [0.0; 128]);
    assert_eq!(port.frame(), 17);
}
