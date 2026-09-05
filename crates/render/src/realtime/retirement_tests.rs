use super::direct::DirectCore;
use super::ring::{SpscQueue, SpscRing};
use super::tests::{compile_graph, fresh_queues};
use super::worker::{Reconfig, WorkerCore, WorkerMsg};
use oxitone_graph::{PluginInstance, ProcessContext};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

struct DropProbe(Arc<AtomicUsize>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl PluginInstance for DropProbe {
    fn prepare(&mut self, _: f64, _: u32) {}
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for channel in ctx.outputs.iter_mut() {
            channel.fill(0.);
        }
    }
    fn reset(&mut self) {}
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}

#[test]
fn direct_swaps_retire_plugins_without_destroying_them_in_pull() {
    let (commands, events, _, counters, mirror) = fresh_queues();
    let retired = Arc::new(SpscQueue::new(2));
    let slot = Arc::new(Mutex::new(None));
    let drops = Arc::new(AtomicUsize::new(0));
    let mut graph = compile_graph();
    graph.channels[0].instrument = Box::new(DropProbe(drops.clone()));
    graph.seek(1024);
    let mut core = DirectCore::new(
        graph,
        slot,
        commands.clone(),
        retired.clone(),
        events,
        counters,
        mirror.clone(),
        2,
    );
    for _ in 0..3 {
        assert!(commands
            .push(WorkerMsg::ReplaceGraph(compile_graph()))
            .is_ok());
    }
    core.pull(&mut [0.; 256]);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(retired.is_full());
    assert_eq!(mirror.load().1, 1152, "swap retains cursor");
    let old = retired.pop().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(old);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    core.pull(&mut [0.; 256]);
    assert!(
        commands.pop().is_none(),
        "pending swap resumes after reclamation"
    );
    assert_eq!(mirror.load().1, 1280);
}

#[test]
fn worker_defers_graph_and_device_chain_destruction() {
    let (commands, events, _, counters, mirror) = fresh_queues();
    let retired = Arc::new(SpscQueue::new(2));
    let slot = Arc::new(Mutex::new(None));
    let drops = Arc::new(AtomicUsize::new(0));
    let mut graph = compile_graph();
    graph.channels[0].instrument = Box::new(DropProbe(drops.clone()));
    let ring = Arc::new(SpscRing::new(512, 2));
    let old_ring = Arc::downgrade(&ring);
    let mut core = WorkerCore::new(
        graph,
        ring,
        None,
        commands.clone(),
        retired.clone(),
        Arc::new(AtomicBool::new(false)),
        events,
        counters,
        mirror,
        slot,
        2,
        128,
    );
    assert!(commands
        .push(WorkerMsg::ReplaceGraph(compile_graph()))
        .is_ok());
    assert!(commands
        .push(WorkerMsg::Reconfigure(Box::new(Reconfig {
            ring: Arc::new(SpscRing::new(512, 1)),
            resampler: None,
            dev_block_frames: 128,
            channels: 1,
            convert_buf: vec![0.; 128],
            resample_buf: vec![0.; 136],
        })))
        .is_ok());
    core.drain_commands();
    core.render_block();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(old_ring.upgrade().is_some());
    while retired.pop().is_some() {}
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(old_ring.upgrade().is_none());
}

#[test]
fn worker_shutdown_ignores_full_command_and_retirement_queues() {
    let (commands, events, _, counters, mirror) = fresh_queues();
    let retired = Arc::new(SpscQueue::new(2));
    for _ in 0..2 {
        assert!(retired
            .push(WorkerMsg::ReplaceGraph(compile_graph()))
            .is_ok());
    }
    while commands
        .push(WorkerMsg::Transport(super::TransportCmd::Pause))
        .is_ok()
    {}
    let slot = Arc::new(Mutex::new(None));
    let core = WorkerCore::new(
        compile_graph(),
        Arc::new(SpscRing::new(512, 2)),
        None,
        commands,
        retired,
        Arc::new(AtomicBool::new(true)),
        events,
        counters,
        mirror,
        slot.clone(),
        2,
        128,
    );
    std::thread::spawn(move || core.run()).join().unwrap();
    assert!(slot.lock().unwrap().is_some());
}
