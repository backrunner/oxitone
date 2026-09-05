use super::{fault, LoadedPlugin};
use oxitone_core::OxitoneError;
use oxitone_graph::abi_c::{OxiHostContextV1, OxiParameterEventV1, OxiProcessContextV1};
use oxitone_graph::{HostContext, PluginInstance, ProcessContext};
use std::ffi::c_void;
use std::sync::{atomic::Ordering, Arc};

pub(super) struct CInstance {
    shared: Arc<LoadedPlugin>,
    handle: *mut c_void,
    mono: Vec<f32>,
    max_block: usize,
    latency: u64,
    faulted: bool,
}
// SAFETY: ownership is exclusive and ABI instances may move between threads.
unsafe impl Send for CInstance {}

impl CInstance {
    pub(super) fn new(shared: Arc<LoadedPlugin>, host: &HostContext) -> Result<Self, OxitoneError> {
        let context = OxiHostContextV1 {
            sample_rate: host.sample_rate,
            max_block_size: host.max_block_size,
        };
        // SAFETY: validated factory, lifetime retained by shared.
        let handle = unsafe { shared.entry.create.unwrap()(&context) };
        if handle.is_null() {
            return Err(fault("plugin create returned null"));
        }
        Ok(Self {
            shared,
            handle,
            mono: Vec::new(),
            max_block: 0,
            latency: 0,
            faulted: false,
        })
    }
    fn mute(&mut self, ctx: &mut ProcessContext<'_>) {
        if !self.faulted {
            self.shared.faults.fetch_add(1, Ordering::Relaxed);
        }
        self.faulted = true;
        for output in ctx.outputs.iter_mut() {
            let n = ctx.frames.min(output.len());
            output[..n].fill(0.0);
        }
    }
}

