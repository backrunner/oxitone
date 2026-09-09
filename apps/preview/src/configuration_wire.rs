use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub struct ConfigurationUsage {
    pub handle: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationValue {
    pub plugin_id: String,
    pub plugin_version: String,
    pub parameters: BTreeMap<String, f64>,
    pub mix: Option<f64>,
    pub bypass: Option<bool>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ConfigurationSite {
    pub handle: String,
    pub scope: String,
    pub kind: String,
    pub usages: Vec<ConfigurationUsage>,
    pub config: ConfigurationValue,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConfigurationEdit {
    Parameters { values: BTreeMap<String, f64> },
    Host { values: HostEdit },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass: Option<bool>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct RackUsage {
    pub owner: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct RackSite {
    pub handle: String,
    pub scope: String,
    pub usages: Vec<RackUsage>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RackMaterializationReview {
    pub plan_id: String,
    pub file_name: String,
    pub before_text: String,
    pub after_text: String,
    pub affected_owners: Vec<String>,
    pub effects: usize,
    pub retains_original_evaluation: bool,
}
