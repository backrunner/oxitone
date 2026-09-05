//! Typed wrappers around `AudioObjectGetPropertyData`/`SetPropertyData`.
//! Everything here runs on control threads (device enumeration, stream
//! setup, rebuild); none of it is reachable from the HAL callback.

use coreaudio_sys::{
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectSetPropertyData, OSStatus,
};

pub(crate) fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: 0,
    }
}

pub(crate) fn global(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: coreaudio_sys::kAudioObjectPropertyScopeGlobal,
        mElement: coreaudio_sys::kAudioObjectPropertyElementMain,
    }
}

/// Read a single POD value. Returns the raw OSStatus on failure.
pub(crate) fn get<T: Copy + Default>(
    object: AudioObjectID,
    address: &AudioObjectPropertyAddress,
) -> Result<T, OSStatus> {
    let mut value = T::default();
    let mut size = std::mem::size_of::<T>() as u32;
    // SAFETY: `value` is a valid, properly sized output buffer.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            address,
            0,
            std::ptr::null(),
            &mut size,
            &mut value as *mut T as *mut _,
        )
    };
    if status == 0 {
        Ok(value)
    } else {
        Err(status)
    }
}

/// Read a variable-length property as raw bytes.
pub(crate) fn get_vec(
    object: AudioObjectID,
    address: &AudioObjectPropertyAddress,
) -> Result<Vec<u8>, OSStatus> {
    let mut size = 0u32;
    // SAFETY: outDataSize is a valid out-pointer; no data buffer is read yet.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(object, address, 0, std::ptr::null(), &mut size) };
    if status != 0 {
        return Err(status);
    }
    let mut buffer = vec![0u8; size as usize];
    // SAFETY: `buffer` is exactly `size` bytes, as CoreFoundation requested.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            address,
            0,
            std::ptr::null(),
            &mut size,
            buffer.as_mut_ptr() as *mut _,
        )
    };
    if status == 0 {
        Ok(buffer)
    } else {
        Err(status)
    }
}

/// Write a single POD value.
pub(crate) fn set<T>(
    object: AudioObjectID,
    address: &AudioObjectPropertyAddress,
    value: &T,
) -> Result<(), OSStatus> {
    // SAFETY: `value` is a valid, properly sized input buffer.
    let status = unsafe {
        AudioObjectSetPropertyData(
            object,
            address,
            0,
            std::ptr::null(),
            std::mem::size_of::<T>() as u32,
            value as *const T as *const _,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}
