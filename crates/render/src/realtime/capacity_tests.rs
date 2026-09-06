//! Deterministic ring-horizon checks without OS scheduling or audio devices.
use super::{
    ring::{SpscQueue, SpscRing},
    tests::{compile_graph, fresh_queues},
    worker::{ResamplerPair, WorkerCore},
};
use std::sync::{atomic::AtomicBool, Arc, Mutex};

fn worker(ring: Arc<SpscRing>, resampler: Option<Box<ResamplerPair>>) -> WorkerCore {
    let (commands, events, _, counters, mirror) = fresh_queues();
    WorkerCore::new(
        compile_graph(),
        ring,
        resampler,
        commands,
        Arc::new(SpscQueue::new(16)),
        Arc::new(AtomicBool::new(false)),
        events,
        counters,
        mirror,
        Arc::new(Mutex::new(None)),
        2,
        128,
    )
}

#[test]
fn unchanged_rate_can_fill_and_refill_the_entire_four_block_horizon() {
    let ring = Arc::new(SpscRing::new(512, 2));
    let mut core = worker(ring.clone(), None);
    for block in 0..4 {
        assert!(
            core.has_render_capacity(),
            "missing capacity for block {block}"
        );
        core.render_block();
    }
    assert!(!core.has_render_capacity());
    assert_eq!(ring.available_to_read_frames(), 512);
    assert_eq!(ring.read_frames(&mut [0.; 256]), 128);
    assert!(core.has_render_capacity());
    core.render_block();
    assert_eq!(ring.available_to_read_frames(), 512);
}

#[test]
fn resampling_still_reserves_its_extra_output_headroom() {
    let ring = Arc::new(SpscRing::new(512, 2));
    let core = worker(
        ring.clone(),
        Some(Box::new(ResamplerPair::new(48_000., 44_100., 128, 128))),
    );
    assert!(core.has_render_capacity());
    assert_eq!(ring.write_frames(&[0.; 768]), 384);
    assert!(!core.has_render_capacity()); // 128 free frames < 128 + 8.
    assert_eq!(ring.read_frames(&mut [0.; 16]), 8);
    assert!(core.has_render_capacity());
}
