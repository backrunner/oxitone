//! Output device enumeration and property negotiation (control thread).
//! Every query maps HAL failures to `DeviceUnavailable` so a missing or
//! unplugged device never panics (03-audio-runtime-spec.md §错误与恢复).

use coreaudio_sys::{
    kAudioDevicePropertyAvailableNominalSampleRates, kAudioDevicePropertyBufferFrameSize,
    kAudioDevicePropertyBufferFrameSizeRange, kAudioDevicePropertyDeviceIsAlive,
    kAudioDevicePropertyDeviceUID, kAudioDevicePropertyLatency,
    kAudioDevicePropertyNominalSampleRate, kAudioDevicePropertySafetyOffset,
    kAudioDevicePropertyStreamConfiguration, kAudioDevicePropertyStreamFormat,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeOutput,
    kAudioObjectSystemObject, AudioDeviceID, AudioObjectID, AudioStreamBasicDescription,
    AudioValueRange, CFStringRef,
};
use oxitone_core::error::{codes, OxitoneError};

use crate::cf::take_cf_string;
use crate::props::{self, address, global};

/// Standard discrete rates used to expand continuous rate ranges for the
/// wire `nominalSampleRates` list.
const STANDARD_RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

fn device_unavailable(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::DEVICE_UNAVAILABLE, message)
}

fn status_context(context: &str, status: i32) -> OxitoneError {
    device_unavailable(format!("{context} (OSStatus {status})"))
}

/// Public output device descriptor (04-api-contracts.md `OutputDeviceInfo`,
/// extended with latency/safety/channel fields for the latency report).
#[derive(Debug, Clone)]
pub struct OutputDeviceInfo {
    /// Stable device UID (survives reboots; used as `outputDeviceId`).
    pub id: String,
    pub name: String,
    /// Discrete supported nominal sample rates (continuous ranges are
    /// expanded to the standard rates they cover).
    pub nominal_sample_rates: Vec<u32>,
    pub buffer_frame_size_range: (u32, u32),
    pub is_default: bool,
    /// `kAudioDevicePropertyLatency` (output scope), frames.
    pub latency_frames: u32,
    /// `kAudioDevicePropertySafetyOffset` (output scope), frames.
    pub safety_offset_frames: u32,
    /// Total output channel count across output streams.
    pub output_channels: u32,
    /// HAL device id (valid for this process boot; not serialized).
    pub device_id: AudioDeviceID,
}

fn device_uid(device: AudioObjectID) -> Option<String> {
    let reference: CFStringRef = props::get(device, &global(kAudioDevicePropertyDeviceUID)).ok()?;
    take_cf_string(reference)
}

fn device_name(device: AudioObjectID) -> String {
    let reference: Option<CFStringRef> = props::get(device, &global(kAudioObjectPropertyName)).ok();
    reference
        .and_then(take_cf_string)
        .unwrap_or_else(|| format!("device {device}"))
}

/// Supported nominal sample rates: discrete points plus the standard rates
/// covered by continuous ranges.
pub fn available_nominal_sample_rates(device: AudioDeviceID) -> Vec<u32> {
    let raw = match props::get_vec(
        device,
        &address(
            kAudioDevicePropertyAvailableNominalSampleRates,
            kAudioObjectPropertyScopeGlobal,
        ),
    ) {
        Ok(raw) => raw,
        Err(_) => return Vec::new(),
    };
    let mut rates = Vec::new();
    for chunk in raw
        .as_chunks::<{ std::mem::size_of::<AudioValueRange>() }>()
        .0
    {
        // SAFETY: `chunk` is exactly one AudioValueRange (two f64s, no
        // padding-sensitive fields; the HAL returns a packed array of them).
        let range: AudioValueRange =
            unsafe { std::ptr::read_unaligned(chunk.as_ptr() as *const _) };
        let (min, max) = (range.mMinimum, range.mMaximum);
        if (max - min).abs() < 0.5 {
            rates.push(min.round() as u32);
        } else {
            for &rate in &STANDARD_RATES {
                if f64::from(rate) >= min && f64::from(rate) <= max {
                    rates.push(rate);
                }
            }
        }
    }
    rates.sort_unstable();
    rates.dedup();
    rates
}

