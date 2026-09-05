//! HAL output stream over `AudioDeviceCreateIOProcID`. The IOProc callback
//! pulls already-device-native interleaved f32 frames from the `pull`
//! closure (owned by `oxitone-render`'s realtime engine) and copies them
//! into the output buffer list. Per 03-audio-runtime-spec.md §Realtime
//! callback 不变量 the callback does no allocation, locking, clock reads,
//! or format conversion — the worker side of `pull` renders and converts,
//! so the only per-callback work here is a bounded copy (plus a one-time
//! FTZ/DAZ enable on the callback thread, 03 §CPU 尖峰防线).

use std::sync::mpsc::Sender;

use coreaudio_sys::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM, AudioDeviceCreateIOProcID, AudioDeviceDestroyIOProcID, AudioDeviceID,
    AudioDeviceIOProcID, AudioDeviceStart, AudioDeviceStop, AudioStreamBasicDescription, OSStatus,
};
use oxitone_core::error::{codes, OxitoneError};

use crate::device;
use crate::events::{DeviceEvent, ListenerGuard};

/// Frame producer invoked from the HAL callback. Must fill the whole
/// interleaved f32 slice (zero-filling on underrun); runs under the
/// realtime callback invariants.
pub type Pull = Box<dyn FnMut(&mut [f32]) + Send + 'static>;

/// What the session asks for; negotiation may fall back with diagnostics.
pub struct StreamRequest {
    pub device: AudioDeviceID,
    /// `deviceRatePolicy: 'adapt-device'` — set the device nominal rate to
    /// the project rate. System-wide effect while it holds (03 §低延迟和
    /// 设备); failure falls back to resampling with a diagnostic.
    pub adapt_sample_rate: Option<f64>,
    /// Target buffer frame size (project `blockSize`, typically 128).
    pub target_buffer_frames: u32,
    /// Hard lower bound from the spec (64).
    pub min_buffer_frames: u32,
    /// Device-change events are forwarded here for the monitor thread.
    pub events: Sender<DeviceEvent>,
}

/// Negotiated stream configuration (input to ring-depth and latency math).
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub device: AudioDeviceID,
    pub sample_rate: f64,
    pub frames_per_slice: u32,
    pub channels: u32,
    pub device_latency_frames: u32,
    pub safety_offset_frames: u32,
    /// False when `adapt-device` failed and the worker must resample.
    pub rate_adapted: bool,
    /// Human-readable fallback notes (surfaced as diagnostics).
    pub diagnostics: Vec<String>,
}

struct CallbackState {
    pull: Pull,
    channels: usize,
    non_interleaved: bool,
    max_frames: usize,
    scratch: Vec<f32>,
    ftz_done: bool,
}

/// SAFETY: `client_data` is a `Box<CallbackState>` leaked in
/// `OutputStream::start` and reclaimed in `OutputStream::drop` after
/// `AudioDeviceStop` + `AudioDeviceDestroyIOProcID`, so it is valid and
/// exclusively owned by this (serially invoked) callback for the stream's
/// lifetime. `output` points at the HAL's buffer list for this call only.
/// RT-safe: no allocation, no locks, no clock reads.
unsafe extern "C" fn io_proc(
    _device: AudioDeviceID,
    _now: *const coreaudio_sys::AudioTimeStamp,
    _input: *const coreaudio_sys::AudioBufferList,
    _input_time: *const coreaudio_sys::AudioTimeStamp,
    output: *mut coreaudio_sys::AudioBufferList,
    _output_time: *const coreaudio_sys::AudioTimeStamp,
    client_data: *mut std::ffi::c_void,
) -> OSStatus {
    let state = unsafe { &mut *(client_data as *mut CallbackState) };
    if !state.ftz_done {
        oxitone_dsp::ftz::set_ftz_daz(true);
        state.ftz_done = true;
    }
    let buffers = unsafe { &mut *output };
    if buffers.mNumberBuffers == 0 {
        return 0;
    }
    let CallbackState {
        pull,
        channels,
        non_interleaved,
        max_frames,
        scratch,
        ..
    } = state;
    let channels = *channels;
    let first = &buffers.mBuffers[0];
    let frames = if *non_interleaved {
        first.mDataByteSize as usize / 4
    } else {
        first.mDataByteSize as usize / (4 * channels.max(1))
    }
    .min(*max_frames);
    if frames == 0 {
        return 0;
    }
    pull(&mut scratch[..frames * channels]);

    if *non_interleaved {
        // Device-native planar layout: one mono buffer per channel. This is
        // a strided copy, not a format conversion (the worker already
        // produced device channel count/order).
        let count = (buffers.mNumberBuffers as usize).min(channels);
        for index in 0..count {
            // SAFETY: `index < mNumberBuffers`, so the buffer slot exists.
            let buffer = unsafe { *buffers.mBuffers.as_ptr().add(index) };
            let n = (buffer.mDataByteSize as usize / 4).min(frames);
            if buffer.mData.is_null() {
                continue;
            }
            let dst = buffer.mData as *mut f32;
            for frame in 0..n {
                // SAFETY: `dst` has `n` f32 slots per the HAL byte size;
                // `scratch` holds `frames * channels` samples and
                // `frame * channels + index` stays inside it.
                unsafe { *dst.add(frame) = scratch[frame * channels + index] };
            }
        }
    } else if !first.mData.is_null() {
        // SAFETY: the HAL buffer holds `frames * channels` f32 samples per
        // its byte size; scratch was just filled with the same count.
        unsafe {
            std::ptr::copy_nonoverlapping(
                scratch.as_ptr(),
                first.mData as *mut f32,
                frames * channels,
            )
        };
    }
    0
}

