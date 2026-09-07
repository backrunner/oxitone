//! oxitone-instruments — built-in instruments as statically linked Plugin ABI
//! v1 plugins (`.agents/docs/02-domain-spec.md` §内置音源, `08-plugin-abi.md`):
//! WavetableSynth, Sampler, Multisampler, and Slicer share the exact
//! [`Plugin`]/[`PluginInstance`] contract with third-party dylib plugins —
//! there is no private built-in path.
//!
//! ## Host integration: sample and config injection
//!
//! ABI v1 `Plugin::create` only carries [`HostContext`] (sample rate and max
//! block size); it cannot see sample assets or an `InstrumentRef`'s initial
//! parameters, `resources`, or structured `state`. Built-ins therefore expose
//! an additional control-thread entry point, [`create_builtin_instance`],
//! which the graph compiler calls when it compiles a channel whose
//! `pluginId` is one of the built-ins:
//!
//! - initial `parameters` are validated against the descriptor (unknown,
//!   out-of-range, or non-finite values are `InvalidProject`),
//! - sample references (`Sampler`'s `resources["sample"]`, `Slicer`'s
//!   `state.sampleId`) resolve through the host-provided [`SampleProvider`]
//!   (missing assets are `AssetUnavailable`),
//! - the Slicer's structured state is parsed and resolved — including
//!   `onset-v1` detection — into an immutable frame-interval slice table
//!   before `prepare`, so `process` only reads preallocated state.
//!
//! Third-party plugins keep using plain `Plugin::create` plus parameter
//! events. The registry wiring (`PluginRegistry::with_builtins`) lives in
//! `oxitone-graph` and consumes [`builtin_plugins`].

mod block;
pub mod multisampler;
mod params;
mod sample_voice;
pub mod sampler;
pub mod slicer;
pub mod wavetable;

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::Beat;
use oxitone_graph::abi::{HostContext, Plugin, PluginInstance};
use oxitone_samples::PreparedSample;

pub use sampler::SamplerPlugin;
pub use slicer::SlicerPlugin;
pub use wavetable::WavetableSynthPlugin;

/// `pluginId` of the built-in WavetableSynth.
pub const WAVETABLE_PLUGIN_ID: &str = "oxitone.wavetable";
/// `pluginId` of the built-in Sampler.
pub const SAMPLER_PLUGIN_ID: &str = "oxitone.sampler";
/// `pluginId` of the built-in Slicer (mirrors `oxitone_graph::SLICER_PLUGIN_ID`).
pub const SLICER_PLUGIN_ID: &str = oxitone_graph::SLICER_PLUGIN_ID;

/// Built-in instrument plugin versions (ABI plugin version, not crate version).
pub const BUILTIN_PLUGIN_VERSION: &str = "1.0.0";

/// Host-provided prepared-sample lookup. Called on the control thread during
/// graph compile (`create_configured`); never reachable from `process`.
pub trait SampleProvider {
    fn prepared_sample(&self, sample_id: &str) -> Option<Arc<PreparedSample>>;
}

/// Compile-time mirror of `InstrumentRef` (04-api-contracts.md). All data is
/// borrowed from the snapshot being compiled; the configured instance copies
/// what it needs before `prepare`.
pub struct InstrumentConfig<'a> {
    pub parameters: &'a BTreeMap<String, f64>,
    pub resources: Option<&'a BTreeMap<String, String>>,
    pub state: Option<&'a serde_json::Value>,
}

impl<'a> InstrumentConfig<'a> {
    /// Config carrying only initial parameters.
    pub fn new(parameters: &'a BTreeMap<String, f64>) -> Self {
        Self {
            parameters,
            resources: None,
            state: None,
        }
    }
}

/// All built-in instrument plugins, ready for `PluginRegistry::register`.
pub fn builtin_plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(WavetableSynthPlugin),
        Arc::new(SamplerPlugin),
        Arc::new(multisampler::MultisamplerPlugin),
        Arc::new(SlicerPlugin),
    ]
}

