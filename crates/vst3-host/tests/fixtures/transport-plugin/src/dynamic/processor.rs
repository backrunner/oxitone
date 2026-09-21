use super::*;

impl IAudioProcessorTrait for Dynamic {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        ni: i32,
        outputs: *mut SpeakerArrangement,
        no: i32,
    ) -> tresult {
        if self.active.get() || self.processing.get() {
            return kResultFalse;
        }
        let result = self.buses().arrangements(inputs, ni, outputs, no);
        if result == kResultOk {
            self.configured.set(true);
        }
        result
    }
    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        self.buses().arrangement(dir, index, arr)
    }
    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == SymbolicSampleSizes_::kSample32 as i32 {
            kResultOk
        } else {
            kNotImplemented
        }
    }
    unsafe fn getLatencySamples(&self) -> u32 {
        if self.mode.get() == 1 {
            64
        } else {
            0
        }
    }
    unsafe fn setupProcessing(&self, _setup: *mut ProcessSetup) -> tresult {
        if self.active.get() || self.processing.get() {
            kResultFalse
        } else {
            kResultOk
        }
    }
    unsafe fn setProcessing(&self, state: TBool) -> tresult {
        if state == 0 {
            *self.delay.borrow_mut() = [[0.; 64]; 2];
            self.cursor.set(0);
        }
        self.processing.set(state != 0);
        kResultOk
    }
    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        if self.live && data.numSamples > 0 {
            return self.process_live(data);
        }
        // This fixture refuses nonzero processing, proving the configuration path never renders audio.
        if data.numSamples != 0
            || !self.active.get()
            || !self.processing.get()
            || !self.configured.get()
        {
            return kResultFalse;
        }
        if data.numInputs != 0
            || data.numOutputs != 0
            || !data.inputs.is_null()
            || !data.outputs.is_null()
        {
            return kResultFalse;
        }
        if let Some(changes) = ComRef::from_raw(data.inputParameterChanges) {
            for i in 0..changes.getParameterCount() {
                let Some(queue) = ComRef::from_raw(changes.getParameterData(i)) else {
                    return kResultFalse;
                };
                if queue.getParameterId() == if self.dense { 4095 } else { 7 } {
                    for j in 0..queue.getPointCount() {
                        let (mut frame, mut value) = (0, 0.);
                        if queue.getPoint(j, &mut frame, &mut value) == kResultOk {
                            self.dsp_gain.set(value);
                        }
                    }
                }
            }
        }
        if self.bulk_requested.replace(false) {
            self.gain.set(0.625);
            self.restart(RestartFlags_::kParamValuesChanged);
        }
        match self.mode.get() {
            2 => self.restart(RestartFlags_::kReloadComponent),
            3 => self.restart(RestartFlags_::kLatencyChanged),
            _ => (),
        }
        kResultOk
    }
    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}
