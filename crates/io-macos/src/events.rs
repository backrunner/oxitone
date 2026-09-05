//! Device-change listeners. Listener procs run on HAL notification threads
//! (never on the audio callback thread); they only classify the changed
//! property and forward a `DeviceEvent` over an `mpsc` channel to the
//! session monitor thread, where all rebuild work happens
//! (03-audio-runtime-spec.md §低延迟和设备: 回调只置 atomic/发控制线程队列).

use std::sync::mpsc::Sender;

use coreaudio_sys::{
    kAudioDevicePropertyBufferFrameSize, kAudioDevicePropertyDeviceIsAlive,
    kAudioDevicePropertyNominalSampleRate, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectAddPropertyListener,
    AudioObjectID, AudioObjectPropertyAddress, AudioObjectRemovePropertyListener, OSStatus,
};
use oxitone_core::error::{codes, OxitoneError};

use crate::props::{address, global};

/// Something about the output device chain changed; the monitor thread
/// decides whether to rebuild or pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEvent {
    DefaultDeviceChanged,
    DeviceRemoved,
    NominalRateChanged,
    BufferSizeChanged,
}

struct ListenerClient {
    sender: Sender<DeviceEvent>,
}

/// SAFETY: `client` is a leaked `Box<ListenerClient>` owned by the
/// accompanying `ListenerGuard`, which removes every registration before
/// reclaiming the box, so the pointer is valid for the registration's
/// lifetime. Runs on an HAL notification thread; the channel send is the
/// only effect (bounded work, no locks on the audio path).
unsafe extern "C" fn listener_proc(
    _object: AudioObjectID,
    count: u32,
    addresses: *const AudioObjectPropertyAddress,
    client: *mut std::ffi::c_void,
) -> OSStatus {
    let client = unsafe { &*(client as *const ListenerClient) };
    let addresses = unsafe { std::slice::from_raw_parts(addresses, count as usize) };
    for address in addresses {
        let event = if address.mSelector == kAudioHardwarePropertyDefaultOutputDevice {
            DeviceEvent::DefaultDeviceChanged
        } else if address.mSelector == kAudioDevicePropertyDeviceIsAlive {
            DeviceEvent::DeviceRemoved
        } else if address.mSelector == kAudioDevicePropertyNominalSampleRate {
            DeviceEvent::NominalRateChanged
        } else if address.mSelector == kAudioDevicePropertyBufferFrameSize {
            DeviceEvent::BufferSizeChanged
        } else {
            continue;
        };
        // A full/closed channel only loses a hint; the monitor also
        // re-reads live device state on every rebuild.
        let _ = client.sender.send(event);
    }
    0
}

struct Registration {
    object: AudioObjectID,
    address: AudioObjectPropertyAddress,
}

/// Owns a set of property listeners; removes them on drop before the
/// client box is reclaimed.
pub(crate) struct ListenerGuard {
    registrations: Vec<Registration>,
    client: *mut ListenerClient,
}

impl ListenerGuard {
    pub(crate) fn new(
        device: AudioObjectID,
        sender: Sender<DeviceEvent>,
    ) -> Result<Self, OxitoneError> {
        let client = Box::into_raw(Box::new(ListenerClient { sender }));
        let selectors = [
            (
                kAudioObjectSystemObject,
                kAudioHardwarePropertyDefaultOutputDevice,
            ),
            (device, kAudioDevicePropertyDeviceIsAlive),
            (device, kAudioDevicePropertyNominalSampleRate),
            (device, kAudioDevicePropertyBufferFrameSize),
        ];
        let mut guard = Self {
            registrations: Vec::new(),
            client,
        };
        for (object, selector) in selectors {
            let address = if object == kAudioObjectSystemObject {
                global(selector)
            } else {
                address(selector, kAudioObjectPropertyScopeGlobal)
            };
            // SAFETY: `address` and `client` outlive the registration; the
            // registration is removed in `drop` before either is freed.
            let status = unsafe {
                AudioObjectAddPropertyListener(object, &address, Some(listener_proc), client as _)
            };
            if status != 0 {
                drop(guard);
                return Err(OxitoneError::new(
                    codes::DEVICE_UNAVAILABLE,
                    format!("failed to register device listener (OSStatus {status})"),
                ));
            }
            guard.registrations.push(Registration { object, address });
        }
        Ok(guard)
    }
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        for registration in &self.registrations {
            // SAFETY: pairs each successful `AudioObjectAddPropertyListener`.
            unsafe {
                AudioObjectRemovePropertyListener(
                    registration.object,
                    &registration.address,
                    Some(listener_proc),
                    self.client as _,
                );
            }
        }
        // SAFETY: all registrations using this pointer were removed above,
        // so no callback can observe the box being reclaimed.
        drop(unsafe { Box::from_raw(self.client) });
    }
}

// The raw client pointer is only dereferenced by HAL listener threads while
// a registration is alive; the guard is moved between control threads only.
unsafe impl Send for ListenerGuard {}
