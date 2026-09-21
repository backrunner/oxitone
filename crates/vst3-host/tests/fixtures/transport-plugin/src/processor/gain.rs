//! Sample-offset gain is an independent observation of the host's actual parameter delivery.
use super::*;
pub(super) struct Gain(Cell<f64>);
pub(super) const CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187983);
impl GainProcessor {
    pub(crate) const RECORDING_CID: TUID = CID;
    pub(crate) fn recording() -> Self {
        Self {
            gain: Some(Gain(Cell::new(1.))),
            ..Self::new()
        }
    }
}
impl Gain {
    pub unsafe fn process(&self, data: &ProcessData) -> tresult {
        let changes = vst3::ComRef::from_raw(data.inputParameterChanges);
        let mut queue = None;
        if let Some(changes) = &changes {
            for index in 0..changes.getParameterCount() {
                if let Some(candidate) = vst3::ComRef::from_raw(changes.getParameterData(index)) {
                    if candidate.getParameterId() == 0 {
                        queue = Some(candidate);
                    }
                }
            }
        }
        let count = queue.as_ref().map_or(0, |queue| queue.getPointCount());
        let mut index = 0;
        let mut offset = 0;
        let mut next = 0.;
        let mut point =
            count > 0 && queue.as_ref().unwrap().getPoint(0, &mut offset, &mut next) == kResultOk;
        if data.numSamples == 0 {
            if count > 0
                && queue
                    .as_ref()
                    .unwrap()
                    .getPoint(count - 1, &mut offset, &mut next)
                    == kResultOk
            {
                self.0.set(next);
            }
            return kResultOk;
        }
        if data.numInputs != 1
            || data.numOutputs != 1
            || (*data.inputs).numChannels != 2
            || (*data.outputs).numChannels != 2
        {
            return kInvalidArgument;
        }
        for frame in 0..data.numSamples {
            while point && offset <= frame {
                self.0.set(next);
                index += 1;
                point = index < count
                    && queue
                        .as_ref()
                        .unwrap()
                        .getPoint(index, &mut offset, &mut next)
                        == kResultOk;
            }
            for channel in 0..2 {
                *(*(*data.outputs).__field0.channelBuffers32.add(channel)).add(frame as usize) =
                    *(*(*data.inputs).__field0.channelBuffers32.add(channel)).add(frame as usize)
                        * self.0.get() as f32;
            }
        }
        kResultOk
    }
}
