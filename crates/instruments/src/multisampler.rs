//! Native key/velocity-mapped sample bank. Loading and table construction are control-thread only.
use crate::{sampler::SamplerInstance, InstrumentConfig, SampleProvider};
use oxitone_core::{
    codes,
    wire::{ParameterMapping, ParameterSpec, ParameterUnit},
    OxitoneError,
};
use oxitone_graph::{
    abi::{HostContext, Plugin, PluginInstance},
    descriptor::PluginDescriptor,
};
use oxitone_samples::PreparedSample;
use std::sync::{Arc, OnceLock};

// All remaining indices deliberately match Sampler; slot zero is transpose, not rootKey.
pub(crate) const TRANSPOSE: usize = crate::sampler::params::ROOT_KEY;
pub(crate) struct Region {
    pub sample: Arc<PreparedSample>,
    pub root: f64,
    pub gain: f64,
}
pub(crate) struct Bank {
    pub regions: Vec<Region>,
    pub lookup: Vec<u16>,
}
impl Bank {
    pub fn select(&self, key: u8, velocity: f32) -> Option<usize> {
        let v = (velocity.clamp(0., 1.) * 127.).round().clamp(1., 127.) as usize;
        let index = *self.lookup.get(key as usize * 128 + v)?;
        (index != u16::MAX).then_some(index as usize)
    }
}
static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();
pub fn descriptor() -> &'static PluginDescriptor {
    DESCRIPTOR.get_or_init(|| {
        let mut d = crate::sampler::descriptor().clone();
        d.plugin_id = oxitone_graph::multisampler::PLUGIN_ID.into();
        d.state_schema = Some(oxitone_graph::multisampler::SCHEMA_ID);
        let transpose = &mut d.parameters[TRANSPOSE];
        transpose.id = "transpose".into();
        transpose.label = "Transpose".into();
        transpose.unit = ParameterUnit::Semitones;
        transpose.min = -48.;
        transpose.max = 48.;
        transpose.default = 0.;
        transpose.mapping = Some(ParameterMapping::Bipolar);
        d
    })
}
pub fn parameter_specs() -> &'static [ParameterSpec] {
    &descriptor().parameters
}
pub struct MultisamplerPlugin;
impl MultisamplerPlugin {
    pub fn create_configured(
        &self,
        host: &HostContext,
        config: &InstrumentConfig<'_>,
        samples: &dyn SampleProvider,
    ) -> Result<SamplerInstance, OxitoneError> {
        let values = crate::params::initial_values(parameter_specs(), config.parameters)?;
        let state = config.state.ok_or_else(|| {
            OxitoneError::new(codes::INVALID_PROJECT, "multisampler requires region state")
        })?;
        let (state, lookup) =
            oxitone_graph::multisampler::parse(state, config.resources, "$.state")?;
        let mut regions = Vec::with_capacity(state.regions.len());
        for region in state.regions {
            let id = &config.resources.unwrap()[&region.resource];
            let sample = samples.prepared_sample(id).ok_or_else(|| {
                OxitoneError::new(
                    codes::ASSET_UNAVAILABLE,
                    format!("multisampler sample {id:?} is not prepared"),
                )
            })?;
            regions.push(Region {
                sample,
                root: region.root_key as f64,
                gain: region.gain,
            });
        }
        Ok(SamplerInstance::with_bank(
            host.sample_rate,
            host.max_block_size as usize,
            values,
            Bank { regions, lookup },
        ))
    }
}

#[cfg(test)]
#[path = "multisampler_tests.rs"]
mod tests;
impl Plugin for MultisamplerPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        descriptor()
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(SamplerInstance::with_bank(
            host.sample_rate,
            host.max_block_size as usize,
            parameter_specs().iter().map(|p| p.default).collect(),
            Bank {
                regions: vec![],
                lookup: vec![u16::MAX; 128 * 128],
            },
        ))
    }
}
