//! IO/helper timing only. No clock reads or diagnostic writes on submit/receive/process.
use serde::Serialize;
use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

#[derive(Default)]
pub(super) struct Counters {
    completed: AtomicU64,
    idle: AtomicU64,
    send: AtomicU64,
    reply: AtomicU64,
    processing: AtomicU64,
    pub(super) time_constraint: AtomicBool,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub completed_blocks: u64,
    pub maximum_idle_wait_ms: f64,
    pub maximum_send_ms: f64,
    pub maximum_reply_wait_ms: f64,
    pub maximum_helper_processing_ms: f64,
    pub worker_time_constraint: bool,
    pub helper_time_constraint: bool,
}
impl Counters {
    pub(super) fn idle(&self, elapsed: Duration) {
        maximum(&self.idle, elapsed);
    }
    pub(super) fn sent(&self, elapsed: Duration) {
        maximum(&self.send, elapsed);
    }
    pub(super) fn replied(&self, elapsed: Duration, processing_micros: u32) {
        maximum(&self.reply, elapsed);
        self.processing
            .fetch_max(u64::from(processing_micros) * 1000, Ordering::Relaxed);
        self.completed.fetch_add(1, Ordering::Relaxed);
    }
    pub(super) fn snapshot(&self, helper_time_constraint: bool) -> Diagnostics {
        let ms = |counter: &AtomicU64| counter.load(Ordering::Relaxed) as f64 / 1_000_000.;
        Diagnostics {
            completed_blocks: self.completed.load(Ordering::Relaxed),
            maximum_idle_wait_ms: ms(&self.idle),
            maximum_send_ms: ms(&self.send),
            maximum_reply_wait_ms: ms(&self.reply),
            maximum_helper_processing_ms: ms(&self.processing),
            worker_time_constraint: self.time_constraint.load(Ordering::Relaxed),
            helper_time_constraint,
        }
    }
}
fn maximum(counter: &AtomicU64, elapsed: Duration) {
    counter.fetch_max(
        elapsed.as_nanos().min(u64::MAX as u128) as u64,
        Ordering::Relaxed,
    );
}
