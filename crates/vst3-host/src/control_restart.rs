//! Freeze a changed processor before any further old-layout processing. Helper main thread only.
use super::*;
use crate::control_wire::{Restart, RestartReason};

pub(super) struct Frozen {
    pub summary: Restart,
    pub info: serde_json::Value,
}
pub(super) fn required() -> crate::Error {
    crate::Error::new(
        "PluginRestartRequired",
        "VST3 state is frozen; capture it and prepare a replacement graph",
    )
}
impl Controls {
    pub fn restart_required(&self) -> bool {
        self.frozen.is_some()
    }
    fn freeze(
        &mut self,
        plugin: &mut Plugin,
        ready: &Ready,
        reasons: Vec<RestartReason>,
    ) -> Result<()> {
        self.close(plugin)?;
        self.edits.invalidate_for_restart();
        let info = crate::inspection::inspect_frozen(plugin, ready.sha256.clone())?;
        let summary = Restart {
            reasons,
            latency_frames: plugin.latency_samples(),
            tail_frames: plugin.tail_samples(),
        };
        plugin.stop_processing().map_err(native)?;
        self.frozen = Some(Frozen { summary, info });
        Ok(())
    }
    /// Taking flags refreshes vendor parameter/bus caches, without renegotiating the old buffers.
    pub(super) fn check_restart(&mut self, plugin: &mut Plugin, ready: &Ready) -> Result<bool> {
        if self.restart_required() {
            return Ok(false);
        }
        let flags = plugin.take_restart_flags();
        let mut reasons = Vec::new();
        if flags.io_changed() {
            reasons.push(RestartReason::Io);
        }
        if flags.latency_changed() || plugin.latency_samples() != ready.latency_frames {
            reasons.push(RestartReason::Latency);
        }
        if flags.param_id_mapping_changed() || flags.param_titles_changed() {
            reasons.push(RestartReason::Parameters);
        }
        if flags.reload_component() {
            reasons.push(RestartReason::Reload);
        }
        if !reasons.is_empty() {
            self.freeze(plugin, ready, reasons)?;
            return Ok(false);
        }
        Ok(flags.param_values_changed())
    }
    pub(super) fn sync_values(&mut self, plugin: &mut Plugin, ready: &Ready) -> Result<()> {
        let mut parameters = crate::inspection::parameters(plugin)?;
        parameters.sort_by_key(|p| p.id);
        if parameters.len() != ready.parameters.len()
            || parameters
                .iter()
                .zip(&ready.parameters)
                .any(|(actual, expected)| {
                    actual.id != expected.id
                        || actual.is_read_only == expected.writable
                        || actual.can_automate != expected.automatable
                })
        {
            return self.freeze(plugin, ready, vec![RestartReason::Parameters]);
        }
        for parameter in parameters {
            if !parameter.is_read_only {
                plugin
                    .set_parameter(parameter.id, parameter.value)
                    .map_err(native)?;
            }
        }
        Ok(())
    }
    pub(super) fn flush(&mut self, plugin: &mut Plugin, ready: &Ready) -> Result<()> {
        for _ in 0..8 {
            // Mirroring a values notification can itself request a layout change.
            if !self.can_process(plugin, ready)? {
                return Ok(());
            }
            plugin.process_bus_audio(&mut self.silent).map_err(native)?;
            plugin.get_parameter_changes();
            if !self.check_restart(plugin, ready)? {
                return Ok(());
            }
            self.sync_values(plugin, ready)?;
        }
        Err(crate::Error::new(
            "RealtimeFault",
            "VST3 parameter refresh did not settle after 8 rounds",
        ))
    }
}