fn device_unavailable(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::DEVICE_UNAVAILABLE, message)
}

fn adapt_rate(
    request: &StreamRequest,
    diagnostics: &mut Vec<String>,
) -> Result<bool, OxitoneError> {
    let Some(project_rate) = request.adapt_sample_rate else {
        return Ok(false);
    };
    let current = device::nominal_sample_rate(request.device)?;
    if (current - project_rate).abs() < 0.5 {
        return Ok(true);
    }
    if !device::supports_rate(request.device, project_rate) {
        diagnostics.push(format!(
            "device does not support {project_rate} Hz natively; falling back to resample"
        ));
        return Ok(false);
    }
    match device::set_nominal_sample_rate(request.device, project_rate) {
        Ok(()) => Ok(true),
        Err(status) => {
            diagnostics.push(format!(
                "failed to set device nominal rate to {project_rate} Hz (OSStatus {status}); falling back to resample"
            ));
            Ok(false)
        }
    }
}

fn negotiate_buffer_frames(
    request: &StreamRequest,
    diagnostics: &mut Vec<String>,
) -> Result<u32, OxitoneError> {
    let (min, max) = device::buffer_frame_size_range(request.device);
    let target = request
        .target_buffer_frames
        .clamp(min.max(request.min_buffer_frames), max.max(min));
    if device::set_buffer_frame_size(request.device, target).is_ok() {
        return device::buffer_frame_size(request.device);
    }
    let current = device::buffer_frame_size(request.device)?;
    diagnostics.push(format!(
        "failed to set buffer frame size to {target}; keeping device value {current}"
    ));
    Ok(current)
}

/// Force the output stream format to interleaved f32 when the device
/// accepts it; otherwise keep the device format (must still be f32 PCM).
/// Returns `non_interleaved`.
fn configure_format(device: AudioDeviceID, channels: u32, rate: f64) -> Result<bool, OxitoneError> {
    let interleaved = AudioStreamBasicDescription {
        mSampleRate: rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 4 * channels,
        mFramesPerPacket: 1,
        mBytesPerFrame: 4 * channels,
        mChannelsPerFrame: channels,
        mBitsPerChannel: 32,
        mReserved: 0,
    };
    let _ = device::set_stream_format(device, &interleaved);
    let actual = device::stream_format(device)?;
    if actual.mFormatID != kAudioFormatLinearPCM
        || actual.mFormatFlags & kAudioFormatFlagIsFloat == 0
        || actual.mBitsPerChannel != 32
    {
        return Err(device_unavailable(format!(
            "device output format is not 32-bit float PCM (format id {}, flags {:#x}, {} bits)",
            actual.mFormatID, actual.mFormatFlags, actual.mBitsPerChannel
        )));
    }
    Ok(actual.mFormatFlags & kAudioFormatFlagIsNonInterleaved != 0)
}

/// A device negotiated for output but not yet streaming. Splitting
/// negotiation from `start` lets the caller size the render-ahead ring
/// from the negotiated `StreamInfo` before building the pull closure.
pub struct PreparedStream {
    device: AudioDeviceID,
    info: StreamInfo,
    non_interleaved: bool,
    max_frames: usize,
    events: Sender<DeviceEvent>,
}

