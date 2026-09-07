//! Built-in WavetableSynth (`oxitone.wavetable`), a statically linked Plugin
//! ABI v1 instrument (02-domain-spec.md §内置 WavetableSynth).

mod control;
mod cycles;
mod instance;
mod motion;
mod params;
mod voice;
pub use cycles::cycle_value;
pub use motion::lfo_value;

#[cfg(test)]
mod motion_tests;
#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ParameterSpec;
use oxitone_graph::abi::{HostContext, Plugin, PluginInstance};
use oxitone_graph::descriptor::PluginDescriptor;

use crate::InstrumentConfig;

pub use instance::WavetableSynthInstance;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

/// Process-lifetime descriptor shared by all instances.
pub fn descriptor() -> &'static PluginDescriptor {
    DESCRIPTOR.get_or_init(params::descriptor)
}

/// Parameter specs in descriptor order (borrowed from the static descriptor).
pub fn parameter_specs() -> &'static [ParameterSpec] {
    &descriptor().parameters
}

pub struct WavetableSynthPlugin;

impl WavetableSynthPlugin {
    /// Control-thread configured creation: validates initial parameters
    /// against the descriptor. WavetableSynth takes no resources or state.
    pub fn create_configured(
        &self,
        host: &HostContext,
        config: &InstrumentConfig<'_>,
    ) -> Result<WavetableSynthInstance, OxitoneError> {
        if config.resources.is_some_and(|r| !r.is_empty()) {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "oxitone.wavetable does not declare resources",
            ));
        }
        if config.state.is_some() {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "oxitone.wavetable declares no state schema but received state",
            ));
        }
        let values = crate::params::initial_values(parameter_specs(), config.parameters)?;
        Ok(WavetableSynthInstance::new(
            host.sample_rate,
            host.max_block_size as usize,
            values,
        ))
    }
}

impl Plugin for WavetableSynthPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        descriptor()
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        let values: Vec<f64> = parameter_specs().iter().map(|s| s.default).collect();
        Box::new(WavetableSynthInstance::new(
            host.sample_rate,
            host.max_block_size as usize,
            values,
        ))
    }
}
