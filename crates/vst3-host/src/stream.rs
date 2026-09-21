//! Native asynchronous PCM port. Only submit/receive/status are realtime methods.
//! Spawn, close and destruction belong to control or isolated background execution.
use crate::{
    stream_wire::{Block, Ready},
    wire::Event,
};
use crossbeam_queue::ArrayQueue;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};

#[path = "stream_process.rs"]
mod process;
pub use process::{Session, SessionOptions};
#[path = "stream_control.rs"]
mod control;
pub use control::Controller;
#[path = "stream_audio.rs"]
mod audio;
#[path = "stream_diagnostics.rs"]
mod diagnostics;
pub use audio::BlockContext;
#[path = "stream_managed.rs"]
pub mod managed;
#[path = "stream_schedule.rs"]
pub mod schedule;
pub use diagnostics::Diagnostics;
/// Background/offline waiting only. This is never part of a RealtimePort operation.
pub fn wait_for_completion() {
    crate::stream_thread::IdleWait::new().wait();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Running,
    Closed,
    TimedOut,
    Crashed,
    InvalidResponse,
    PluginFault,
    DeadlineMissed,
    Invalidated,
    ScheduleFault,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortError {
    Full,
    InvalidBlock,
    Closed,
    SequenceExhausted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Received {
    pub sequence: u64,
    pub frames: usize,
    /// Old-layout audio is silent; capture the frozen helper state before rebuilding its graph.
    pub restart_required: bool,
}

struct Shared {
    diagnostics: diagnostics::Counters,
    free: ArrayQueue<Box<Block>>,
    pending: ArrayQueue<Box<Block>>,
    completed: ArrayQueue<Box<Block>>,
    status: AtomicU8,
    restart_required: AtomicBool,
    info: Ready,
    max_frames: usize,
    midi_output: bool,
}
impl Shared {
    fn new(info: Ready, frames: usize, depth: usize, midi_output: bool) -> Arc<Self> {
        let result = Arc::new(Self {
            diagnostics: diagnostics::Counters::default(),
            free: ArrayQueue::new(depth),
            pending: ArrayQueue::new(depth),
            completed: ArrayQueue::new(depth),
            status: AtomicU8::new(0),
            restart_required: AtomicBool::new(false),
            info,
            max_frames: frames,
            midi_output,
        });
        for _ in 0..depth {
            let _ = result.free.push(Box::new(Block::with_buses(
                frames,
                result.info.audio_buses.capacity(),
            )));
        }
        result
    }
    fn status(&self) -> Status {
        match self.status.load(Ordering::Acquire) {
            0 => Status::Running,
            1 => Status::Closed,
            2 => Status::TimedOut,
            3 => Status::Crashed,
            5 => Status::PluginFault,
            6 => Status::DeadlineMissed,
            7 => Status::Invalidated,
            8 => Status::ScheduleFault,
            _ => Status::InvalidResponse,
        }
    }
    fn stop(&self, status: Status) {
        let _ = self
            .status
            .compare_exchange(0, status as u8, Ordering::AcqRel, Ordering::Acquire);
    }
    fn recycle(&self, block: Box<Block>, queue: &ArrayQueue<Box<Block>>) {
        // Exactly depth packets exist across all queues and owners. Each queue has depth capacity.
        // A violated invariant must never deallocate on an audio thread.
        if let Err(block) = queue.push(block) {
            std::mem::forget(block);
            self.stop(Status::InvalidResponse);
        }
    }
}

pub struct RealtimePort {
    shared: Arc<Shared>,
    next_sequence: u64,
    output_events: [Event; crate::stream_wire::MAX_EVENTS],
    output_event_count: usize,
    output_payload: Vec<u8>,
}
impl RealtimePort {
    /// SysEx bytes referenced by output_events; invalidated by the next receive or fault.
    pub fn output_payload(&self) -> &[u8] {
        if self.status() == Status::Running {
            &self.output_payload
        } else {
            &[]
        }
    }
    /// Events of the last successful receive, with offsets relative to its PCM. Borrow until next receive.
    pub fn output_events(&self) -> &[Event] {
        if self.status() == Status::Running {
            &self.output_events[..self.output_event_count]
        } else {
            &[]
        }
    }
    pub fn info(&self) -> &Ready {
        &self.shared.info
    }
    pub fn status(&self) -> Status {
        self.shared.status()
    }
    /// Atomic notification only; the control handle remains usable to recover the frozen state.
    pub fn restart_required(&self) -> bool {
        self.shared.restart_required.load(Ordering::Acquire)
    }
    /// Bounded copy to preallocated storage. Stereo planar input; instruments require silence.
    /// Full means nothing was accepted: the caller must retry or explicitly retire the session.
    pub fn submit(
        &mut self,
        left: &[f32],
        right: &[f32],
        events: &[Event],
    ) -> Result<u64, PortError> {
        self.submit_inner(left, right, events, None, false)
    }
    /// Exact first-sample project context; PCM and context enter the same bounded packet.
    /// This changes the advertised position without resetting processor state.
    pub fn submit_at(
        &mut self,
        left: &[f32],
        right: &[f32],
        events: &[Event],
        transport: crate::transport_wire::Transport,
    ) -> Result<u64, PortError> {
        if !transport.valid() {
            return Err(PortError::InvalidBlock);
        }
        self.submit_inner(left, right, events, Some(transport), false)
    }
    /// Reset processor state immediately before this packet's events/audio, in the same request.
    /// The helper performs setProcessing(false/true); parameter values and sequence are retained.
    pub fn submit_reset_at(
        &mut self,
        left: &[f32],
        right: &[f32],
        events: &[Event],
        transport: crate::transport_wire::Transport,
    ) -> Result<u64, PortError> {
        if !transport.valid() {
            return Err(PortError::InvalidBlock);
        }
        self.submit_inner(left, right, events, Some(transport), true)
    }
    fn submit_inner(
        &mut self,
        left: &[f32],
        right: &[f32],
        events: &[Event],
        transport: Option<crate::transport_wire::Transport>,
        reset: bool,
    ) -> Result<u64, PortError> {
        self.submit_buses(
            &[[left, right]],
            events,
            BlockContext {
                transport,
                reset,
                payload: &[],
            },
        )
    }
    /// Stereo convenience adapter. Multibus output requires receive_buses; nothing is discarded.
    pub fn receive(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<Option<Received>, PortError> {
        self.receive_buses(&mut [[left, right]])
    }
}
impl Drop for RealtimePort {
    fn drop(&mut self) {
        self.shared.stop(Status::Closed);
    }
}

#[cfg(test)]
#[path = "stream_bus_tests.rs"]
mod bus_tests;
#[cfg(test)]
#[path = "sysex_port_tests.rs"]
mod sysex_tests;
#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
