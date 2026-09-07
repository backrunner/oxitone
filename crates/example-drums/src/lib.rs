//! Self-contained drum-machine example exported through the public C ABI.
mod metadata;
#[cfg(test)]
mod tests;
mod voice;

use oxitone_graph::{abi_c::*, NoteEventKind};
use std::ffi::c_void;
use voice::Voice;

struct Machine {
    voices: [Voice; 4],
    rate: f64,
    level: f64,
    decay: f64,
    noise: u32,
}

impl Machine {
    fn reset(&mut self) {
        self.voices.fill(Voice::default());
        self.noise = 0x6d2b79f5;
    }

    fn trigger(&mut self, pitch: u8, velocity: f32) {
        let pad = match pitch {
            36 => 0,
            38 => 1,
            42 => {
                self.voices[3].remaining = 0;
                2
            }
            46 => 3,
            _ => return,
        };
        self.voices[pad].trigger(pad, velocity, self.rate, self.decay);
    }

    fn tick(&mut self) -> [f32; 2] {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = f64::from(self.noise) / f64::from(u32::MAX) * 2.0 - 1.0;
        let mut output = [0.0; 2];
        for (pad, voice) in self.voices.iter_mut().enumerate() {
            let value = voice.tick(pad, self.rate, noise) * self.level;
            let pan = [0.0, -0.08, 0.28, 0.35][pad];
            output[0] += (value * (1.0 - pan) * 0.5) as f32;
            output[1] += (value * (1.0 + pan) * 0.5) as f32;
        }
        output
    }
}

unsafe extern "C" fn create(_: *const OxiHostContextV1) -> *mut c_void {
    let mut machine = Machine {
        voices: [Voice::default(); 4],
        rate: 48000.0,
        level: 0.8,
        decay: 1.0,
        noise: 0,
    };
    machine.reset();
    Box::into_raw(Box::new(machine)).cast()
}

unsafe extern "C" fn dispose(ptr: *mut c_void) {
    // SAFETY: host passes the unique instance returned by create, exactly once.
    drop(unsafe { Box::from_raw(ptr.cast::<Machine>()) });
}

unsafe extern "C" fn prepare(ptr: *mut c_void, rate: f64, block: u32) -> u32 {
    if !rate.is_finite() || !(8000.0..=768000.0).contains(&rate) || block == 0 || block > 65536 {
        return 1;
    }
    let machine = unsafe { &mut *ptr.cast::<Machine>() };
    machine.rate = rate;
    machine.reset();
    0
}

unsafe extern "C" fn reset(ptr: *mut c_void) {
    unsafe { &mut *ptr.cast::<Machine>() }.reset();
}

unsafe extern "C" fn tail(ptr: *const c_void) -> u64 {
    unsafe { &*ptr.cast::<Machine>() }
        .voices
        .iter()
        .map(|v| v.remaining)
        .max()
        .unwrap_or(0)
}

unsafe extern "C" fn latency(_: *const c_void) -> u64 {
    0
}

unsafe extern "C" fn process(ptr: *mut c_void, context: *const OxiProcessContextV1) -> u32 {
    // SAFETY: the ABI host serializes access and validates borrowed buffer/event tables.
    let machine = unsafe { &mut *ptr.cast::<Machine>() };
    let ctx = unsafe { &*context };
    if ctx.output_count != 2 || ctx.input_count != 0 {
        return 1;
    }
    let mut parameter = 0;
    let mut note = 0;
    for frame in 0..ctx.frames {
        // One-shots ignore note-off; parameters must precede note-on at this frame.
        while parameter < ctx.parameter_count {
            let event = unsafe { &*ctx.parameters.add(parameter as usize) };
            if event.frame_offset != frame {
                break;
            }
            match event.parameter_index {
                0 => machine.level = event.value,
                1 => machine.decay = event.value,
                _ => return 1,
            }
            parameter += 1;
        }
        while note < ctx.note_count {
            let event = unsafe { &*ctx.notes.add(note as usize) };
            if event.frame_offset != frame {
                break;
            }
            if event.kind == NoteEventKind::NoteOn {
                machine.trigger(event.pitch, event.velocity);
            }
            note += 1;
        }
        let output = machine.tick();
        for (channel, value) in output.into_iter().enumerate() {
            unsafe {
                *(*ctx.outputs.add(channel)).add(frame as usize) = value;
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn oxitone_plugin_entry_v1() -> *const OxiPluginEntryV1 {
    &metadata::ENTRY.0
}
