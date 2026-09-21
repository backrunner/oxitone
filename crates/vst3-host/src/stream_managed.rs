//! Bounded ownership transfer; only the control half ever owns Session/process handles.
use super::{
    schedule::{Fault, Input, ScheduledPort},
    Status,
};
use crossbeam_queue::ArrayQueue;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

#[path = "stream_managed_control.rs"]
mod control;
pub use control::Controller;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveInfo {
    pub epoch: u64,
    pub latency_frames: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateError {
    Closed,
    PreparedFault { epoch: u64, status: Status },
    OwnershipFault,
    InvalidEpoch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    Closed,
    NotPrepared,
    Schedule(Fault),
}

struct Shared {
    pending: ArrayQueue<Box<ScheduledPort>>,
    retired: ArrayQueue<Box<ScheduledPort>>,
    closed: AtomicBool,
    minimum_epoch: AtomicU64,
    block_size: usize,
}
impl Shared {
    fn retire(&self, port: Box<ScheduledPort>) -> Result<(), ActivateError> {
        // At most capacity ports exist across both queues and the active slot. Control keeps
        // each session's budget reservation until retirement is collected, even after a crash.
        if let Err(port) = self.retired.push(port) {
            // Defensive invariant failure must not destroy audio buffers on the render thread.
            std::mem::forget(port);
            self.closed.store(true, Ordering::Release);
            return Err(ActivateError::OwnershipFault);
        }
        Ok(())
    }
}

/// Move to the render thread after preparation. Move back to the control thread for destruction.
/// This object is a single slot, not an Engine graph, transport mapper or PDC implementation.
pub struct AudioSlot {
    shared: Arc<Shared>,
    active: Option<Box<ScheduledPort>>,
}
impl AudioSlot {
    /// Call once at an engine block boundary, before its first segment. Retires at most capacity
    /// stale entries and activates the first eligible FIFO replacement. Empty/failed preparation
    /// preserves the existing active instance (which may already have been invalidated).
    /// A successful swap starts at epoch-relative frame zero; caller must use the returned epoch.
    pub fn activate_next(&mut self) -> Result<Option<ActiveInfo>, ActivateError> {
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(ActivateError::Closed);
        }
        for _ in 0..self.shared.pending.capacity() {
            let Some(mut next) = self.shared.pending.pop() else {
                return Ok(None);
            };
            if next.epoch() < self.shared.minimum_epoch.load(Ordering::Acquire) {
                next.invalidate();
                self.shared.retire(next)?;
                continue;
            }
            return self.activate(next);
        }
        Ok(None)
    }
    fn activate(
        &mut self,
        mut next: Box<ScheduledPort>,
    ) -> Result<Option<ActiveInfo>, ActivateError> {
        let status = next.stream_status();
        if status != Status::Running {
            let epoch = next.epoch();
            next.invalidate();
            self.shared.retire(next)?;
            return Err(ActivateError::PreparedFault { epoch, status });
        }
        let info = ActiveInfo {
            epoch: next.epoch(),
            latency_frames: next.latency_frames(),
        };
        if let Some(mut previous) = self.active.replace(next) {
            previous.invalidate();
            self.shared.retire(previous)?;
        }
        Ok(Some(info))
    }
    pub fn active(&self) -> Option<ActiveInfo> {
        self.active.as_ref().map(|port| ActiveInfo {
            epoch: port.epoch(),
            latency_frames: port.latency_frames(),
        })
    }
    pub fn fault(&self) -> Option<Fault> {
        self.active.as_ref().and_then(|port| port.fault())
    }
    /// Stops old epoch data immediately. It stays owned until replacement or control-side drop.
    pub fn invalidate(&mut self) {
        if let Some(active) = &mut self.active {
            active.invalidate();
        }
    }
    /// Monotonic floor for seek/new graph requests, including preparations still in progress.
    /// Stops an older active epoch now; activate_next retires older queued epochs without using them.
    /// Equal epochs are not reset. To restart, request a strictly newer epoch and prepare it.
    pub fn require_epoch(&mut self, epoch: u64) -> Result<(), ActivateError> {
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(ActivateError::Closed);
        }
        if epoch > 9_007_199_254_740_991
            || epoch < self.shared.minimum_epoch.load(Ordering::Acquire)
        {
            return Err(ActivateError::InvalidEpoch);
        }
        self.shared.minimum_epoch.store(epoch, Ordering::Release);
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.epoch() < epoch)
        {
            self.invalidate();
        }
        Ok(())
    }
    /// Same fixed-frame contract as ScheduledPort. No preparation, helper IO, waiting or reclamation.
    pub fn process(
        &mut self,
        input: Input<'_>,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), ProcessError> {
        self.process_inner(input, None, left, right)
    }
    /// Complete aligned block with an explicit VST3 project context; retains epoch ownership checks.
    pub fn process_at(
        &mut self,
        input: Input<'_>,
        transport: crate::transport_wire::Transport,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), ProcessError> {
        self.process_inner(input, Some(transport), left, right)
    }
    fn process_inner(
        &mut self,
        input: Input<'_>,
        transport: Option<crate::transport_wire::Transport>,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), ProcessError> {
        if self.shared.closed.load(Ordering::Acquire) || self.active.is_none() {
            for value in left
                .iter_mut()
                .take(self.shared.block_size)
                .chain(right.iter_mut().take(self.shared.block_size))
            {
                *value = 0.;
            }
            return Err(if self.shared.closed.load(Ordering::Acquire) {
                ProcessError::Closed
            } else {
                ProcessError::NotPrepared
            });
        }
        let active = self.active.as_mut().unwrap();
        let result = if let Some(transport) = transport {
            active.process_at(input, transport, left, right)
        } else {
            active.process(input, left, right)
        };
        result.map_err(ProcessError::Schedule)
    }
}
impl Drop for AudioSlot {
    fn drop(&mut self) {
        // Drop is control-thread only. The controller observes this and closes queued sessions
        // on its next reclaim/shutdown; active buffers are destroyed here, never in activate_next.
        self.shared.closed.store(true, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "stream_managed_tests.rs"]
mod tests;