/// Create a configured built-in instance from an `InstrumentRef`-shaped
/// config. This is the graph compiler's integration point for built-ins;
/// `beat_to_frame` converts `{ beat }` slice markers through the compiler's
/// baked tempo table (required only when a Slicer state uses beat markers).
pub fn create_builtin_instance(
    plugin_id: &str,
    host: &HostContext,
    config: &InstrumentConfig<'_>,
    samples: &dyn SampleProvider,
    beat_to_frame: Option<&dyn Fn(Beat) -> Option<u64>>,
) -> Result<Box<dyn PluginInstance>, OxitoneError> {
    match plugin_id {
        oxitone_graph::multisampler::PLUGIN_ID => Ok(Box::new(
            multisampler::MultisamplerPlugin.create_configured(host, config, samples)?,
        )),
        WAVETABLE_PLUGIN_ID => Ok(Box::new(
            WavetableSynthPlugin.create_configured(host, config)?,
        )),
        SAMPLER_PLUGIN_ID => Ok(Box::new(
            SamplerPlugin.create_configured(host, config, samples)?,
        )),
        SLICER_PLUGIN_ID => Ok(Box::new(SlicerPlugin.create_configured(
            host,
            config,
            samples,
            beat_to_frame,
        )?)),
        other => Err(OxitoneError::new(
            codes::INVALID_PROJECT,
            format!("{other:?} is not a built-in instrument plugin"),
        )),
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use oxitone_core::wire::SampleFormat;
    use oxitone_graph::abi::{NoteEvent, ParameterEvent, PluginInstance, ProcessContext};
    use oxitone_samples::{ChannelLayoutAction, SampleMetadata};

    /// Map-backed sample provider for tests.
    pub struct MapSamples(pub BTreeMap<String, Arc<PreparedSample>>);

    impl SampleProvider for MapSamples {
        fn prepared_sample(&self, sample_id: &str) -> Option<Arc<PreparedSample>> {
            self.0.get(sample_id).cloned()
        }
    }

    /// Prepared mono/stereo sample at 48 kHz with optional loop points.
    pub fn prepared(channels: Vec<Vec<f32>>, loop_points: Option<(u64, u64)>) -> PreparedSample {
        PreparedSample {
            channels,
            sample_rate: 48_000,
            loop_points: loop_points.map(|(start_frame, end_frame)| oxitone_samples::LoopPoints {
                start_frame,
                end_frame,
            }),
            musical_length_beats: None,
            metadata: SampleMetadata {
                sha256: "test".to_string(),
                format: SampleFormat::Wav,
                source_sample_rate: 48_000,
                source_channels: 1,
                source_bit_depth: Some(32),
                channel_layout_action: ChannelLayoutAction::Kept,
                decoder: None,
            },
        }
    }

    pub fn host() -> HostContext {
        HostContext {
            sample_rate: 48_000.0,
            max_block_size: 128,
        }
    }

    /// Events for one block.
    #[derive(Default)]
    pub struct Block<'a> {
        pub notes: Vec<NoteEvent>,
        pub params: Vec<ParameterEvent<'a>>,
    }

    pub fn note_on(offset: u32, pitch: u8, velocity: f32) -> NoteEvent {
        NoteEvent {
            frame_offset: offset,
            kind: oxitone_graph::abi::NoteEventKind::NoteOn,
            pitch,
            velocity,
        }
    }

    pub fn note_off(offset: u32, pitch: u8) -> NoteEvent {
        NoteEvent {
            frame_offset: offset,
            kind: oxitone_graph::abi::NoteEventKind::NoteOff,
            pitch,
            velocity: 1.0,
        }
    }

    pub const FRAMES: usize = 128;

    /// Drive `instance` through `blocks` blocks of `FRAMES` frames at 48 kHz,
    /// returning the concatenated stereo output.
    pub fn run(instance: &mut dyn PluginInstance, blocks: &[Block<'_>]) -> (Vec<f32>, Vec<f32>) {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for block in blocks {
            let mut l = [0.0f32; FRAMES];
            let mut r = [0.0f32; FRAMES];
            {
                let mut outputs: [&mut [f32]; 2] = [&mut l, &mut r];
                let mut ctx = ProcessContext {
                    frames: FRAMES,
                    sample_rate: 48_000.0,
                    inputs: &[],
                    outputs: &mut outputs,
                    note_events: &block.notes,
                    parameter_events: &block.params,
                    sidechain: None,
                };
                instance.process(&mut ctx);
            }
            left.extend_from_slice(&l);
            right.extend_from_slice(&r);
        }
        (left, right)
    }

    /// `n` silent blocks.
    pub fn silence(n: usize) -> Vec<Block<'static>> {
        (0..n).map(|_| Block::default()).collect()
    }

    pub fn rms(buf: &[f32]) -> f64 {
        if buf.is_empty() {
            return 0.0;
        }
        (buf.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>() / buf.len() as f64).sqrt()
    }

    pub fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |a, &b| a.max(b.abs()))
    }
}
