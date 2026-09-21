//! Prepared stereo bus routing buffers and delays. No allocation during staging/capture.
use crate::DelayLine;
use oxitone_core::OxitoneError;
use oxitone_graph::PluginInstance;

pub(super) fn delay(frames: usize, block: usize) -> DelayLine {
    let mut line = DelayLine::new(frames, block);
    line.set_delay(frames);
    line
}

pub(super) struct InputRoute {
    pub bus_index: usize,
    pub incoming: [Vec<f32>; 2],
    pub delay: DelayLine,
}
impl InputRoute {
    pub fn new(bus_index: usize, prefix: usize, block: usize) -> Self {
        Self {
            bus_index,
            incoming: [vec![0.; block], vec![0.; block]],
            delay: delay(prefix, block),
        }
    }
    pub fn stage(
        &mut self,
        plugin: &mut dyn PluginInstance,
        frames: usize,
    ) -> Result<(), OxitoneError> {
        let [left, right] = plugin.input_bus_mut(self.bus_index).ok_or_else(|| {
            OxitoneError::new("RealtimeFault", "insert auxiliary input is unavailable")
        })?;
        if left.len() < frames || right.len() < frames {
            return Err(OxitoneError::new(
                "RealtimeFault",
                "insert auxiliary input is too short",
            ));
        }
        if self.delay.delay_frames() == 0 {
            left[..frames].copy_from_slice(&self.incoming[0][..frames]);
            right[..frames].copy_from_slice(&self.incoming[1][..frames]);
            return Ok(());
        }
        left[..frames].fill(0.);
        right[..frames].fill(0.);
        self.delay.process_add(
            &self.incoming[0][..frames],
            &self.incoming[1][..frames],
            1.,
            &mut left[..frames],
            &mut right[..frames],
        );
        Ok(())
    }
}

pub(super) struct SidechainInput {
    pub aligned: [Vec<f32>; 2],
    pub delay: DelayLine,
}
impl SidechainInput {
    pub fn new(prefix: usize, block: usize) -> Self {
        Self {
            aligned: [vec![0.; block], vec![0.; block]],
            delay: delay(prefix, block),
        }
    }
    pub fn stage(&mut self, left: &[f32], right: &[f32]) {
        let frames = left.len();
        let [out_l, out_r] = &mut self.aligned;
        if self.delay.delay_frames() == 0 {
            out_l[..frames].copy_from_slice(left);
            out_r[..frames].copy_from_slice(right);
            return;
        }
        out_l[..frames].fill(0.);
        out_r[..frames].fill(0.);
        self.delay
            .process_add(left, right, 1., &mut out_l[..frames], &mut out_r[..frames]);
    }
}

pub(super) struct OutputRoute {
    pub bus_index: usize,
    pub dest_index: usize,
    pub wet: [Vec<f32>; 2],
    pub aligned: [Vec<f32>; 2],
    pub catchup: DelayLine,
    pub route_delay: DelayLine,
}
impl OutputRoute {
    pub fn new(bus_index: usize, dest_index: usize, block: usize) -> Self {
        Self {
            bus_index,
            dest_index,
            wet: [vec![0.; block], vec![0.; block]],
            aligned: [vec![0.; block], vec![0.; block]],
            catchup: delay(0, block),
            route_delay: delay(0, block),
        }
    }
    pub fn capture(
        &mut self,
        plugin: &dyn PluginInstance,
        frames: usize,
    ) -> Result<(), OxitoneError> {
        let output = plugin.output_bus(self.bus_index).ok_or_else(|| {
            OxitoneError::new("RealtimeFault", "insert auxiliary output is unavailable")
        })?;
        for (dst, src) in self.wet.iter_mut().zip(output) {
            if src.len() < frames {
                return Err(OxitoneError::new(
                    "RealtimeFault",
                    "insert auxiliary output is too short",
                ));
            }
            dst[..frames].copy_from_slice(&src[..frames]);
        }
        Ok(())
    }
    pub fn align(&mut self, frames: usize) {
        let [out_l, out_r] = &mut self.aligned;
        if self.catchup.delay_frames() == 0 {
            out_l[..frames].copy_from_slice(&self.wet[0][..frames]);
            out_r[..frames].copy_from_slice(&self.wet[1][..frames]);
            return;
        }
        out_l[..frames].fill(0.);
        out_r[..frames].fill(0.);
        self.catchup.process_add(
            &self.wet[0][..frames],
            &self.wet[1][..frames],
            1.,
            &mut out_l[..frames],
            &mut out_r[..frames],
        );
    }
}

pub(super) struct InputSend {
    pub dest_index: usize,
    pub insert_index: usize,
    pub input_index: usize,
    pub delay: DelayLine,
}
