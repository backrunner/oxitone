//! Real processor event output: transposed MIDI thru, generated insert notes and fault injection.
use super::*;
use vst3::ComRef;

pub(super) const CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187986);
pub(super) const FX_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187987);
pub(super) const OVERFLOW_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187988);
pub(super) const INVALID_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187989);
pub(super) const ONLY_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318798A);
pub(super) const GENERATOR_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318798B);

impl GainProcessor {
    pub(super) fn midi(mode: u8) -> Self {
        let mut processor = if mode == 2 {
            Self::new()
        } else {
            Self::instrument()
        };
        processor.midi = mode;
        if mode >= 5 {
            processor.buses = Some(super::buses::Buses::midi_only(mode == 5 || mode == 7));
        }
        processor
    }
    pub(super) unsafe fn emit_midi(&self, data: &ProcessData) {
        if data.numSamples <= 0 {
            return;
        }
        if self.midi >= 7 {
            return super::sysex::emit(data, self.midi);
        }
        let Some(output) = ComRef::from_raw(data.outputEvents) else {
            return;
        };
        if self.midi == 3 || self.midi == 4 {
            // Ignore addEvent failures deliberately: the host must detect rejected events itself.
            for _ in 0..if self.midi == 3 { 4200 } else { 1 } {
                let mut event: Event = std::mem::zeroed();
                event.r#type = Event_::EventTypes_::kNoteOffEvent as u16;
                event.sampleOffset = if self.midi == 4 { data.numSamples } else { 0 };
                event.__field0.noteOff.pitch = 60;
                output.addEvent(&mut event);
            }
        } else if self.midi == 2 || self.midi == 6 {
            let start = if data.processContext.is_null() {
                0
            } else {
                (*data.processContext).projectTimeSamples
            };
            for i in 0..data.numSamples {
                let phase = (start + i as i64).rem_euclid(256);
                if phase != 13 && phase != 109 {
                    continue;
                }
                let mut event: Event = std::mem::zeroed();
                event.sampleOffset = i;
                if phase == 13 {
                    event.r#type = Event_::EventTypes_::kNoteOnEvent as u16;
                    event.__field0.noteOn.pitch = 72;
                    event.__field0.noteOn.channel = 15;
                    event.__field0.noteOn.velocity = 64. / 127.;
                    event.__field0.noteOn.noteId = -1;
                } else {
                    event.r#type = Event_::EventTypes_::kNoteOffEvent as u16;
                    event.__field0.noteOff.pitch = 72;
                    event.__field0.noteOff.channel = 15;
                    event.__field0.noteOff.noteId = -1;
                }
                output.addEvent(&mut event);
            }
        } else if let Some(input) = ComRef::from_raw(data.inputEvents) {
            for i in 0..input.getEventCount() {
                let mut event: Event = std::mem::zeroed();
                if input.getEvent(i, &mut event) != kResultOk {
                    continue;
                }
                match event.r#type as u32 {
                    Event_::EventTypes_::kNoteOnEvent => {
                        event.__field0.noteOn.pitch = (event.__field0.noteOn.pitch + 12).min(127);
                        event.__field0.noteOn.channel = 15;
                    }
                    Event_::EventTypes_::kNoteOffEvent => {
                        event.__field0.noteOff.pitch = (event.__field0.noteOff.pitch + 12).min(127);
                        event.__field0.noteOff.channel = 15;
                    }
                    _ => {}
                }
                output.addEvent(&mut event);
            }
        }
    }
}
