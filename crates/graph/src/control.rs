//! Control-only native capabilities. No calls through this interface belong to a DSP callback.
use oxitone_core::OxitoneError;
use std::time::Duration;

pub trait NativeControl: Send + Sync {
    /// Protocol implemented by the adapter (currently `vst3`, stream control 1).
    fn protocol(&self) -> &'static str;
    /// Strictly validate and execute a versioned request on the native control side.
    fn request(&self, json: &str, timeout: Duration) -> Result<String, OxitoneError>;
}
