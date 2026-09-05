//! oxitone-graph — plugin descriptor model, Plugin ABI v1 traits, the static
//! plugin registry, snapshot validation, and mixer routing topology. The
//! RenderGraph compiler builds on these foundations; see
//! `.agents/docs/01-architecture.md` (ownership), `08-plugin-abi.md` (ABI),
//! `02-domain-spec.md` (domain rules) and `04-api-contracts.md` (parameter
//! contracts).

pub mod abi;
pub mod abi_c;
pub mod builtin_params;
pub mod compile;
pub mod descriptor;
pub mod registry;
pub mod topology;
pub mod validate;

pub use abi::{
    HostContext, NoteEvent, NoteEventKind, ParameterEvent, Plugin, PluginInstance, ProcessContext,
};
pub use compile::{
    compile_plan, AutomationBinding, BindingTarget, ChannelPlan, ClipLoop, CompileOptions,
    CompiledLane, PlanSampleProvider, RenderPlan, SampleClipPlan,
};
pub use descriptor::{
    ChannelLayout, PluginCapabilities, PluginDescriptor, PluginKind, StateSchemaId,
    SLICER_PLUGIN_ID, SLICER_STATE_SCHEMA_ID,
};
pub use registry::PluginRegistry;
pub use topology::{build_mixer_routing, MixerRouting, RoutingEdge, MASTER_MIXER_CHANNEL_ID};
pub use validate::validate;
