//! Live restart fixture. Requests a topology change from process(), after writing old-layout PCM.
use super::*;
impl Dynamic {
    pub(super) unsafe fn process_live(&self, data: &ProcessData) -> tresult {
        if !self.active.get()
            || !self.processing.get()
            || !self.configured.get()
            || data.numSamples < 1
            || data.numOutputs < 1
            || data.outputs.is_null()
        {
            return kResultFalse;
        }
        let before = self.mode.get();
        // Independent process-originated notification, without a preceding controller mutation.
        let mut mode = (!data.processContext.is_null()
            && (*data.processContext).projectTimeSamples == 96_001
            && before == 0)
            .then_some(1);
        if let Some(changes) = ComRef::from_raw(data.inputParameterChanges) {
            for i in 0..changes.getParameterCount() {
                let Some(queue) = ComRef::from_raw(changes.getParameterData(i)) else {
                    return kResultFalse;
                };
                for j in 0..queue.getPointCount() {
                    let (mut frame, mut value) = (0, 0.);
                    if queue.getPoint(j, &mut frame, &mut value) != kResultOk {
                        return kResultFalse;
                    }
                    match queue.getParameterId() {
                        0 => mode = Some((value * 3.).round() as u8),
                        7 if before == 1 => {
                            self.gain.set(value);
                            self.dsp_gain.set(value);
                        }
                        _ => (),
                    }
                }
            }
        }
        if self.graph {
            self.process_graph(data);
        } else {
            for bus in 0..data.numOutputs as usize {
                if !self.buses().output_active(bus) {
                    continue;
                }
                let output = &mut *data.outputs.add(bus);
                for channel in 0..output.numChannels as usize {
                    let target = *output.__field0.channelBuffers32.add(channel);
                    for frame in 0..data.numSamples as usize {
                        *target.add(frame) = self.dsp_gain.get() as f32;
                    }
                }
                output.silenceFlags = 0;
            }
        }
        if let Some(mode) = mode {
            self.set_mode(mode);
        }
        kResultOk
    }

    unsafe fn process_graph(&self, data: &ProcessData) {
        let mut delay = self.delay.borrow_mut();
        let mut cursor = self.cursor.get();
        let input = &*data.inputs;
        let output = &mut *data.outputs;
        for frame in 0..data.numSamples as usize {
            for ch in 0..output.numChannels as usize {
                let value = *(*input.__field0.channelBuffers32.add(ch)).add(frame)
                    * self.dsp_gain.get() as f32;
                let target = (*output.__field0.channelBuffers32.add(ch)).add(frame);
                *target = if self.mode.get() == 1 {
                    delay[ch][cursor]
                } else {
                    value
                };
                delay[ch][cursor] = value;
            }
            cursor = (cursor + 1) % 64;
        }
        self.cursor.set(cursor);
        output.silenceFlags = 0;
    }
}
