//! Stable error codes and the structured engine error. Codes must stay in
//! sync with `packages/protocol/src/errors.ts`; callers branch on `code`,
//! never on the human-readable message.

use thiserror::Error;

/// Stable error codes named in `.agents/docs/`.
pub mod codes {
    pub const INVALID_PROJECT: &str = "InvalidProject";
    pub const PROTOCOL_VERSION_UNSUPPORTED: &str = "ProtocolVersionUnsupported";
    pub const TEMPO_RANGE: &str = "TempoRange";
    pub const TEMPO_MAP_ORDER: &str = "TempoMapOrder";
    pub const TEMPO_MAP_COMPLEXITY: &str = "TempoMapComplexity";
    pub const TEMPO_AUTOMATION_CONFLICT: &str = "TempoAutomationConflict";
    pub const AUTOMATION_NON_FINITE: &str = "AutomationNonFinite";
    pub const AUTOMATION_RANGE: &str = "AutomationRange";
    pub const AUTOMATION_PERIOD: &str = "AutomationPeriod";
    pub const AUTOMATION_CHANCE_FREQUENCY: &str = "AutomationChanceFrequency";
    pub const AUTOMATION_POINTS: &str = "AutomationPoints";
    pub const AUTOMATION_EXPONENTIAL_ZERO: &str = "AutomationExponentialZero";
    pub const AUTOMATION_DEPTH_LIMIT: &str = "AutomationDepthLimit";
    pub const AUTOMATION_NODE_LIMIT: &str = "AutomationNodeLimit";
    pub const AUTOMATION_RATE_BUDGET: &str = "AutomationRateBudget";
    pub const AUTOMATION_TEMPO_RESTRICTION: &str = "AutomationTempoRestriction";
    pub const AUTOMATION_TARGET_INVALID: &str = "AutomationTargetInvalid";
    pub const MIDI_CHANNEL_LIMIT: &str = "MidiChannelLimit";
    pub const SAMPLE_STRETCH_RANGE: &str = "SampleStretchRange";
    pub const SAMPLE_FORMAT_UNSUPPORTED: &str = "SampleFormatUnsupported";
    pub const ASSET_UNAVAILABLE: &str = "AssetUnavailable";
    pub const DEVICE_UNAVAILABLE: &str = "DeviceUnavailable";
    pub const REALTIME_FAULT: &str = "RealtimeFault";
    pub const WAV_TOO_LARGE: &str = "WavTooLarge";
    pub const PLUGIN_ABI_MISMATCH: &str = "PluginAbiMismatch";
    pub const PLUGIN_MANIFEST_MISMATCH: &str = "PluginManifestMismatch";
    pub const PERFORMANCE_WARNING: &str = "PerformanceWarning";
}

/// Structured error carried across the N-API boundary.
#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct OxitoneError {
    pub code: &'static str,
    pub message: String,
    /// JSON path of the offending value, when known.
    pub path: Option<String>,
}

impl OxitoneError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(
        code: &'static str,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            path: Some(path.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_code_and_message() {
        let err =
            OxitoneError::with_path(codes::AUTOMATION_POINTS, "empty points", "$.source.points");
        assert_eq!(err.to_string(), "AutomationPoints: empty points");
        assert_eq!(err.path.as_deref(), Some("$.source.points"));
    }
}
