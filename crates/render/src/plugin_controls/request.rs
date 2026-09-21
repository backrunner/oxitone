//! Instance control 1 wire validation shared by N-API and the native Preview backend.
use oxitone_core::OxitoneError;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub instance_control_version: u32,
    pub graph_generation: String,
    pub instance_id: String,
    pub command: serde_json::Value,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}
fn default_timeout() -> u64 {
    5000
}
impl Request {
    pub fn decode(json: &str) -> Result<Self, OxitoneError> {
        let invalid = |message: String| OxitoneError::new("PluginConfigInvalid", message);
        if json.len() > 16_384 {
            return Err(invalid("VST3 instance command exceeds 16 KiB".into()));
        }
        let request: Self = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        if request.instance_control_version != 1 {
            return Err(OxitoneError::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 instance control version",
            ));
        }
        let generation = request.graph_generation.parse::<u64>().ok();
        if !generation.is_some_and(|n| n > 0 && n.to_string() == request.graph_generation)
            || request.instance_id.is_empty()
            || request.instance_id.len() > 128
            || !request
                .instance_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            || !(1..=600_000).contains(&request.timeout_ms)
        {
            return Err(invalid("invalid VST3 instance target or timeout".into()));
        }
        Ok(request)
    }
    pub fn execute(
        &self,
        registry: &super::ControlRegistry,
    ) -> Result<serde_json::Value, OxitoneError> {
        let state = registry.request(
            &self.graph_generation,
            &self.instance_id,
            "vst3",
            &serde_json::json!({"controlProtocolVersion":1,"command":self.command}).to_string(),
            Duration::from_millis(self.timeout_ms),
        )?;
        let state: serde_json::Value = serde_json::from_str(&state)
            .map_err(|e| OxitoneError::new("RealtimeFault", e.to_string()))?;
        Ok(
            serde_json::json!({"instanceControlVersion":1,"graphGeneration":self.graph_generation,
            "instanceId":self.instance_id,"state":state}),
        )
    }
}
