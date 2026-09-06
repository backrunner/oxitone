//! Output sinks: the real CoreAudio HAL stream and a simulated sink used
//! by tests and the soak harness (macOS has no null device; the simulated
//! sink drives the same pull closure from a plain thread at the nominal
//! device period). Scheduling-jitter injection lives on the worker side
//! (`realtime::worker::JitterConfig`) because ring underruns are producer
//! starvation, never consumer lateness.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use oxitone_core::error::OxitoneError;
use oxitone_io_macos::{Pull, StreamInfo};

/// A started output stream. `Drop` must stop the device/thread.
pub trait RunningSink: Send {
    fn info(&self) -> &StreamInfo;
}

impl RunningSink for oxitone_io_macos::OutputStream {
    fn info(&self) -> &StreamInfo {
        oxitone_io_macos::OutputStream::info(self)
    }
}

pub struct SimulatedSinkConfig {
    pub sample_rate: f64,
    pub frames_per_slice: u32,
    pub channels: u32,
    pub latency_frames: u32,
    pub safety_offset_frames: u32,
}

/// Sink thread calling `pull` at the nominal device period.
pub struct SimulatedSink {
    info: StreamInfo,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl SimulatedSink {
    pub fn start(config: SimulatedSinkConfig, mut pull: Pull) -> Result<Self, OxitoneError> {
        let info = StreamInfo {
            device: 0,
            sample_rate: config.sample_rate,
            frames_per_slice: config.frames_per_slice,
            channels: config.channels,
            device_latency_frames: config.latency_frames,
            safety_offset_frames: config.safety_offset_frames,
            rate_adapted: false,
            diagnostics: Vec::new(),
        };
        let period =
            Duration::from_secs_f64(f64::from(config.frames_per_slice) / config.sample_rate);
        let channels = config.channels as usize;
        let max_frames = (config.frames_per_slice * 2).max(1024) as usize;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let join = std::thread::Builder::new()
            .name("oxitone-sim-sink".into())
            .spawn(move || {
                let mut scratch = vec![0.0f32; max_frames * channels];
                let frames = config.frames_per_slice as usize;
                let mut next = Instant::now();
                while !stop_thread.load(Ordering::Relaxed) {
                    pull(&mut scratch[..frames * channels]);
                    let now = Instant::now();
                    let (deadline, wait) = next_pull(next, now, period);
                    next = deadline;
                    std::thread::sleep(wait);
                }
            })
            .map_err(|e| {
                OxitoneError::new(
                    oxitone_core::error::codes::DEVICE_UNAVAILABLE,
                    format!("failed to spawn simulated sink thread: {e}"),
                )
            })?;
        Ok(Self {
            info,
            stop,
            join: Some(join),
        })
    }
}

/// Keep the nominal cadence while on time. A missed period starts a fresh
/// interval; an immediate second pull would fabricate consumer starvation.
fn next_pull(previous: Instant, now: Instant, period: Duration) -> (Instant, Duration) {
    let scheduled = previous + period;
    let wait = scheduled
        .checked_duration_since(now)
        .filter(|wait| !wait.is_zero())
        .unwrap_or(period);
    (now + wait, wait)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_time_pulls_preserve_nominal_deadlines() {
        let start = Instant::now();
        let period = Duration::from_millis(3);
        let now = start + Duration::from_millis(1);
        assert_eq!(
            next_pull(start, now, period),
            (start + period, Duration::from_millis(2))
        );
    }

    #[test]
    fn late_pulls_wait_a_full_period_instead_of_bursting() {
        let start = Instant::now();
        let period = Duration::from_millis(3);
        for elapsed in [3, 4, 100] {
            let now = start + Duration::from_millis(elapsed);
            let (deadline, wait) = next_pull(start, now, period);
            assert_eq!(wait, period);
            assert_eq!(deadline, now + period);
            // The following on-time pull also waits; no deferred catch-up.
            assert_eq!(
                next_pull(deadline, deadline, period),
                (deadline + period, period)
            );
        }
    }
}

impl RunningSink for SimulatedSink {
    fn info(&self) -> &StreamInfo {
        &self.info
    }
}

impl Drop for SimulatedSink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
