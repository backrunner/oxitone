//! Pointer-backed output is copied by the host before these stack buffers are overwritten.
use super::*;
use vst3::ComRef;
pub(super) const CLASSES: [TUID; 5] = [
    uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318798C),
    uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318798D),
    uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318798E),
    uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318798F),
    uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187990),
];
impl GainProcessor {
    pub(super) fn sysex_receiver() -> Self {
        let mut processor = Self::instrument();
        processor.buses = Some(super::buses::Buses::instrument(1));
        processor
    }
}

pub(super) unsafe fn emit(data: &ProcessData, mode: u8) {
    let Some(output) = ComRef::from_raw(data.outputEvents) else {
        return;
    };
    let send = |bytes: &mut [u8], offset: i32| {
        let mut event: Event = std::mem::zeroed();
        event.r#type = Event_::EventTypes_::kDataEvent as u16;
        event.sampleOffset = offset;
        event.__field0.data = DataEvent {
            size: bytes.len() as u32,
            r#type: 0,
            bytes: bytes.as_ptr(),
        };
        output.addEvent(&mut event);
        bytes.fill(0x55);
    };
    if mode == 7 {
        let Some(input) = ComRef::from_raw(data.inputEvents) else {
            return;
        };
        for i in 0..input.getEventCount() {
            let mut event: Event = std::mem::zeroed();
            if input.getEvent(i, &mut event) != kResultOk {
                continue;
            }
            let value = match event.r#type as u32 {
                Event_::EventTypes_::kNoteOnEvent => {
                    (event.__field0.noteOn.velocity * 127.).round() as u8
                }
                Event_::EventTypes_::kNoteOffEvent => 0,
                _ => continue,
            };
            send(&mut [0xf0, 0x7d, 1, value, 0xf7], event.sampleOffset);
        }
    } else if mode == 9 {
        send(&mut [0xf0, 0x80, 0xf7], 0);
    } else {
        for _ in 0..if mode == 8 { 5 } else { 1 } {
            let mut bytes = [0x7d; 4097];
            let size = if mode == 8 { 4096 } else { 4097 };
            bytes[0] = 0xf0;
            bytes[size - 1] = 0xf7;
            send(&mut bytes[..size], 0);
        }
    }
}

pub(super) unsafe fn level(event: &Event) -> Option<f32> {
    let data = event.__field0.data;
    if data.r#type != 0 || data.size != 5 || data.bytes.is_null() {
        return None;
    }
    let bytes = slice::from_raw_parts(data.bytes, 5);
    if bytes[..3] != [0xf0, 0x7d, 1] || bytes[4] != 0xf7 || bytes[3] > 127 {
        return None;
    }
    Some(bytes[3] as f32 / 127. * 0.05)
}
