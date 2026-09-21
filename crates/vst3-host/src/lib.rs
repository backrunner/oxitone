//! Isolated VST3 host and bounded native audio transport, shared by SDK and engine adapters.
pub mod bundle;
#[cfg(all(feature = "stream", feature = "host", unix))]
mod bus_buffers;
pub mod bus_wire;
#[cfg(feature = "host")]
mod configuration;
pub mod configuration_wire;
#[cfg(feature = "host")]
pub mod configure;
#[cfg(all(feature = "stream", feature = "host", unix))]
mod control_server;
pub mod control_wire;
#[cfg(feature = "host")]
pub mod discovery;
#[cfg(all(feature = "stream", feature = "host", unix))]
mod edit_journal;
pub mod edit_wire;
#[cfg(all(feature = "host", target_os = "macos"))]
pub mod editor;
pub mod event_wire;
#[cfg(feature = "host")]
mod events;
#[cfg(feature = "host")]
pub mod hosting;
#[cfg(feature = "host")]
mod inspection;
pub mod manager_wire;
#[cfg(all(feature = "stream", feature = "host", unix))]
mod output_midi;
#[cfg(feature = "host")]
mod process_buffers;
#[cfg(feature = "host")]
mod render;
pub mod schedule_wire;
#[cfg(feature = "host")]
mod silent_processing;
#[cfg(all(feature = "stream", unix))]
pub mod stream;
#[cfg(all(feature = "stream", unix))]
mod stream_codec;
#[cfg(all(feature = "stream", feature = "host", unix))]
pub mod stream_server;
#[cfg(all(feature = "stream", unix))]
mod stream_thread;
pub mod stream_wire;
#[cfg(all(feature = "stream", unix))]
mod transport_codec;
#[cfg(all(test, feature = "stream", unix))]
mod transport_tests;
pub mod transport_wire;
#[cfg(feature = "host")]
mod wave;
pub mod wire;

#[derive(Debug, serde::Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(code: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn invalid(message: impl ToString) -> Error {
    Error::new("PluginConfigInvalid", message)
}
pub fn unsupported(message: impl ToString) -> Error {
    Error::new("PluginCapabilityUnsupported", message)
}
#[cfg(feature = "host")]
pub fn native(error: vst3_host::Error) -> Error {
    Error::new("RealtimeFault", error)
}
