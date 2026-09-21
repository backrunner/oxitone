use super::*;
impl GainProcessor {
    pub(super) unsafe fn process_audio(&self, data: *mut ProcessData) -> tresult {
        if let Some(b) = &self.buses {
            return b.process(&*data);
        }
        let process_data = &*data;

        if process_data.numSamples == 0 {
            return kResultOk;
        }

        if process_data.processContext.is_null() || process_data.numOutputs != 1 {
            return kInvalidArgument;
        }
        let context = &*process_data.processContext;
        let frames = process_data.numSamples as usize;
        let outputs = &*process_data.outputs;
        if outputs.numChannels != 2 {
            return kInvalidArgument;
        }
        let channels = slice::from_raw_parts(outputs.__field0.channelBuffers32, 2);
        let left = slice::from_raw_parts_mut(channels[0], frames);
        let right = slice::from_raw_parts_mut(channels[1], frames);
        let start = self.processed.fetch_add(frames as u64, Ordering::Relaxed);
        let fields = [
            context.projectTimeSamples as f32,
            context.continousTimeSamples as f32,
            context.projectTimeMusic as f32,
            context.barPositionMusic as f32,
            context.tempo as f32,
            context.timeSigNumerator as f32,
            context.timeSigDenominator as f32,
            (context.state & ProcessContext_::StatesAndFlags_::kPlaying as u32 != 0) as u8 as f32,
            context.cycleStartMusic as f32,
            context.cycleEndMusic as f32,
            context.state as f32,
        ];
        for i in 0..frames {
            left[i] = (start + i as u64) as f32 / 1_000_000.0;
            right[i] = fields[i % fields.len()];
        }

        kResultOk
    }
}
