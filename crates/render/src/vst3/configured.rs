//! Inspect restored capabilities in an isolated helper. The base catalog remains immutable.
use super::*;
use oxitone_vst3_host::control_wire::Command;
use std::time::Duration;

impl Vst3Plugin {
    pub(super) fn configured(
        &self,
        host: &HostContext,
        parameters: &BTreeMap<String, f64>,
        state: Option<&serde_json::Value>,
    ) -> Result<Option<Arc<dyn Plugin>>, OxitoneError> {
        if state.is_none() && parameters.is_empty() {
            return Ok(None);
        }
        if host.sample_rate.fract() != 0. || !(8000. ..=192000.).contains(&host.sample_rate) {
            return Err(invalid("invalid VST3 sample rate"));
        }
        let mut start = self
            .options
            .start(host.sample_rate as u32, host.max_block_size as usize);
        // The saved configuration can change physical bus counts. Discover before choosing routes.
        start.options.bus_activation = None;
        start.options.configuration = state
            .map(|s| serde_json::from_value(s.clone()).map_err(|e| invalid(e.to_string())))
            .transpose()?;
        start.options.parameters = parameters.clone();
        let (session, port) =
            Session::spawn(&self.options.helper_path, start, SessionOptions::default())
                .map_err(error)?;
        let captured = session
            .controller()
            .request(Command::Capture {}, Duration::from_secs(5))
            .map_err(error)?;
        if captured.restart.is_some() {
            return Err(OxitoneError::new(
                "PluginRestartRequired",
                "VST3 configuration changed again during preparation",
            ));
        }
        let mut options = self.options.clone();
        options.metadata = serde_json::from_value(
            captured
                .info
                .ok_or_else(|| invalid("VST3 capture omitted capabilities"))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
        options.metadata.validate(&options.source)?;
        options.metadata.check_ready(session.info())?;
        let descriptor = options.metadata.descriptor()?;
        if descriptor.kind != self.descriptor.kind {
            return Err(invalid("VST3 configuration changed plugin kind"));
        }
        drop(port);
        drop(session);
        Ok(Some(Arc::new(Self {
            registration: self.registration.clone(),
            descriptor,
            options,
            faults: self.faults.clone(),
        })))
    }
}
