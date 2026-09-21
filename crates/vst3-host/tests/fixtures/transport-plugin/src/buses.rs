//! Real VST3 sidechain and asymmetric bus fixtures; inactive slots deliberately sit between active ones.
use super::*;
pub(super) const SIDECHAIN_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318797E);
pub(super) const MULTIBUS_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318797F);

pub(super) struct Buses {
    outputs: usize,
    mono_main: bool,
    instrument: Option<super::instrument::Instrument>,
    inputs_active: [Cell<bool>; 3],
    outputs_active: [Cell<bool>; 3],
}
impl Buses {
    pub fn midi_only(note_input: bool) -> Self {
        Self {
            instrument: note_input.then(super::instrument::Instrument::new),
            ..Self::new(0)
        }
    }
    pub fn output_active(&self, index: usize) -> bool {
        self.outputs_active[index].get()
    }
    pub fn instrument(outputs: usize) -> Self {
        Self {
            instrument: Some(super::instrument::Instrument::new()),
            ..Self::new(outputs)
        }
    }
    pub fn reset(&self) {
        if let Some(instrument) = &self.instrument {
            instrument.reset();
        }
    }
    pub fn new(outputs: usize) -> Self {
        Self {
            outputs,
            mono_main: false,
            instrument: None,
            inputs_active: [Cell::new(true), Cell::new(false), Cell::new(true)],
            outputs_active: [Cell::new(true), Cell::new(false), Cell::new(true)],
        }
    }
    pub fn mono_main() -> Self {
        Self {
            mono_main: true,
            ..Self::new(1)
        }
    }
    pub fn count(&self, media: MediaType, dir: BusDirection) -> i32 {
        if self.outputs == 0 {
            return (self.instrument.is_some()
                && media == MediaTypes_::kEvent as i32
                && dir == BusDirections_::kInput as i32) as i32;
        }
        if self.instrument.is_some() && dir == BusDirections_::kInput as i32 {
            return (media == MediaTypes_::kEvent as i32) as i32;
        }
        if media != MediaTypes_::kAudio as i32 {
            return 0;
        }
        if dir == BusDirections_::kInput as i32 {
            if self.outputs == 1 {
                2
            } else {
                3
            }
        } else if dir == BusDirections_::kOutput as i32 {
            self.outputs as i32
        } else {
            0
        }
    }
    pub unsafe fn info(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if index < 0 || index >= self.count(media, dir) {
            return kInvalidArgument;
        }
        let bus = &mut *bus;
        bus.mediaType = media;
        bus.direction = dir;
        bus.channelCount = if media == MediaTypes_::kEvent as i32 {
            16
        } else if index == 1 || self.mono_main {
            1
        } else {
            2
        };
        bus.busType = if index == 0 {
            BusTypes_::kMain
        } else {
            BusTypes_::kAux
        } as i32;
        bus.flags = if index == 1 {
            0
        } else {
            BusInfo_::BusFlags_::kDefaultActive as u32
        };
        copy_wstring(if index == 0 { "Main" } else { "Auxiliary" }, &mut bus.name);
        kResultOk
    }
    pub fn activate(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        state: TBool,
    ) -> tresult {
        if index < 0 || index >= self.count(media, dir) {
            return kInvalidArgument;
        }
        if media == MediaTypes_::kEvent as i32 {
            return kResultOk;
        }
        let states = if dir == BusDirections_::kInput as i32 {
            &self.inputs_active
        } else {
            &self.outputs_active
        };
        states[index as usize].set(state != 0);
        kResultOk
    }
    pub unsafe fn arrangement(
        &self,
        dir: BusDirection,
        index: i32,
        result: *mut SpeakerArrangement,
    ) -> tresult {
        if index < 0 || index >= self.count(MediaTypes_::kAudio as i32, dir) {
            return kInvalidArgument;
        }
        *result = if index == 1 || self.mono_main {
            SpeakerArr::kMono
        } else {
            SpeakerArr::kStereo
        };
        kResultOk
    }
    pub unsafe fn arrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        ni: i32,
        outputs: *mut SpeakerArrangement,
        no: i32,
    ) -> tresult {
        if ni != self.count(MediaTypes_::kAudio as i32, BusDirections_::kInput as i32)
            || no != self.outputs as i32
        {
            return kResultFalse;
        }
        for (pointer, count) in [(inputs, ni), (outputs, no)] {
            for i in 0..count {
                if *pointer.add(i as usize)
                    != if i == 1 || self.mono_main {
                        SpeakerArr::kMono
                    } else {
                        SpeakerArr::kStereo
                    }
                {
                    return kResultFalse;
                }
            }
        }
        kResultTrue
    }
    pub unsafe fn process(&self, data: &ProcessData) -> tresult {
        if data.numSamples == 0 {
            return kResultOk;
        }
        if data.numInputs != self.count(MediaTypes_::kAudio as i32, BusDirections_::kInput as i32)
            || data.numOutputs != self.outputs as i32
        {
            return kInvalidArgument;
        }
        if self.outputs == 0 {
            return kResultOk;
        }
        if let Some(instrument) = &self.instrument {
            return instrument.process(data, &self.outputs_active);
        }
        let inputs = slice::from_raw_parts(data.inputs, data.numInputs as usize);
        let outputs = slice::from_raw_parts_mut(data.outputs, data.numOutputs as usize);
        let sample = |bus: usize, ch: usize, frame: usize| {
            if !self.inputs_active[bus].get() {
                return 0.;
            }
            let input = &inputs[bus];
            *(*input
                .__field0
                .channelBuffers32
                .add(ch.min(input.numChannels as usize - 1)))
            .add(frame)
        };
        for (index, bus) in outputs.iter_mut().enumerate() {
            if !self.outputs_active[index].get() {
                continue;
            }
            for ch in 0..bus.numChannels as usize {
                let target = slice::from_raw_parts_mut(
                    *bus.__field0.channelBuffers32.add(ch),
                    data.numSamples as usize,
                );
                for (frame, value) in target.iter_mut().enumerate() {
                    *value = if self.outputs == 1 {
                        sample(0, ch, frame) * sample(1, 0, frame)
                    } else {
                        match index {
                            0 => sample(0, ch, frame) + sample(2, ch, frame) * 0.125,
                            1 => sample(1, 0, frame) * 0.5,
                            _ => sample(2, ch, frame) * -0.25,
                        }
                    };
                }
            }
        }
        kResultOk
    }
}
