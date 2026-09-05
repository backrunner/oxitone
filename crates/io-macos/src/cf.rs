//! Minimal CoreFoundation string interop: device UIDs and names arrive as
//! `CFStringRef` and are converted to owned Rust strings. Property reads of
//! CFString values follow the Create rule, so every reference obtained here
//! is released before returning.

use std::ffi::c_void;

use coreaudio_sys::CFStringRef;

type CFIndex = i64;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetLength(s: CFStringRef) -> CFIndex;
    fn CFStringGetMaximumSizeForEncoding(length: CFIndex, encoding: u32) -> CFIndex;
    fn CFStringGetCString(
        s: CFStringRef,
        buffer: *mut std::ffi::c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> bool;
    fn CFRelease(cf: *const c_void);
}

/// Convert a retained `CFStringRef` to a Rust `String` and release it.
/// Returns `None` for null references.
pub(crate) fn take_cf_string(reference: CFStringRef) -> Option<String> {
    if reference.is_null() {
        return None;
    }
    // SAFETY: `reference` is a valid, retained CFStringRef obtained from an
    // AudioObject property read (Create rule); it is released exactly once
    // on every path below. The buffer is sized by CoreFoundation itself.
    let text = unsafe {
        let length = CFStringGetLength(reference);
        let capacity = CFStringGetMaximumSizeForEncoding(length, K_CF_STRING_ENCODING_UTF8) + 1;
        let mut buffer = vec![0u8; capacity.max(1) as usize];
        let ok = CFStringGetCString(
            reference,
            buffer.as_mut_ptr() as *mut std::ffi::c_char,
            capacity,
            K_CF_STRING_ENCODING_UTF8,
        );
        let text = if ok {
            let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
            String::from_utf8_lossy(&buffer[..end]).into_owned()
        } else {
            String::new()
        };
        CFRelease(reference as *const c_void);
        text
    };
    Some(text)
}
