//! Non-macOS stub: same API surface, every operation reports
//! `DeviceUnavailable`. Oxitone is macOS-first (00-roadmap.md); this keeps
//! the workspace compilable elsewhere for type-checking only.

use std::sync::mpsc::Sender;

use oxitone_core::error::{codes, OxitoneError};

fn unsupported() -> OxitoneError {
    OxitoneError::new(
        codes::DEVICE_UNAVAILABLE,
        "audio output is only implemented on macOS",
    )
}

pub type AudioDeviceId = u32;
pub type Pull = Box<dyn FnMut(&mut [f32]) + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEvent {
    DefaultDeviceChanged,
    DeviceRemoved,
    NominalRateChanged,
    BufferSizeChanged,
}

#[derive(Debug, Clone)]
pub struct OutputDeviceInfo {
    pub id: String,
    pub name: String,
    pub nominal_sample_rates: Vec<u32>,
    pub buffer_frame_size_range: (u32, u32),
    pub is_default: bool,
    pub latency_frames: u32,
    pub safety_offset_frames: u32,
    pub output_channels: u32,
    pub device_id: AudioDeviceId,
}

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub device: AudioDeviceId,
    pub sample_rate: f64,
    pub frames_per_slice: u32,
    pub channels: u32,
    pub device_latency_frames: u32,
    pub safety_offset_frames: u32,
    pub rate_adapted: bool,
    pub diagnostics: Vec<String>,
}

pub struct StreamRequest {
    pub device: AudioDeviceId,
    pub adapt_sample_rate: Option<f64>,
    pub target_buffer_frames: u32,
    pub min_buffer_frames: u32,
    pub events: Sender<DeviceEvent>,
}

pub struct PreparedStream {
    info: StreamInfo,
}

impl PreparedStream {
    pub fn prepare(_request: StreamRequest) -> Result<Self, OxitoneError> {
        Err(unsupported())
    }

    pub fn info(&self) -> &StreamInfo {
        &self.info
    }

    pub fn start(self, _pull: Pull) -> Result<OutputStream, OxitoneError> {
        Err(unsupported())
    }
}

pub struct OutputStream {
    info: StreamInfo,
}

impl OutputStream {
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }
}

pub fn list_output_devices() -> Result<Vec<OutputDeviceInfo>, OxitoneError> {
    Err(unsupported())
}

pub fn default_output_device() -> Result<Option<AudioDeviceId>, OxitoneError> {
    Err(unsupported())
}

pub fn device_for_uid(_uid: Option<&str>) -> Result<AudioDeviceId, OxitoneError> {
    Err(unsupported())
}

pub mod thread {
    pub fn set_time_constraint(
        _period: std::time::Duration,
        _computation: std::time::Duration,
        _constraint: std::time::Duration,
    ) -> bool {
        false
    }
}