impl PluginInstance for CInstance {
    fn prepare(&mut self, rate: f64, block: u32) {
        let _ = self.try_prepare(rate, block);
    }
    fn try_prepare(&mut self, rate: f64, block: u32) -> Result<(), OxitoneError> {
        self.max_block = 0;
        if !rate.is_finite() || !(1.0..=768000.0).contains(&rate) || block == 0 || block > 65536 {
            return Err(fault("invalid plugin prepare configuration"));
        }
        // SAFETY: exclusive instance, called outside realtime processing.
        let status = unsafe { self.shared.entry.prepare.unwrap()(self.handle, rate, block) };
        if status != 0 {
            return Err(fault("plugin prepare failed"));
        }
        self.latency = unsafe { self.shared.entry.latency_frames.unwrap()(self.handle) };
        if self.latency > rate as u64 * 10 {
            return Err(fault("plugin latency exceeds ten-second budget"));
        }
        self.max_block = block as usize;
        self.mono.resize(self.max_block, 0.0);
        self.faulted = false;
        Ok(())
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        if self.faulted
            || ctx.frames == 0
            || ctx.frames > self.max_block
            || ctx.outputs.iter().any(|o| o.len() < ctx.frames)
            || ctx.inputs.iter().any(|i| i.len() < ctx.frames)
            || ctx
                .sidechain
                .is_some_and(|s| s.iter().any(|i| i.len() < ctx.frames))
            || ctx.note_events.len() > u32::MAX as usize
            || ctx.note_events.iter().any(|e| {
                e.frame_offset as usize >= ctx.frames
                    || !e.velocity.is_finite()
                    || !(0.0..=1.0).contains(&e.velocity)
                    || e.pitch > 127
            })
            || ctx
                .note_events
                .windows(2)
                .any(|e| e[0].frame_offset > e[1].frame_offset)
            || ctx
                .parameter_events
                .windows(2)
                .any(|e| e[0].frame_offset > e[1].frame_offset)
            || ctx.outputs.len() < 2
            || ctx.parameter_events.len() > 256
        {
            self.mute(ctx);
            return;
        }
        let descriptor = &self.shared.descriptor;
        let input_count = descriptor.input_layout.bus_count();
        let output_count = descriptor.output_layout.bus_count();
        let mut inputs = [std::ptr::null(); 2];
        if input_count > 0 {
            if ctx.inputs.len() < 2 {
                self.mute(ctx);
                return;
            }
            if input_count == 1 {
                for n in 0..ctx.frames {
                    self.mono[n] = (ctx.inputs[0][n] + ctx.inputs[1][n]) * 0.5;
                }
                inputs[0] = self.mono.as_ptr();
            } else {
                inputs = [ctx.inputs[0].as_ptr(), ctx.inputs[1].as_ptr()];
            }
        }
        let outputs = [ctx.outputs[0].as_mut_ptr(), ctx.outputs[1].as_mut_ptr()];
        for output in ctx.outputs.iter_mut() {
            output[..ctx.frames].fill(0.0);
        }
        let mut parameters = [OxiParameterEventV1 {
            frame_offset: 0,
            parameter_index: 0,
            value: 0.0,
        }; 256];
        for (out, event) in parameters.iter_mut().zip(ctx.parameter_events) {
            let Some(index) = descriptor
                .parameters
                .iter()
                .position(|p| p.id == event.parameter_id)
            else {
                self.mute(ctx);
                return;
            };
            let spec = &descriptor.parameters[index];
            if event.frame_offset as usize >= ctx.frames
                || !event.value.is_finite()
                || !(spec.min..=spec.max).contains(&event.value)
            {
                self.mute(ctx);
                return;
            }
            *out = OxiParameterEventV1 {
                frame_offset: event.frame_offset,
                parameter_index: index as u32,
                value: event.value,
            };
        }
        let sidechain = ctx
            .sidechain
            .filter(|s| descriptor.capabilities.sidechain_input && s.len() >= 2);
        let side_ptrs = sidechain
            .map(|s| [s[0].as_ptr(), s[1].as_ptr()])
            .unwrap_or([std::ptr::null(); 2]);
        let context = OxiProcessContextV1 {
            frames: ctx.frames as u32,
            sample_rate: ctx.sample_rate,
            inputs: inputs.as_ptr(),
            input_count: input_count as u32,
            outputs: outputs.as_ptr(),
            output_count: output_count as u32,
            notes: ctx.note_events.as_ptr(),
            note_count: ctx.note_events.len() as u32,
            parameters: parameters.as_ptr(),
            parameter_count: ctx.parameter_events.len() as u32,
            sidechain: side_ptrs.as_ptr(),
            sidechain_count: if sidechain.is_some() { 2 } else { 0 },
        };
        // SAFETY: pointers borrow live host buffers for this call only; the
        // exclusive instance and library remain owned by self.
        let status = unsafe { self.shared.entry.process.unwrap()(self.handle, &context) };
        let latency = unsafe { self.shared.entry.latency_frames.unwrap()(self.handle) };
        if status != 0
            || latency != self.latency
            || ctx
                .outputs
                .iter()
                .any(|o| o[..ctx.frames].iter().any(|v| !v.is_finite()))
        {
            self.mute(ctx);
            return;
        }
        if output_count == 1 {
            let (left, right) = ctx.outputs.split_at_mut(1);
            right[0][..ctx.frames].copy_from_slice(&left[0][..ctx.frames]);
        }
    }
    fn reset(&mut self) {
        // ABI reset is a realtime-safe flush of preallocated state.
        unsafe { self.shared.entry.reset.unwrap()(self.handle) };
        self.faulted = false;
    }
    fn tail_frames(&self) -> u64 {
        if self.shared.descriptor.capabilities.reports_tail && !self.faulted {
            unsafe { self.shared.entry.tail_frames.unwrap()(self.handle) }
        } else {
            0
        }
    }
    fn latency_frames(&self) -> u64 {
        self.latency
    }
}
impl Drop for CInstance {
    fn drop(&mut self) {
        // Graph retirement must happen on a control thread, before releasing
        // the last owner of the library that contains dispose itself.
        unsafe { self.shared.entry.dispose.unwrap()(self.handle) };
    }
}
