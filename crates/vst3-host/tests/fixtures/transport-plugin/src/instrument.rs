//! A note-gated three-output instrument: each bus carries the same signal at a distinct gain.
use super::*;
use vst3::ComRef;

pub(super) const CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187980);
pub(super) struct Instrument {
    level: Cell<f32>,
    phase: Cell<u32>,
}
impl Instrument {
    pub fn new() -> Self {
        Self {
            level: Cell::new(0.),
            phase: Cell::new(0),
        }
    }
    pub fn reset(&self) {
        self.level.set(0.);
        self.phase.set(0);
    }
    pub unsafe fn process(&self, data: &ProcessData, active: &[Cell<bool>; 3]) -> tresult {
        if data.numSamples < 0 || data.numSamples > 4096 {
            return kInvalidArgument;
        }
        let events = ComRef::from_raw(data.inputEvents);
        let count = events.as_ref().map_or(0, |list| list.getEventCount());
        let mut cursor = 0;
        let mut next: Event = std::mem::zeroed();
        let mut pending = count > 0 && events.as_ref().unwrap().getEvent(0, &mut next) == kResultOk;
        let mut source = [0.; 4096];
        let mut phase = self.phase.get();
        let mut level = self.level.get();
        for (frame, sample) in source[..data.numSamples as usize].iter_mut().enumerate() {
            while pending && next.sampleOffset <= frame as i32 {
                match next.r#type as u32 {
                    Event_::EventTypes_::kNoteOnEvent => {
                        level = next.__field0.noteOn.velocity * 0.05
                    }
                    Event_::EventTypes_::kNoteOffEvent => level = 0.,
                    Event_::EventTypes_::kDataEvent => {
                        if let Some(value) = super::sysex::level(&next) {
                            level = value;
                        }
                    }
                    _ => {}
                }
                cursor += 1;
                pending = cursor < count
                    && events.as_ref().unwrap().getEvent(cursor, &mut next) == kResultOk;
            }
            *sample = if phase % 100 < 50 { level } else { -level };
            phase = phase.wrapping_add(1);
        }
        self.phase.set(phase);
        self.level.set(level);
        for (index, bus) in slice::from_raw_parts_mut(data.outputs, data.numOutputs as usize)
            .iter_mut()
            .enumerate()
        {
            if !active[index].get() {
                continue;
            }
            for ch in 0..bus.numChannels as usize {
                let target = slice::from_raw_parts_mut(
                    *bus.__field0.channelBuffers32.add(ch),
                    data.numSamples as usize,
                );
                for (output, sample) in target.iter_mut().zip(source.iter()) {
                    *output = *sample * (index + 1) as f32;
                }
            }
        }
        kResultOk
    }
}
