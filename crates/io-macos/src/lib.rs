//! oxitone-io-macos — CoreAudio HAL output adapter
//! (03-audio-runtime-spec.md §低延迟和设备). Uses the
//! `AudioDeviceCreateIOProcID` HAL path directly (not AudioUnit HALOutput):
//! the IOProc receives the device's `AudioBufferList` with no AudioUnit
//! graph in between, the pull model lets the callback be a pure
//! ring→output copy, and listener registration is the same AudioObject
//! API either way. AudioQueue/AVAudioEngine are not used; there is no
//! input stream and no input permission request (input scope is never
//! touched — every property address here uses the output/global scope).
//!
//! The HAL callback runs on a CoreAudio realtime thread and obeys the
//! realtime callback invariants of 03: it only calls the user `pull`
//! closure (a ring read + zero-fill owned by `oxitone-render`), copies the
//! result into the output buffers, and never allocates, locks, or touches
//! the clock. Device-property listeners run on HAL notification threads
//! (not the audio thread); they only forward a `DeviceEvent` to the
//! session's control/monitor thread, where all rebuilds happen.

#[cfg(target_os = "macos")]
mod cf;
#[cfg(target_os = "macos")]
mod device;
#[cfg(target_os = "macos")]
mod events;
#[cfg(target_os = "macos")]
mod props;
#[cfg(target_os = "macos")]
mod stream;
#[cfg(target_os = "macos")]
pub mod thread;

#[cfg(target_os = "macos")]
pub use device::{
    available_nominal_sample_rates, default_output_device, device_for_uid, device_is_alive,
    list_output_devices, OutputDeviceInfo,
};
#[cfg(target_os = "macos")]
pub use events::DeviceEvent;
#[cfg(target_os = "macos")]
pub use stream::{OutputStream, PreparedStream, Pull, StreamInfo, StreamRequest};

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
pub use stub::*;