impl PreparedStream {
    pub fn prepare(request: StreamRequest) -> Result<Self, OxitoneError> {
        let mut diagnostics = Vec::new();
        let rate_adapted = adapt_rate(&request, &mut diagnostics)?;
        let sample_rate = device::nominal_sample_rate(request.device)?;
        let frames_per_slice = negotiate_buffer_frames(&request, &mut diagnostics)?;
        let channels = device::output_channels(request.device);
        if channels == 0 {
            return Err(device_unavailable(
                "output device reports no output channels",
            ));
        }
        let non_interleaved = configure_format(request.device, channels, sample_rate)?;
        let (_, max_frames) = device::buffer_frame_size_range(request.device);
        let max_frames = (max_frames.max(frames_per_slice) as usize).min(16_384);
        Ok(Self {
            device: request.device,
            info: StreamInfo {
                device: request.device,
                sample_rate,
                frames_per_slice,
                channels,
                device_latency_frames: device::device_latency_frames(request.device),
                safety_offset_frames: device::safety_offset_frames(request.device),
                rate_adapted,
                diagnostics,
            },
            non_interleaved,
            max_frames,
            events: request.events,
        })
    }

    pub fn info(&self) -> &StreamInfo {
        &self.info
    }

    pub fn start(self, pull: Pull) -> Result<OutputStream, OxitoneError> {
        let state = Box::into_raw(Box::new(CallbackState {
            pull,
            channels: self.info.channels as usize,
            non_interleaved: self.non_interleaved,
            max_frames: self.max_frames,
            scratch: vec![0.0; self.max_frames * self.info.channels as usize],
            ftz_done: false,
        }));
        let mut proc_id: AudioDeviceIOProcID = None;
        // SAFETY: `state` is a live Box reclaimed in `drop` after the proc
        // is destroyed; `proc_id` is a valid out-pointer.
        let status = unsafe {
            AudioDeviceCreateIOProcID(self.device, Some(io_proc), state as *mut _, &mut proc_id)
        };
        if status != 0 {
            // SAFETY: proc creation failed, so nothing can reference `state`.
            drop(unsafe { Box::from_raw(state) });
            return Err(device_unavailable(format!(
                "AudioDeviceCreateIOProcID failed (OSStatus {status})"
            )));
        }
        let listeners = match ListenerGuard::new(self.device, self.events.clone()) {
            Ok(listeners) => listeners,
            Err(error) => {
                // SAFETY: the proc was created but never started.
                unsafe { AudioDeviceDestroyIOProcID(self.device, proc_id) };
                unsafe { drop(Box::from_raw(state)) };
                return Err(error);
            }
        };
        // SAFETY: `proc_id` was just created for this device.
        let status = unsafe { AudioDeviceStart(self.device, proc_id) };
        if status != 0 {
            // SAFETY: the proc was created above and never started.
            unsafe { AudioDeviceDestroyIOProcID(self.device, proc_id) };
            drop(listeners);
            // SAFETY: the proc is destroyed; nothing references `state`.
            drop(unsafe { Box::from_raw(state) });
            return Err(device_unavailable(format!(
                "AudioDeviceStart failed (OSStatus {status})"
            )));
        }
        Ok(OutputStream {
            device: self.device,
            proc_id,
            state,
            listeners: Some(listeners),
            info: self.info,
        })
    }
}

/// A running HAL output stream. Dropping stops the device proc.
pub struct OutputStream {
    device: AudioDeviceID,
    proc_id: AudioDeviceIOProcID,
    state: *mut CallbackState,
    listeners: Option<ListenerGuard>,
    info: StreamInfo,
}

impl OutputStream {
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }
}

impl Drop for OutputStream {
    fn drop(&mut self) {
        // SAFETY: pairs with the successful create/start in `OutputStream::start`.
        unsafe {
            AudioDeviceStop(self.device, self.proc_id);
            AudioDeviceDestroyIOProcID(self.device, self.proc_id);
        }
        drop(self.listeners.take());
        // SAFETY: the proc is stopped and destroyed, so the callback can no
        // longer run; reclaim the leaked box exactly once.
        drop(unsafe { Box::from_raw(self.state) });
    }
}

// The raw state pointer is only dereferenced by the HAL callback thread
// while the stream is alive; the stream itself is moved between control
// threads only.
unsafe impl Send for OutputStream {}
