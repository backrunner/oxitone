//! Control-side prepare/publish/reap. None of these methods are realtime-safe.
use super::{ActiveInfo, AudioSlot, Shared};
use crate::{
    manager_wire::ManagerOptions,
    schedule_wire::Schedule,
    stream::{schedule::ScheduledPort, Session, SessionOptions, Status},
    stream_wire::{Ready, Start},
    Error, Result,
};
use crossbeam_queue::ArrayQueue;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

struct Owned {
    epoch: u64,
    latency_frames: u32,
    session: Session,
}
pub struct Controller {
    shared: Arc<Shared>,
    options: ManagerOptions,
    sessions: Vec<Owned>,
    last_epoch: Option<u64>,
}
impl Controller {
    /// Allocates ownership queues, but does not load any plugin or open an audio device.
    pub fn new(options: ManagerOptions) -> Result<(Self, AudioSlot)> {
        options.validate()?;
        let shared = Arc::new(Shared {
            pending: ArrayQueue::new(options.capacity),
            retired: ArrayQueue::new(options.capacity),
            closed: AtomicBool::new(false),
            minimum_epoch: AtomicU64::new(0),
            block_size: options.block_size,
        });
        Ok((
            Self {
                shared: shared.clone(),
                options,
                sessions: Vec::with_capacity(options.capacity),
                last_epoch: None,
            },
            AudioSlot {
                shared,
                active: None,
            },
        ))
    }
    /// Fully prepares before publishing. Failure leaves the active slot and queued replacements
    /// untouched. Epochs of successful publications must strictly increase, without wrap/reuse.
    /// Capacity includes pending, active and retired sessions; call reclaim regularly.
    pub fn prepare(
        &mut self,
        path: &Path,
        start: Start,
        schedule: Schedule,
        options: SessionOptions,
    ) -> Result<()> {
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(Error::new("RealtimeFault", "VST3 manager is closed"));
        }
        start.validate()?;
        if start.options.midi_output {
            return Err(crate::unsupported(
                "PCM session manager does not expose MIDI output; use the isolated project graph",
            ));
        }
        schedule.validate()?;
        self.check_epoch(schedule.epoch)?;
        if start.options.sample_rate != self.options.sample_rate
            || start.options.block_size != self.options.block_size
        {
            return Err(crate::invalid(
                "VST3 replacement cannot change the manager audio format",
            ));
        }
        if self.last_epoch.is_some_and(|epoch| schedule.epoch <= epoch) {
            return Err(crate::invalid("VST3 replacement epoch must increase"));
        }
        if schedule.latency_blocks > options.queue_depth {
            return Err(crate::invalid(
                "VST3 scheduling latency exceeds queue capacity",
            ));
        }
        self.reclaim();
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(Error::new("RealtimeFault", "VST3 manager is closed"));
        }
        if self.sessions.len() == self.options.capacity {
            return Err(Error::new(
                "BudgetExceeded",
                "VST3 session capacity is full",
            ));
        }
        let (session, port) = Session::spawn(path, start, options)?;
        let port = Box::new(ScheduledPort::prepare(port, schedule)?);
        self.check_epoch(schedule.epoch)?;
        // AudioSlot may have been dropped while the helper was initializing.
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(Error::new(
                "RealtimeFault",
                "VST3 manager closed during preparation",
            ));
        }
        self.sessions.push(Owned {
            epoch: schedule.epoch,
            latency_frames: port.latency_frames(),
            session,
        });
        if let Err(port) = self.shared.pending.push(port) {
            drop(port);
            self.sessions.pop();
            return Err(Error::new(
                "BudgetExceeded",
                "VST3 publication queue is full",
            ));
        }
        self.last_epoch = Some(schedule.epoch);
        if self.shared.closed.load(Ordering::Acquire) {
            self.shutdown();
            return Err(Error::new(
                "RealtimeFault",
                "VST3 manager closed during publication",
            ));
        }
        Ok(())
    }
    fn check_epoch(&self, epoch: u64) -> Result<()> {
        if epoch < self.shared.minimum_epoch.load(Ordering::Acquire) {
            return Err(Error::new(
                "SourceChanged",
                "VST3 preparation belongs to an obsolete epoch",
            ));
        }
        Ok(())
    }
    pub fn live_sessions(&self) -> usize {
        self.sessions.len()
    }
    /// Available before activation, so a future graph adapter can compile compensation off-thread.
    pub fn info(&self, epoch: u64) -> Option<ActiveInfo> {
        self.sessions
            .iter()
            .find(|owned| owned.epoch == epoch)
            .map(|owned| ActiveInfo {
                epoch,
                latency_frames: owned.latency_frames,
            })
    }
    pub fn plugin_info(&self, epoch: u64) -> Option<&Ready> {
        self.sessions
            .iter()
            .find(|owned| owned.epoch == epoch)
            .map(|owned| owned.session.info())
    }
    pub fn status(&self, epoch: u64) -> Option<Status> {
        self.sessions
            .iter()
            .find(|owned| owned.epoch == epoch)
            .map(|owned| owned.session.status())
    }
    /// Control-side diagnostics; querying a PID never transfers process ownership to audio.
    pub fn pid(&self, epoch: u64) -> Option<u32> {
        self.sessions
            .iter()
            .find(|owned| owned.epoch == epoch)
            .map(|owned| owned.session.pid())
    }
    pub fn diagnostics(&self, epoch: u64) -> Option<crate::stream::Diagnostics> {
        self.sessions
            .iter()
            .find(|owned| owned.epoch == epoch)
            .map(|owned| owned.session.diagnostics())
    }
    /// Retired ports are no longer reachable by audio. Close/reap each helper, then release its
    /// buffers and capacity reservation here. A dead active helper retains its reservation.
    pub fn reclaim(&mut self) -> usize {
        if self.shared.closed.load(Ordering::Acquire) {
            let count = self.sessions.len();
            self.shutdown();
            return count;
        }
        let mut count = 0;
        while let Some(port) = self.shared.retired.pop() {
            if let Some(index) = self
                .sessions
                .iter()
                .position(|owned| owned.epoch == port.epoch())
            {
                let mut owned = self.sessions.swap_remove(index);
                owned.session.close();
                count += 1;
            }
            drop(port);
        }
        count
    }
    /// Terminal and idempotent. Active buffers remain owned by AudioSlot until it is returned
    /// to the control thread; current/concurrent render calls never wait for this cleanup.
    pub fn shutdown(&mut self) {
        self.shared.closed.store(true, Ordering::Release);
        for owned in &mut self.sessions {
            owned.session.close();
        }
        self.sessions.clear();
        while self.shared.pending.pop().is_some() {}
        while self.shared.retired.pop().is_some() {}
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shutdown();
    }
}