/// Whether `rate` is a supported nominal sample rate (discrete point or
/// inside a continuous range).
pub(crate) fn supports_rate(device: AudioDeviceID, rate: f64) -> bool {
    let raw = match props::get_vec(
        device,
        &address(
            kAudioDevicePropertyAvailableNominalSampleRates,
            kAudioObjectPropertyScopeGlobal,
        ),
    ) {
        Ok(raw) => raw,
        Err(_) => return false,
    };
    raw.as_chunks::<{ std::mem::size_of::<AudioValueRange>() }>()
        .0
        .iter()
        .any(|chunk| {
            // SAFETY: same layout argument as `available_nominal_sample_rates`.
            let range: AudioValueRange =
                unsafe { std::ptr::read_unaligned(chunk.as_ptr() as *const _) };
            rate >= range.mMinimum && rate <= range.mMaximum
        })
}

pub(crate) fn nominal_sample_rate(device: AudioDeviceID) -> Result<f64, OxitoneError> {
    props::get::<f64>(device, &global(kAudioDevicePropertyNominalSampleRate))
        .map_err(|s| status_context("failed to read nominal sample rate", s))
}

pub(crate) fn set_nominal_sample_rate(device: AudioDeviceID, rate: f64) -> Result<(), i32> {
    props::set(
        device,
        &global(kAudioDevicePropertyNominalSampleRate),
        &rate,
    )
}

pub(crate) fn buffer_frame_size(device: AudioDeviceID) -> Result<u32, OxitoneError> {
    props::get::<u32>(device, &global(kAudioDevicePropertyBufferFrameSize))
        .map_err(|s| status_context("failed to read buffer frame size", s))
}

pub(crate) fn set_buffer_frame_size(device: AudioDeviceID, frames: u32) -> Result<(), i32> {
    props::set(
        device,
        &global(kAudioDevicePropertyBufferFrameSize),
        &frames,
    )
}

pub(crate) fn buffer_frame_size_range(device: AudioDeviceID) -> (u32, u32) {
    let range: AudioValueRange =
        match props::get(device, &global(kAudioDevicePropertyBufferFrameSizeRange)) {
            Ok(range) => range,
            Err(_) => return (64, 4096),
        };
    (
        range.mMinimum.max(1.0) as u32,
        range.mMaximum.max(range.mMinimum).max(1.0) as u32,
    )
}

pub(crate) fn device_latency_frames(device: AudioDeviceID) -> u32 {
    props::get::<u32>(
        device,
        &address(kAudioDevicePropertyLatency, kAudioObjectPropertyScopeOutput),
    )
    .unwrap_or(0)
}

pub(crate) fn safety_offset_frames(device: AudioDeviceID) -> u32 {
    props::get::<u32>(
        device,
        &address(
            kAudioDevicePropertySafetyOffset,
            kAudioObjectPropertyScopeOutput,
        ),
    )
    .unwrap_or(0)
}

/// Sum of `mNumberChannels` over all output-stream buffers. The variable-
/// length `AudioBufferList` layout on 64-bit is `u32 count` (padded to 8)
/// followed by packed `AudioBuffer { u32 channels, u32 bytes, *mut data }`
/// entries (16 bytes each, 8-byte aligned).
pub(crate) fn output_channels(device: AudioDeviceID) -> u32 {
    const BUFFER_LEN: usize = 16;
    const BUFFERS_OFFSET: usize = 8;
    let raw = match props::get_vec(
        device,
        &address(
            kAudioDevicePropertyStreamConfiguration,
            kAudioObjectPropertyScopeOutput,
        ),
    ) {
        Ok(raw) => raw,
        Err(_) => return 0,
    };
    if raw.len() < 4 {
        return 0;
    }
    let count = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    let mut channels = 0;
    for index in 0..count {
        let offset = BUFFERS_OFFSET + index * BUFFER_LEN;
        if raw.len() < offset + BUFFER_LEN {
            break;
        }
        channels += u32::from_le_bytes([
            raw[offset],
            raw[offset + 1],
            raw[offset + 2],
            raw[offset + 3],
        ]);
    }
    channels
}

