//! Realtime diagnostics: atomic counters plus bounded event queues
//! (05-performance-and-benchmarks.md §诊断). The callback only updates
//! atomics and pushes fixed-size events; the worker additionally maintains
//! the `engineLoad` EMA and a block-time histogram. The control thread
//! samples everything through `DiagnosticsSnapshot` — no per-block logging.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::ring::SpscQueue;

/// Stable event codes surfaced to TypeScript diagnostics.
pub mod event_codes {
    pub const UNDERRUN: &str = "Underrun";
    pub const PERFORMANCE_WARNING: &str = "PerformanceWarning";
    pub const REALTIME_FAULT: &str = "RealtimeFault";
    pub const DEVICE_CHANGE: &str = "DeviceChange";
    pub const DEVICE_UNAVAILABLE: &str = "DeviceUnavailable";
    pub const MODE_FALLBACK: &str = "ModeFallback";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

const MESSAGE_LEN: usize = 160;

/// Fixed-size diagnostic event (Copy so the RT queues stay allocation-free).
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticEvent {
    pub code: &'static str,
    pub severity: Severity,
    /// Transport frame (project rate) when the event was raised, if known.
    pub frame: u64,
    message: [u8; MESSAGE_LEN],
    message_len: u8,
}

impl DiagnosticEvent {
    pub fn new(code: &'static str, severity: Severity, frame: u64, message: &str) -> Self {
        let bytes = message.as_bytes();
        let len = bytes.len().min(MESSAGE_LEN);
        let mut buffer = [0u8; MESSAGE_LEN];
        buffer[..len].copy_from_slice(&bytes[..len]);
        Self {
            code,
            severity,
            frame,
            message: buffer,
            message_len: len as u8,
        }
    }

    pub fn message(&self) -> &str {
        std::str::from_utf8(&self.message[..self.message_len as usize]).unwrap_or("")
    }
}

/// Producer half of a bounded diagnostic event queue.
pub struct EventProducer<'a> {
    pub(crate) queue: &'a SpscQueue<DiagnosticEvent>,
    pub(crate) drops: &'a AtomicU64,
}

impl EventProducer<'_> {
    pub fn push(&self, event: DiagnosticEvent) {
        if self.queue.push(event).is_err() {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// All realtime counters. `pub` fields are plain atomics read by the
/// control thread at sample time.
pub struct RtCounters {
    pub blocks: AtomicU64,
    pub deadline_misses: AtomicU64,
    pub xruns: AtomicU64,
    pub nan_blocks: AtomicU64,
    pub queue_drops: AtomicU64,
    pub performance_warnings: AtomicU64,
    /// `engineLoad` EMA (block render time / deadline), f64 bits.
    pub engine_load_bits: AtomicU64,
    pub ring_occupancy_frames: AtomicUsize,
    pub consecutive_misses: AtomicU64,
    /// Channel index with the most active voices, published per block by
    /// the worker; used for best-effort underrun attribution.
    pub hottest_channel: AtomicUsize,
    /// Block-time histogram, log2(nanoseconds) buckets (worker-side timing;
    /// zero in direct mode where the callback must not read clocks).
    pub block_time_histogram: [AtomicU64; 64],
}

impl Default for RtCounters {
    fn default() -> Self {
        Self {
            blocks: AtomicU64::new(0),
            deadline_misses: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            nan_blocks: AtomicU64::new(0),
            queue_drops: AtomicU64::new(0),
            performance_warnings: AtomicU64::new(0),
            engine_load_bits: AtomicU64::new(0),
            ring_occupancy_frames: AtomicUsize::new(0),
            consecutive_misses: AtomicU64::new(0),
            hottest_channel: AtomicUsize::new(0),
            block_time_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl RtCounters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn engine_load(&self) -> f64 {
        f64::from_bits(self.engine_load_bits.load(Ordering::Relaxed))
    }

    pub fn store_engine_load(&self, load: f64) {
        self.engine_load_bits
            .store(load.to_bits(), Ordering::Relaxed);
    }

    pub fn record_block_time(&self, nanos: u64) {
        let bucket = (63 - nanos.leading_zeros()).min(62) as usize;
        self.block_time_histogram[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// Percentile estimate from the log2 histogram, in nanoseconds
    /// (returns the containing bucket's upper bound).
    pub fn block_time_percentile(&self, percentile: f64) -> u64 {
        let total: u64 = self
            .block_time_histogram
            .iter()
            .map(|b| b.load(Ordering::Relaxed))
            .sum();
        if total == 0 {
            return 0;
        }
        let target = (total as f64 * percentile / 100.0).ceil() as u64;
        let mut seen = 0u64;
        for (bucket, count) in self.block_time_histogram.iter().enumerate() {
            seen += count.load(Ordering::Relaxed);
            if seen >= target {
                return 1u64 << (bucket + 1);
            }
        }
        u64::MAX
    }

    pub fn block_time_max(&self) -> u64 {
        for bucket in (0..64).rev() {
            if self.block_time_histogram[bucket].load(Ordering::Relaxed) > 0 {
                return 1u64 << (bucket + 1);
            }
        }
        0
    }
}

/// Point-in-time read of every counter plus drained events.
pub struct DiagnosticsSnapshot {
    pub blocks: u64,
    pub deadline_misses: u64,
    pub xruns: u64,
    pub nan_blocks: u64,
    pub queue_drops: u64,
    pub performance_warnings: u64,
    pub engine_load: f64,
    pub ring_occupancy_frames: usize,
    pub block_time_p50_ns: u64,
    pub block_time_p95_ns: u64,
    pub block_time_p99_ns: u64,
    pub block_time_max_ns: u64,
    pub events: Vec<DiagnosticEvent>,
}

impl RtCounters {
    pub fn snapshot(&self, events: Vec<DiagnosticEvent>) -> DiagnosticsSnapshot {
        DiagnosticsSnapshot {
            blocks: self.blocks.load(Ordering::Relaxed),
            deadline_misses: self.deadline_misses.load(Ordering::Relaxed),
            xruns: self.xruns.load(Ordering::Relaxed),
            nan_blocks: self.nan_blocks.load(Ordering::Relaxed),
            queue_drops: self.queue_drops.load(Ordering::Relaxed),
            performance_warnings: self.performance_warnings.load(Ordering::Relaxed),
            engine_load: self.engine_load(),
            ring_occupancy_frames: self.ring_occupancy_frames.load(Ordering::Relaxed),
            block_time_p50_ns: self.block_time_percentile(50.0),
            block_time_p95_ns: self.block_time_percentile(95.0),
            block_time_p99_ns: self.block_time_percentile(99.0),
            block_time_max_ns: self.block_time_max(),
            events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_truncates_long_messages() {
        let event = DiagnosticEvent::new("X", Severity::Warning, 7, &"a".repeat(500));
        assert_eq!(event.message().len(), MESSAGE_LEN);
        assert_eq!(event.frame, 7);
    }

    #[test]
    fn histogram_percentiles() {
        let counters = RtCounters::new();
        for _ in 0..90 {
            counters.record_block_time(100);
        }
        for _ in 0..10 {
            counters.record_block_time(1_000_000);
        }
        assert_eq!(counters.block_time_percentile(50.0), 128);
        assert_eq!(counters.block_time_percentile(99.0), 1 << 20);
        assert_eq!(counters.block_time_max(), 1 << 20);
    }
}
