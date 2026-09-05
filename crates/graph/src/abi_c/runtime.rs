//! Executable ABI v1 records. See include/oxitone_plugin.h for C declarations.
use super::OxiPluginDescriptorV1;
use crate::NoteEvent;
use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OxiHostContextV1 {
    pub sample_rate: f64,
    pub max_block_size: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OxiParameterEventV1 {
    pub frame_offset: u32,
    pub parameter_index: u32,
    pub value: f64,
}

#[repr(C)]
pub struct OxiProcessContextV1 {
    pub frames: u32,
    pub sample_rate: f64,
    pub inputs: *const *const f32,
    pub input_count: u32,
    pub outputs: *const *mut f32,
    pub output_count: u32,
    pub notes: *const NoteEvent,
    pub note_count: u32,
    pub parameters: *const OxiParameterEventV1,
    pub parameter_count: u32,
    pub sidechain: *const *const f32,
    pub sidechain_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OxiPluginEntryV1 {
    pub abi_major: u32,
    pub abi_minor: u32,
    pub struct_size: u32,
    pub descriptor: *const OxiPluginDescriptorV1,
    pub min_host_version: *const c_char,
    pub create: Option<unsafe extern "C" fn(*const OxiHostContextV1) -> *mut c_void>,
    pub dispose: Option<unsafe extern "C" fn(*mut c_void)>,
    pub prepare: Option<unsafe extern "C" fn(*mut c_void, f64, u32) -> u32>,
    pub process: Option<unsafe extern "C" fn(*mut c_void, *const OxiProcessContextV1) -> u32>,
    pub reset: Option<unsafe extern "C" fn(*mut c_void)>,
    pub tail_frames: Option<unsafe extern "C" fn(*const c_void) -> u64>,
    pub latency_frames: Option<unsafe extern "C" fn(*const c_void) -> u64>,
}
