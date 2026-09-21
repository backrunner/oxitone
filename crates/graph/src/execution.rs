//! Explicit execution domains. Isolated processing is NEVER an audio-callback entry point.
use crate::{PluginInstance, ProcessContext};
use oxitone_core::OxitoneError;

/// Exact first-sample position of a graph segment, in quarter-note beats.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessPosition {
    pub mode: IsolatedMode,
    pub project_frame: u64,
    pub continuous_frame: u64,
    pub project_beat: f64,
    pub bar_beat: f64,
    pub tempo: f64,
    pub time_signature: [i32; 2],
    pub playing: bool,
    pub cycle: Option<[f64; 2]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IsolatedMode {
    #[default]
    Realtime,
    Offline,
}

mod sealed {
    pub trait Sealed {}
}

/// Sealed dispatch so realtime monomorphizations cannot call external processing.
pub trait Execution: sealed::Sealed {
    fn process(
        plugin: &mut dyn PluginInstance,
        context: &mut ProcessContext<'_>,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError>;
}

pub struct Realtime;
pub struct Isolated;
pub struct Offline;
impl sealed::Sealed for Realtime {}
impl sealed::Sealed for Isolated {}
impl sealed::Sealed for Offline {}
impl Execution for Offline {
    fn process(
        plugin: &mut dyn PluginInstance,
        context: &mut ProcessContext<'_>,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        let position = ProcessPosition {
            mode: IsolatedMode::Offline,
            ..*position
        };
        plugin.process_isolated(context, &position)
    }
}
impl Execution for Realtime {
    fn process(
        plugin: &mut dyn PluginInstance,
        context: &mut ProcessContext<'_>,
        _: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        plugin.process(context);
        Ok(())
    }
}
impl Execution for Isolated {
    fn process(
        plugin: &mut dyn PluginInstance,
        context: &mut ProcessContext<'_>,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        plugin.process_isolated(context, position)
    }
}
