//! Import-free memory ABI v1. One single-threaded engine per Wasm instance.
//! Raw callers own valid alloc/len pairs and must never reenter an instance.
mod commands;
pub mod engine;
#[cfg(target_family = "wasm")]
mod memory;

use engine::Engine;
use serde_json::json;
use std::cell::RefCell;

struct Host {
    engine: Option<Engine>,
    response: Vec<u8>,
}
thread_local! {
    static HOST: RefCell<Host> = const { RefCell::new(Host { engine: None, response: Vec::new() }) };
}

#[no_mangle]
pub extern "C" fn oxi_abi_version() -> u32 {
    1
}

/// Allocations are limited to 64 MiB per input. A zero pointer means rejected.
#[no_mangle]
pub extern "C" fn oxi_alloc(len: usize) -> *mut u8 {
    if len == 0 || len > 64 * 1024 * 1024 {
        return std::ptr::null_mut();
    }
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// # Safety
/// ptr/len must identify a live allocation from oxi_alloc, freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn oxi_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

/// # Safety
/// Inputs must be valid, nonoverlapping allocated regions for this call.
/// Response bytes remain valid until the next command; PCM until compile/dispose.
#[no_mangle]
pub unsafe extern "C" fn oxi_command(
    ptr: *const u8,
    len: usize,
    data: *const u8,
    data_len: usize,
) -> u32 {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let result = (|| {
            if ptr.is_null()
                || len == 0
                || len > 16 * 1024 * 1024
                || data_len > 64 * 1024 * 1024
                || (data_len != 0 && data.is_null())
            {
                return Err(engine::invalid("input exceeds memory ABI limits"));
            }
            let request = serde_json::from_slice(unsafe { std::slice::from_raw_parts(ptr, len) })
                .map_err(|e| engine::invalid(e.to_string()))?;
            let bytes = if data_len == 0 {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(data, data_len) }
            };
            if host.engine.is_none() {
                host.engine = Some(Engine::new()?);
            }
            host.engine.as_mut().unwrap().command(request, bytes)
        })();
        let (status, value) = match result {
            Ok(value) => (
                0,
                json!({ "protocolVersion": "1.0", "ok": true, "value": value }),
            ),
            Err(error) => (
                1,
                json!({ "protocolVersion": "1.0", "ok": false,
                "error": { "code": error.code, "message": error.message, "path": error.path } }),
            ),
        };
        host.response = serde_json::to_vec(&value).unwrap();
        status
    })
}

#[no_mangle]
pub extern "C" fn oxi_response_ptr() -> *const u8 {
    HOST.with(|h| h.borrow().response.as_ptr())
}
#[no_mangle]
pub extern "C" fn oxi_response_len() -> usize {
    HOST.with(|h| h.borrow().response.len())
}
#[no_mangle]
pub extern "C" fn oxi_binary_ptr() -> *const u8 {
    HOST.with(|h| {
        h.borrow()
            .engine
            .as_ref()
            .map_or(std::ptr::null(), |e| e.binary.as_ptr())
    })
}
#[no_mangle]
pub extern "C" fn oxi_binary_len() -> usize {
    HOST.with(|h| h.borrow().engine.as_ref().map_or(0, |e| e.binary.len()))
}
#[no_mangle]
pub extern "C" fn oxi_left_ptr() -> *const f32 {
    HOST.with(|h| {
        h.borrow()
            .engine
            .as_ref()
            .map_or(std::ptr::null(), |e| e.left.as_ptr())
    })
}
#[no_mangle]
pub extern "C" fn oxi_right_ptr() -> *const f32 {
    HOST.with(|h| {
        h.borrow()
            .engine
            .as_ref()
            .map_or(std::ptr::null(), |e| e.right.as_ptr())
    })
}
#[no_mangle]
pub extern "C" fn oxi_process() -> u32 {
    HOST.with(|h| h.borrow_mut().engine.as_mut().map_or(1, Engine::process))
}
