//! Built-in Sampler (`oxitone.sampler`), a statically linked Plugin ABI v1
//! instrument (02-domain-spec.md §内置 Sampler). v1 is single-sample: the
//! sample arrives via `InstrumentRef.resources["sample"]` (a sample id
//! resolved through the host's `SampleProvider` at compile time).

mod instance;
mod params;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ParameterSpec;
use oxitone_graph::abi::{HostContext, Plugin, PluginInstance};
use oxitone_graph::descriptor::PluginDescriptor;

use crate::{InstrumentConfig, SampleProvider};

pub use instance::SamplerInstance;

/// Resource key in `InstrumentRef.resources` carrying the sample id.
pub const SAMPLE_RESOURCE_KEY: &str = "sample";

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

/// Process-lifetime descriptor shared by all instances.
pub fn descriptor() -> &'static PluginDescriptor {
    DESCRIPTOR.get_or_init(params::descriptor)
}

/// Parameter specs in descriptor order (borrowed from the static descriptor).
pub fn parameter_specs() -> &'static [ParameterSpec] {
    &descriptor().parameters
}

pub struct SamplerPlugin;

impl SamplerPlugin {
    /// Control-thread configured creation: validates initial parameters and
    /// resolves `resources["sample"]` through `samples`. A missing resource
    /// key is `InvalidProject`; an unknown sample id is `AssetUnavailable`
    /// (03-audio-runtime-spec.md §错误与恢复 — the compiler may keep the
    /// graph and render the node silent instead of failing the compile).
    pub fn create_configured(
        &self,
        host: &HostContext,
        config: &InstrumentConfig<'_>,
        samples: &dyn SampleProvider,
    ) -> Result<SamplerInstance, OxitoneError> {
        if config.state.is_some() {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "oxitone.sampler declares no state schema but received state",
            ));
        }
        let values = crate::params::initial_values(parameter_specs(), config.parameters)?;
        let resources = config.resources.ok_or_else(|| {
            OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "oxitone.sampler requires resources.sample (a sample id)",
                "$.resources",
            )
        })?;
        for key in resources.keys() {
            if key != SAMPLE_RESOURCE_KEY {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("oxitone.sampler does not declare resource {key:?}"),
                    format!("$.resources.{key}"),
                ));
            }
        }
        let sample_id = resources.get(SAMPLE_RESOURCE_KEY).expect("checked above");
        let sample = samples.prepared_sample(sample_id).ok_or_else(|| {
            OxitoneError::with_path(
                codes::ASSET_UNAVAILABLE,
                format!("sample {sample_id:?} is not prepared"),
                format!("$.resources.{SAMPLE_RESOURCE_KEY}"),
            )
        })?;
        Ok(SamplerInstance::new(
            host.sample_rate,
            host.max_block_size as usize,
            values,
            Some(sample),
        ))
    }
}

impl Plugin for SamplerPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        descriptor()
    }

    /// Unconfigured instance (no sample): valid but silent. The graph
    /// compiler uses `create_configured` for built-ins.
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        let values: Vec<f64> = parameter_specs().iter().map(|s| s.default).collect();
        Box::new(SamplerInstance::new(
            host.sample_rate,
            host.max_block_size as usize,
            values,
            None,
        ))
    }
}
