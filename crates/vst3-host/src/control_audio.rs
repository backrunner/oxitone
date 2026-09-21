//! The helper's serialized audio/control boundary. Device callbacks never call these methods.
use super::*;
impl Controls {
    /// Controller mirroring of queued automation may synchronously change the layout.
    /// Detect that before passing the old audio buffers to the processor.
    pub fn can_process(&mut self, plugin: &mut Plugin, ready: &Ready) -> Result<bool> {
        for _ in 0..8 {
            if !self.check_restart(plugin, ready)? {
                return Ok(!self.restart_required());
            }
            self.sync_values(plugin, ready)?;
        }
        Err(crate::Error::new(
            "RealtimeFault",
            "VST3 parameter refresh did not settle before processing",
        ))
    }
    pub fn after_audio(&mut self, plugin: &mut Plugin, ready: &Ready) -> Result<()> {
        if self.check_restart(plugin, ready)? {
            self.sync_values(plugin, ready)?;
            self.flush(plugin, ready)?;
        }
        self.edits.collect(plugin, ready);
        Ok(())
    }
    pub fn before_audio(
        &mut self,
        plugin: &mut Plugin,
        ready: &Ready,
        position: crate::edit_wire::Position,
        frames: usize,
    ) {
        self.edits.collect(plugin, ready);
        self.edits.before_audio(position, frames);
    }
    pub fn apply_events(
        &mut self,
        plugin: &mut Plugin,
        events: &mut [crate::wire::Event],
        payload: &[u8],
        frames: usize,
        playing: bool,
    ) -> Result<()> {
        let count = self.edits.filter_audio(plugin, events, playing)?;
        crate::events::apply(plugin, &events[..count], payload, 0, frames)
    }
}
