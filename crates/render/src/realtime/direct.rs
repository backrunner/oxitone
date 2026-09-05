//! `latencyMode: 'direct'` — the HAL callback renders the graph itself
//! (03-audio-runtime-spec.md §线程模型: 同一 graph 在两种模式下输出必须
//! sample-accurate 一致). Used only when the device runs at the project
//! rate with `frames_per_slice == block_size`; the session falls back to
//! buffered mode otherwise (with a diagnostic).
//!
//! The callback still obeys the realtime invariants: no allocation, no
//! locks, no clock reads. Per spec the only addition over the buffered
//! callback is the block render plus the final stereo→device interleave
//! (there is no worker to pre-convert in this mode); no resampling is
//! possible, which is why a rate match is required. Timing instrumentation
//! is intentionally absent here (the callback must not read clocks), so
//! `deadlineMisses`/histogram stay at zero in direct mode.

use std::sync::Arc;

use oxitone_io_macos::Pull;

use super::diagnostics::{event_codes, DiagnosticEvent, EventProducer, RtCounters, Severity};
use super::layout::stereo_to_device;
use super::ring::SpscQueue;
use super::worker::{apply_transport, ReturnSlot, TransportMirror, WorkerMsg};
use crate::graph::RenderGraph;

pub(crate) struct DirectCore {
    graph: Option<Box<RenderGraph>>,
    return_slot: ReturnSlot,
    commands: Arc<SpscQueue<WorkerMsg>>,
    events: Arc<SpscQueue<DiagnosticEvent>>,
    counters: Arc<RtCounters>,
    mirror: Arc<TransportMirror>,
    block_left: Vec<f32>,
    block_right: Vec<f32>,
    channels: usize,
    block_size: usize,
    fault_latched: bool,
}

impl DirectCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        graph: Box<RenderGraph>,
        return_slot: ReturnSlot,
        commands: Arc<SpscQueue<WorkerMsg>>,
        events: Arc<SpscQueue<DiagnosticEvent>>,
        counters: Arc<RtCounters>,
        mirror: Arc<TransportMirror>,
        channels: usize,
    ) -> Self {
        let block_size = graph.block_size();
        Self {
            graph: Some(graph),
            return_slot,
            commands,
            events,
            counters,
            mirror,
            block_left: vec![0.0; block_size],
            block_right: vec![0.0; block_size],
            channels,
            block_size,
            fault_latched: false,
        }
    }

    fn handle(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Transport(cmd) => {
                if let Some(graph) = self.graph.as_mut() {
                    apply_transport(graph, &cmd, &self.mirror);
                }
            }
            WorkerMsg::Param(event) => {
                if let Some(graph) = self.graph.as_mut() {
                    graph.insert_queued_parameter(event);
                }
            }
            WorkerMsg::ReplaceGraph(mut graph) => {
                if let Some(old) = self.graph.as_ref() {
                    graph.transport.state = old.transport.state;
                    graph.transport.cursor = old.transport.cursor;
                    graph.transport.loop_region = old.transport.loop_region;
                }
                self.block_size = graph.block_size();
                self.block_left.resize(self.block_size, 0.0);
                self.block_right.resize(self.block_size, 0.0);
                self.fault_latched = false;
                let state = graph.state();
                let cursor = graph.transport().cursor;
                self.graph = Some(graph);
                self.mirror.store(state, cursor);
            }
            // Device rebuilds are handled by the monitor thread tearing
            // the stream down; nothing for the callback to do.
            WorkerMsg::Reconfigure(_) | WorkerMsg::Shutdown => {}
        }
    }

    /// Callback body: drain commands, render one block, interleave to the
    /// device layout. RT-safe (no allocation, locks, or clock reads).
    pub(crate) fn pull(&mut self, out: &mut [f32]) {
        while let Some(msg) = self.commands.pop() {
            self.handle(msg);
        }
        let Some(graph) = self.graph.as_mut() else {
            out.fill(0.0);
            return;
        };
        let frames = out.len() / self.channels;
        if frames != self.block_size {
            // The session guarantees frames_per_slice == block_size before
            // choosing direct mode; stay silent rather than corrupt state.
            out.fill(0.0);
            return;
        }
        graph.process_block(&mut self.block_left, &mut self.block_right);
        self.counters
            .blocks
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if graph.faulted() && !self.fault_latched {
            self.fault_latched = true;
            self.counters
                .nan_blocks
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            EventProducer {
                queue: &self.events,
                drops: &self.counters.queue_drops,
            }
            .push(DiagnosticEvent::new(
                event_codes::REALTIME_FAULT,
                Severity::Error,
                graph.transport().cursor,
                "non-finite samples detected; block muted and transport stopped",
            ));
            graph.transport_mut().stop();
        }
        stereo_to_device(&self.block_left, &self.block_right, out, self.channels);
        self.mirror.store(graph.state(), graph.transport().cursor);
    }

    /// Box this core as the HAL pull closure.
    pub(crate) fn into_pull(mut self) -> Pull {
        Box::new(move |out: &mut [f32]| self.pull(out))
    }
}

impl Drop for DirectCore {
    fn drop(&mut self) {
        // Runs on a control thread after the stream has stopped; hands the
        // graph back so the rebuild can reuse it.
        if let Ok(mut slot) = self.return_slot.lock() {
            *slot = self.graph.take();
        }
    }
}
