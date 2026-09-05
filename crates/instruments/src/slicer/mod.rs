//! Built-in Slicer (`oxitone.slicer`), a statically linked Plugin ABI v1
//! instrument (02-domain-spec.md §内置 Slicer). The slice table arrives as
//! structured state through `InstrumentRef.state` (schema
//! `oxitone.slicer.slices@1`, 04-api-contracts.md §SlicerState) and is
//! resolved — markers / grid / `onset-v1` included — into an immutable
//! frame-interval table at compile time.

mod instance;
mod onset;
mod params;
mod resolve;
mod state;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ParameterSpec;
use oxitone_core::Beat;
use oxitone_graph::abi::{HostContext, Plugin, PluginInstance};
use oxitone_graph::descriptor::PluginDescriptor;

use crate::{InstrumentConfig, SampleProvider};

pub use instance::SlicerInstance;
pub use onset::{detect_onsets_v1, ONSET_ALGORITHM_V1};
pub use resolve::resolve_slices;
pub use state::{parse_state, PlayMode, ResolvedSlice, SlicerConfig, MAX_SLICES};

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

/// Process-lifetime descriptor shared by all instances.
pub fn descriptor() -> &'static PluginDescriptor {
    DESCRIPTOR.get_or_init(params::descriptor)
}

/// Parameter specs in descriptor order (borrowed from the static descriptor).
pub fn parameter_specs() -> &'static [ParameterSpec] {
    &descriptor().parameters
}

pub struct SlicerPlugin;

impl SlicerPlugin {
    /// Control-thread configured creation: validates initial parameters,
    /// parses and resolves the structured state (including `onset-v1`
    /// detection), and resolves `state.sampleId` through `samples`.
    /// `beat_to_frame` is the compiler's baked tempo table, required only
    /// when slice markers use `{ beat }` positions.
    pub fn create_configured(
        &self,
        host: &HostContext,
        config: &InstrumentConfig<'_>,
        samples: &dyn SampleProvider,
        beat_to_frame: Option<&dyn Fn(Beat) -> Option<u64>>,
    ) -> Result<SlicerInstance, OxitoneError> {
        if config.resources.is_some_and(|r| !r.is_empty()) {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "oxitone.slicer takes its sample via state.sampleId, not resources",
            ));
        }
        let state = config.state.ok_or_else(|| {
            OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "oxitone.slicer requires InstrumentRef.state (the slice table)",
                "$.state",
            )
        })?;
        let values = crate::params::initial_values(parameter_specs(), config.parameters)?;
        let parsed = parse_state(state)?;
        let sample = samples.prepared_sample(&parsed.sample_id).ok_or_else(|| {
            OxitoneError::with_path(
                codes::ASSET_UNAVAILABLE,
                format!("sample {:?} is not prepared", parsed.sample_id),
                "$.state.sampleId",
            )
        })?;
        let slices = resolve_slices(&parsed, &sample, beat_to_frame)?;
        Ok(SlicerInstance::new(
            host.sample_rate,
            host.max_block_size as usize,
            values,
            Some(sample),
            slices,
            parsed.trigger_note,
            parsed.play_mode,
        ))
    }
}

impl Plugin for SlicerPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        descriptor()
    }

    /// Unconfigured instance (no state/sample): valid but silent. The graph
    /// compiler uses `create_configured` for built-ins.
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        let values: Vec<f64> = parameter_specs().iter().map(|s| s.default).collect();
        Box::new(SlicerInstance::new(
            host.sample_rate,
            host.max_block_size as usize,
            values,
            None,
            Vec::new(),
            60,
            PlayMode::Oneshot,
        ))
    }
}
