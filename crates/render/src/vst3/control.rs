//! Instance-local control proxy; helper replacement never reuses a retired helper handle.
use super::{error, invalid};
use oxitone_core::OxitoneError;
use oxitone_graph::control::NativeControl;
use oxitone_vst3_host::{control_wire::Request, stream::Controller};
use std::{sync::Mutex, time::Duration};

#[derive(Default)]
pub(super) struct Control {
    controller: Mutex<Option<Controller>>,
}
impl Control {
    /// Prepare/isolated-background only. No locks in PluginInstance::reset/process.
    pub fn replace(&self, controller: Option<Controller>) {
        *self.controller.lock().unwrap_or_else(|e| e.into_inner()) = controller;
    }
}
impl NativeControl for Control {
    fn protocol(&self) -> &'static str {
        "vst3"
    }
    fn request(&self, json: &str, timeout: Duration) -> Result<String, OxitoneError> {
        let request: Request = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        request.validate().map_err(error)?;
        let controller = self
            .controller
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| {
                OxitoneError::new(
                    "PluginTaskConflict",
                    "VST3 processing instance is being replaced",
                )
            })?;
        let result = controller
            .request(request.command, timeout)
            .map_err(error)?;
        serde_json::to_string(&result).map_err(|e| invalid(e.to_string()))
    }
}
