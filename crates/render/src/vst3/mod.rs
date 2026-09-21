//! Isolated VST3 factories. Only the helper loads plugin code; no C ABI impersonation.
mod configured;
mod control;
mod instance;
mod metadata;

use metadata::Metadata;
use oxitone_core::{wire::AllowPlugins, OxitoneError};
use oxitone_graph::{HostContext, Plugin, PluginDescriptor, PluginInstance};
use oxitone_vst3_host::{
    stream::{Session, SessionOptions},
    stream_wire::{Options, Start, STREAM_VERSION},
    wire::Source,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationOptions {
    pub registration_version: u32,
    pub source: Source,
    pub helper_path: PathBuf,
    metadata: Metadata,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub registration_version: u32,
    pub plugin_id: String,
    pub plugin_version: String,
    pub sha256: String,
    pub kind: &'static str,
    pub parameters: Vec<oxitone_core::wire::ParameterSpec>,
}

pub struct Vst3Plugin {
    pub registration: Registration,
    descriptor: PluginDescriptor,
    options: RegistrationOptions,
    faults: Arc<AtomicU64>,
}

pub(super) fn error(error: oxitone_vst3_host::Error) -> OxitoneError {
    OxitoneError::new(error.code, error.message)
}
pub(super) fn invalid(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new("PluginConfigInvalid", message)
}

impl Vst3Plugin {
    /// Control-only registration. Inspect identity and capabilities again in a fresh helper.
    pub fn load(
        mut options: RegistrationOptions,
        policy: AllowPlugins,
    ) -> Result<Arc<Self>, OxitoneError> {
        if options.registration_version != 1 {
            return Err(OxitoneError::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 registration",
            ));
        }
        if matches!(policy, AllowPlugins::SignedOnly) {
            options.source.allow_plugins = oxitone_vst3_host::wire::Policy::SignedOnly;
        }
        options.source.validate().map_err(error)?;
        let meta = &options.metadata;
        meta.validate(&options.source)?;
        options.source.expected_hash = Some(meta.sha256.clone());
        let start = options.start(48_000, 128);
        let (session, port) =
            Session::spawn(&options.helper_path, start, SessionOptions::default())
                .map_err(error)?;
        meta.check_ready(session.info())?;
        let descriptor = meta.descriptor()?;
        let registration = Registration {
            registration_version: 1,
            plugin_id: descriptor.plugin_id.clone(),
            plugin_version: descriptor.plugin_version.clone(),
            sha256: meta.sha256.clone(),
            kind: if meta.is_instrument() {
                "instrument"
            } else {
                "effect"
            },
            parameters: descriptor.parameters.clone(),
        };
        drop(port);
        drop(session);
        Ok(Arc::new(Self {
            registration,
            descriptor,
            options,
            faults: Arc::new(AtomicU64::new(0)),
        }))
    }

    pub fn fault_count(&self) -> u64 {
        self.faults.load(Ordering::Relaxed)
    }
    pub fn display_name(&self) -> &str {
        &self.options.metadata.name
    }
}

impl RegistrationOptions {
    fn start(&self, sample_rate: u32, block_size: usize) -> Start {
        Start {
            stream_protocol_version: STREAM_VERSION,
            source: self.source.clone(),
            options: Options {
                midi_output: false,
                sample_rate,
                block_size,
                configuration: None,
                parameters: BTreeMap::new(),
                processing_mode: oxitone_vst3_host::stream_wire::ProcessingMode::Realtime,
                tempo: 120.0,
                time_signature: [4, 4],
                transport: None,
                bus_activation: Some(oxitone_vst3_host::bus_wire::BusActivation {
                    inputs: (0..self.metadata.audio_buses.inputs.len())
                        .map(|i| i == 0 || !self.metadata.is_instrument())
                        .collect(),
                    outputs: (0..self.metadata.audio_buses.outputs.len())
                        .map(|i| i == 0)
                        .collect(),
                }),
            },
        }
    }
}

impl Plugin for Vst3Plugin {
    fn midi_input(&self) -> bool {
        self.options.metadata.note_input
    }
    fn midi_output(&self) -> bool {
        self.options.metadata.note_output
    }
    fn input_bus_count(&self) -> usize {
        self.options.metadata.audio_buses.inputs.len()
    }
    fn configuration_dependent(&self) -> bool {
        true
    }
    fn configured_factory(
        &self,
        host: &HostContext,
        parameters: &BTreeMap<String, f64>,
        state: Option<&serde_json::Value>,
    ) -> Result<Option<Arc<dyn Plugin>>, OxitoneError> {
        self.configured(host, parameters, state)
    }
    fn output_bus_count(&self) -> usize {
        self.options.metadata.audio_buses.outputs.len()
    }
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(instance::Instance::new(
            self.options.clone(),
            host,
            self.faults.clone(),
        ))
    }
    fn try_create_configured(
        &self,
        host: &HostContext,
        parameters: &BTreeMap<String, f64>,
        state: Option<&serde_json::Value>,
    ) -> Result<Box<dyn PluginInstance>, OxitoneError> {
        let mut instance = instance::Instance::new(self.options.clone(), host, self.faults.clone());
        instance.configure(parameters, state)?;
        Ok(Box::new(instance))
    }
}
