//! Built-in WavetableSynth (`oxitone.wavetable`), a statically linked Plugin
//! ABI v1 instrument (02-domain-spec.md §内置 WavetableSynth).

mod advanced;
mod banks;
mod control;
mod cycles;
mod instance;
mod motion;
mod params;
mod voice;
pub use cycles::cycle_value;
pub use motion::lfo_value;

/// Source-only preview: one prepared, bandlimited cycle, without creating a voice.
/// Control/UI thread only; allocates the returned drawing samples.
pub fn preview_cycle(
    bank: usize,
    wave: usize,
    target: usize,
    position: f32,
    phase: f64,
    warp_mode: usize,
    warp: f32,
) -> Vec<f32> {
    let tables = WavetableSynthInstance::build_tables(48_000.);
    let (a, b, blend) = if bank == 0 {
        (wave.min(5), target.min(5), position.clamp(0., 1.))
    } else {
        banks::pair(bank.min(3), position)
    };
    let mut reader = oxitone_dsp::oscillator::WavetableReader::new();
    reader.prepare(&tables[a], 50.);
    reader.set_phase(phase);
    (0..=256)
        .map(|_| {
            reader.next_warped(
                &tables[a],
                &tables[b],
                blend,
                48_000. / 256.,
                warp_mode.min(3),
                warp,
                0.,
            )
        })
        .collect()
}

/// Same Sub waveform tables as DSP, displayed at a fixed reference frequency.
pub fn preview_sub_cycle(wave: usize) -> Vec<f32> {
    let tables = WavetableSynthInstance::build_tables(48_000.);
    let table = &tables[banks::sub_table(wave)];
    let mut reader = oxitone_dsp::oscillator::WavetableReader::new();
    reader.prepare(table, 50.);
    (0..=256)
        .map(|_| reader.next(table, 48_000. / 256.))
        .collect()
}

#[cfg(test)]
mod advanced_tests;
#[cfg(test)]
mod motion_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tuning_tests;

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