pub fn device_is_alive(device: AudioDeviceID) -> bool {
    props::get::<u32>(device, &global(kAudioDevicePropertyDeviceIsAlive))
        .map(|alive| alive != 0)
        .unwrap_or(false)
}

/// Current output stream format of the first output stream.
pub(crate) fn stream_format(
    device: AudioDeviceID,
) -> Result<AudioStreamBasicDescription, OxitoneError> {
    props::get::<AudioStreamBasicDescription>(
        device,
        &address(
            kAudioDevicePropertyStreamFormat,
            kAudioObjectPropertyScopeOutput,
        ),
    )
    .map_err(|s| status_context("failed to read output stream format", s))
}

pub(crate) fn set_stream_format(
    device: AudioDeviceID,
    format: &AudioStreamBasicDescription,
) -> Result<(), i32> {
    props::set(
        device,
        &address(
            kAudioDevicePropertyStreamFormat,
            kAudioObjectPropertyScopeOutput,
        ),
        format,
    )
}

/// All device IDs with at least one output channel.
fn output_device_ids() -> Result<Vec<AudioObjectID>, OxitoneError> {
    let raw = props::get_vec(
        kAudioObjectSystemObject,
        &global(kAudioHardwarePropertyDevices),
    )
    .map_err(|s| status_context("failed to enumerate audio devices", s))?;
    let mut ids = Vec::new();
    for chunk in raw.as_chunks::<4>().0 {
        let id = AudioObjectID::from_le_bytes(*chunk);
        if output_channels(id) > 0 {
            ids.push(id);
        }
    }
    Ok(ids)
}

pub fn default_output_device() -> Result<Option<AudioDeviceID>, OxitoneError> {
    let id = props::get::<AudioDeviceID>(
        kAudioObjectSystemObject,
        &global(kAudioHardwarePropertyDefaultOutputDevice),
    )
    .map_err(|s| status_context("failed to read the default output device", s))?;
    // `kAudioObjectUnknown` (0) means "no default output".
    Ok((id != 0).then_some(id))
}

fn describe(device: AudioDeviceID, default: Option<AudioDeviceID>) -> Option<OutputDeviceInfo> {
    let id = device_uid(device)?;
    Some(OutputDeviceInfo {
        id,
        name: device_name(device),
        nominal_sample_rates: available_nominal_sample_rates(device),
        buffer_frame_size_range: buffer_frame_size_range(device),
        is_default: default == Some(device),
        latency_frames: device_latency_frames(device),
        safety_offset_frames: safety_offset_frames(device),
        output_channels: output_channels(device),
        device_id: device,
    })
}

/// Every output device visible to the HAL, default first.
pub fn list_output_devices() -> Result<Vec<OutputDeviceInfo>, OxitoneError> {
    let default = default_output_device()?;
    let mut devices: Vec<OutputDeviceInfo> = output_device_ids()?
        .into_iter()
        .filter_map(|id| describe(id, default))
        .collect();
    devices.sort_by_key(|device| !device.is_default);
    Ok(devices)
}

/// Resolve an explicit device UID (falls back to the system default output
/// when `None`) to a HAL device id.
pub fn device_for_uid(uid: Option<&str>) -> Result<AudioDeviceID, OxitoneError> {
    match uid {
        Some(uid) => output_device_ids()?
            .into_iter()
            .find(|&device| device_uid(device).as_deref() == Some(uid))
            .ok_or_else(|| device_unavailable(format!("no output device with UID {uid:?}"))),
        None => default_output_device()?
            .ok_or_else(|| device_unavailable("no default audio output device is available")),
    }
}
