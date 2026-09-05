//! Realtime device playback: render-ahead worker + SPSC ring + CoreAudio
//! HAL output (03-audio-runtime-spec.md §低延迟和设备, §线程模型与平滑
//! 播放, §CPU 尖峰防线). See `session` for the threading map.

pub mod diagnostics;
mod direct;
pub mod layout;
#[cfg(test)]
mod retirement_tests;
pub mod ring;
mod session;
mod sink;
#[cfg(test)]
mod tests;
mod worker;

pub use diagnostics::{event_codes, DiagnosticEvent, DiagnosticsSnapshot, RtCounters, Severity};
pub use session::{OutputLatencyReport, RealtimeConfig, RealtimeSession, SessionStartError};
pub use sink::SimulatedSinkConfig;
pub use worker::{JitterConfig, TransportCmd, TransportMirror};
