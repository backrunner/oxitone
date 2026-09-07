//! oxitone-render — RenderGraph assembly, the block renderer, sample-clip
//! playback (`tempoSync` off/stretch/repitch), offline transport, WAV export
//! with stems/loudness, and SMF export (`midi`). See
//! `.agents/docs/03-audio-runtime-spec.md` (DSP 处理顺序, Offline parity),
//! `02-domain-spec.md` (Playback 与 Export 语义) and `06-format-and-export.md`
//! (WAV 导出).
//!
//! Ownership split with `oxitone-graph`: the graph crate compiles the
//! immutable [`oxitone_graph::RenderPlan`] (tempo bake, schedule, automation
//! bindings, sample-clip plans); this crate owns every runtime object built
//! from it — plugin instances, the mixer engine, sample players, the master
//! limiter — because those require depending on `oxitone-instruments` and
//! `oxitone-mixer`, which would be a dependency cycle inside `oxitone-graph`.

pub mod assets;
pub mod bindings;
pub mod build;
pub mod build_plugins;
pub mod channel;
pub mod clip;
pub mod dispatch;
pub mod effect_targets;
pub mod graph;
mod insert;
pub mod loudness;
pub mod metronome;
pub mod midi;
mod param_index;
pub mod params;
pub mod player;
pub mod plugins;
pub mod realtime;
pub mod render_wav;
mod slicer_tempo;
pub mod transport;
pub mod wav;

use std::sync::Arc;

use oxitone_core::error::OxitoneError;
use oxitone_graph::{Plugin, PluginRegistry};

pub use assets::SampleStore;
pub use build::RenderGraphOptions;
pub use graph::RenderGraph;
pub use params::{
    resolve_parameter_event, ParamTargetIndex, ParameterEventInput, QueuedParameterEvent,
};
pub use render_wav::{
    render_wav, BitDepth, DitherMode, RenderFileReport, RenderOptions, RenderPoint, RenderReport,
    StemMode,
};
pub use transport::{Transport, TransportState};

/// Standard plugin assembly: built-in instruments + built-in effects
/// (08-plugin-abi.md §加载模型). This is the registry
/// `PluginRegistry::with_builtins` refers to.
pub fn builtin_registry() -> Result<PluginRegistry, OxitoneError> {
    let plugins: Vec<Arc<dyn Plugin>> = oxitone_instruments::builtin_plugins()
        .into_iter()
        .chain(
            oxitone_mixer::builtin_effect_plugins()
                .into_iter()
                .map(Arc::from),
        )
        .collect();
    PluginRegistry::with_builtins(plugins)
}
