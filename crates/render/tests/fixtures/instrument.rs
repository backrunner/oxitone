//! Standalone Rust cdylib fixture, compiled with rustc (no Cargo dependencies).
use std::ffi::{c_char, c_void};

#[repr(C)]
pub struct NoteEvent {
    frame_offset: u32,
    kind: u32,
    pitch: u8,
    velocity: f32,
}

#[repr(C)]
pub struct OxiPluginDescriptorV1 {
    abi_major: u32,
    abi_minor: u32,
    plugin_id: *const c_char,
    plugin_version: *const c_char,
    kind: u32,
    input_layout: u32,
    output_layout: u32,
    capabilities: u32,
    params: *const c_void,
    param_count: u32,
    max_polyphony: u32,
}

#[path = "../../../graph/src/abi_c/runtime.rs"]
mod runtime;
use runtime::*;

struct Voice(f32);
unsafe extern "C" fn create(_: *const OxiHostContextV1) -> *mut c_void {
    Box::into_raw(Box::new(Voice(0.0))).cast()
}
unsafe extern "C" fn dispose(ptr: *mut c_void) {
    drop(unsafe { Box::from_raw(ptr.cast::<Voice>()) });
}
unsafe extern "C" fn prepare(_: *mut c_void, _: f64, _: u32) -> u32 { 0 }
unsafe extern "C" fn reset(ptr: *mut c_void) {
    unsafe { (*ptr.cast::<Voice>()).0 = 0.0; }
}
unsafe extern "C" fn tail(ptr: *const c_void) -> u64 {
    if unsafe { (*ptr.cast::<Voice>()).0 } > 0.0 { 17 } else { 0 }
}
unsafe extern "C" fn latency(_: *const c_void) -> u64 { 0 }
unsafe extern "C" fn process(ptr: *mut c_void, ctx: *const OxiProcessContextV1) -> u32 {
    let voice = unsafe { &mut *ptr.cast::<Voice>() };
    let ctx = unsafe { &*ctx };
    let mut n = 0;
    for f in 0..ctx.frames {
        while n < ctx.note_count {
            let note = unsafe { &*ctx.notes.add(n as usize) };
            if note.frame_offset != f { break; }
            voice.0 = if note.kind == 0 { note.velocity * (note.pitch as f32 / 64.0) } else { 0.0 };
            n += 1;
        }
        for c in 0..ctx.output_count {
            unsafe { *(*ctx.outputs.add(c as usize)).add(f as usize) = voice.0; }
        }
    }
    0
}

struct Shared<T>(T);
unsafe impl<T> Sync for Shared<T> {}
static DESCRIPTOR: Shared<OxiPluginDescriptorV1> = Shared(OxiPluginDescriptorV1 {
    abi_major: 1, abi_minor: 0,
    plugin_id: c"fixture.rust".as_ptr(), plugin_version: c"1.0.0".as_ptr(),
    kind: 0, input_layout: 0, output_layout: 2, capabilities: 2,
    params: std::ptr::null(), param_count: 0, max_polyphony: 1,
});
static ENTRY: Shared<OxiPluginEntryV1> = Shared(OxiPluginEntryV1 {
    abi_major: 1, abi_minor: 0, struct_size: std::mem::size_of::<OxiPluginEntryV1>() as u32,
    descriptor: &DESCRIPTOR.0, min_host_version: c"0.1.0".as_ptr(),
    create: Some(create), dispose: Some(dispose), prepare: Some(prepare),
    process: Some(process), reset: Some(reset), tail_frames: Some(tail), latency_frames: Some(latency),
});

#[no_mangle]
pub extern "C" fn oxitone_plugin_entry_v1() -> *const OxiPluginEntryV1 { &ENTRY.0 }
