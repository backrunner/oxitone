//! Metronome controls use the same allocation-free block boundary in both execution modes.
use super::{
    direct::DirectCore,
    ring::{SpscQueue, SpscRing},
    tests::{fresh_queues, playable_snapshot},
    worker::{WorkerCore, WorkerMsg},
    TransportCmd,
};
use crate::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use std::sync::{atomic::AtomicBool, Arc, Mutex};

fn graph() -> Box<RenderGraph> {
    let mut snapshot = playable_snapshot();
    snapshot.channels[0].level = 0.;
    let mut graph = Box::new(
        RenderGraph::compile(
            &snapshot,
            &builtin_registry().unwrap(),
            &SampleStore::new(None),
            &RenderGraphOptions {
                master_limiter: false,
                metronome_level: Some(0.5),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    graph.transport_mut().play_from(0, None);
    graph.set_metronome_enabled(false);
    graph
}

#[test]
fn queued_metronome_toggles_are_sample_accurate_in_direct_and_buffered_modes() {
    let (commands, events, _, counters, mirror) = fresh_queues();
    let mut direct = DirectCore::new(
        graph(),
        Arc::new(Mutex::new(None)),
        commands.clone(),
        Arc::new(SpscQueue::new(16)),
        events,
        counters,
        mirror.clone(),
        2,
    );
    let (worker_commands, events, _, counters, worker_mirror) = fresh_queues();
    let ring = Arc::new(SpscRing::new(512, 2));
    let mut worker = WorkerCore::new(
        graph(),
        ring.clone(),
        None,
        worker_commands.clone(),
        Arc::new(SpscQueue::new(16)),
        Arc::new(AtomicBool::new(false)),
        events,
        counters,
        worker_mirror.clone(),
        Arc::new(Mutex::new(None)),
        2,
        128,
    );
    for (enabled, frame, audible) in [
        (false, 0, false),
        (true, 24000, true),
        (false, 48000, false),
        (true, 60000, false),
        (true, 96000, true),
    ] {
        for queue in [&commands, &worker_commands] {
            assert!(queue
                .push(WorkerMsg::Transport(TransportCmd::Seek { frame }))
                .is_ok());
            assert!(queue.push(WorkerMsg::Metronome(enabled)).is_ok());
        }
        let mut actual = [0.; 256];
        let mut buffered = [0.; 256];
        direct.pull(&mut actual);
        worker.drain_commands();
        worker.render_block();
        assert_eq!(ring.read_frames(&mut buffered), 128);
        assert_eq!(actual, buffered);
        let peak = actual
            .iter()
            .fold(0_f32, |peak, sample| peak.max(sample.abs()));
        if audible {
            assert!(peak > 0.2);
        } else {
            assert_eq!(peak, 0.);
        }
        assert_eq!(mirror.load().1, frame + 128);
        assert_eq!(worker_mirror.load().1, frame + 128);
    }
}
