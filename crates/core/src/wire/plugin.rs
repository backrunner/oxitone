//! Explicit plugin registration, mirrored by packages/protocol/src/plugin.ts.
use super::ParameterSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManifest {
    pub plugin_id: String,
    pub plugin_version: String,
    pub abi_major: u32,
    pub abi_minor: u32,
    pub min_host_version: String,
    pub kind: String,
    pub input_layout: String,
    pub output_layout: String,
    pub parameters: Vec<ParameterSpec>,
    pub sidechain_input: bool,
    pub reports_tail: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_polyphony: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterPluginOptions {
    pub library_path: String,
    pub expected_hash: Option<String>,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredPlugin {
    pub plugin_id: String,
    pub plugin_version: String,
    pub sha256: String,
}
