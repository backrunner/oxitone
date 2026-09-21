//! Transport segmentation shared by explicitly separate execution domains.
use super::*;
use oxitone_core::OxitoneError;
use oxitone_graph::execution::{Execution, Isolated, Offline, Realtime};

impl RenderGraph {
    /// Render one block at the transport cursor into `out_l`/`out_r`
    /// (overwrite; `out` length must equal the compiled block size). Silent
    /// blocks are produced while stopped/paused without advancing the
    /// cursor. RT-safe: no allocation, no locks, no I/O.
    pub fn process_block(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        if self.requires_isolation() {
            out_l.fill(0.0);
            out_r.fill(0.0);
            self.faulted = true;
            return;
        }
        let _ = self.process_with::<Realtime>(out_l, out_r);
    }

    /// Background/offline entry point. External processing may wait; never call from a callback.
    pub fn process_isolated_block(
        &mut self,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) -> Result<(), OxitoneError> {
        let result = self.process_with::<Isolated>(out_l, out_r);
        if result.is_err() {
            out_l.fill(0.0);
            out_r.fill(0.0);
            if self.plan.midi_routing.is_some() {
                self.seek(self.transport.cursor);
            }
            self.transport.stop();
        }
        result
    }

    pub fn requires_isolation(&self) -> bool {
        self.isolated
    }

    /// Offline only, including the plugin's explicit offline processing mode.
    pub fn process_offline_block(
        &mut self,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) -> Result<(), OxitoneError> {
        let result = self.process_with::<Offline>(out_l, out_r);
        if result.is_err() {
            out_l.fill(0.0);
            out_r.fill(0.0);
            if self.plan.midi_routing.is_some() {
                self.seek(self.transport.cursor);
            }
            self.transport.stop();
        }
        result
    }

    fn process_with<E: Execution>(
        &mut self,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) -> Result<(), OxitoneError> {
        let frames = out_l.len().min(out_r.len()).min(self.block_size);
        out_l.fill(0.0);
        out_r.fill(0.0);
        if !self.transport.running() {
            return Ok(());
        }
        let mut offset = 0;
        while offset < frames {
            let cur = self.transport.cursor;
            let mut count = frames - offset;
            if self.transport.state == TransportState::Playing {
                if let Some((start, end)) = self.transport.loop_region {
                    if end > start {
                        if cur >= end {
                            self.seek(start);
                            continue;
                        }
                        count = count.min((end - cur) as usize);
                    }
                }
            }
            if let Some(event) = self.param_queue.iter().find(|e| e.frame > cur) {
                count = count.min(event.frame.saturating_sub(cur).min(count as u64) as usize);
            }
            for n in 1..count {
                if self
                    .rt_bindings
                    .iter()
                    .any(|b| crate::bindings::binding_due(self, b, cur + n as u64))
                {
                    count = n;
                    break;
                }
            }
            self.process_segment::<E>(
                &mut out_l[offset..offset + count],
                &mut out_r[offset..offset + count],
                offset == 0,
            )?;
            self.mixer.capture_stem_segment(offset, count);
            self.metro_block_l[offset..offset + count].copy_from_slice(&self.metro_l[..count]);
            self.metro_block_r[offset..offset + count].copy_from_slice(&self.metro_r[..count]);
            if self.transport.cursor < cur + count as u64 {
                let iteration = self.eval_ctx.loop_iteration.wrapping_add(1);
                self.seek(self.transport.cursor);
                self.eval_ctx.loop_iteration = iteration;
            }
            offset += count;
        }
        Ok(())
    }
}
