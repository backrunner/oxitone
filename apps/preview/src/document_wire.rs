//! Document protocol 2 is independent of engine snapshot protocol 1.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceNote {
    pub pitch: u8,
    pub start: f64,
    pub duration: f64,
    pub velocity: f64,
}
#[derive(Clone, Debug, Deserialize)]
pub struct SourceOutput {
    pub note: SourceNote,
    pub select: Value,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternSite {
    pub handle: String,
    pub pattern_id: String,
    pub file_name: String,
    pub expression: String,
    pub label: String,
    pub scope: String,
    pub references: usize,
    pub placements: Vec<String>,
    pub outputs: Vec<SourceOutput>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSite {
    pub handle: String,
    pub expression: String,
    pub scope: String,
    pub lanes: Vec<String>,
    /// Playlist clip identities are consumed by timeline selection code as it
    /// moves from lane-wide to clip-local editing.
    #[allow(dead_code)]
    pub clips: Vec<String>,
    pub source: oxitone_core::wire::AutomationSourceSpec,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    pub beat: f64,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<oxitone_core::wire::Curve>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRangeEdit {
    pub start: f64,
    pub end: f64,
    pub points: Vec<AutomationPoint>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct DocumentError {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceConflict {
    pub path: String,
    pub disk: String,
    pub disk_hash: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterializationReview {
    pub plan_id: String,
    pub file_name: String,
    pub before_text: String,
    pub after_text: String,
    pub affected_clips: Vec<String>,
    pub before_notes: usize,
    pub after_notes: usize,
    pub retains_original_evaluation: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentView {
    pub arrangement_order: Option<crate::playlist_edit::Order>,
    pub session_id: String,
    pub revision: u64,
    pub accepted_revision: i64,
    pub saved_revision: i64,
    pub status: String,
    pub modified: bool,
    pub saving: bool,
    pub sites: Vec<PatternSite>,
    pub automation_sites: Vec<AutomationSite>,
    pub configuration_sites: Vec<crate::configuration_wire::ConfigurationSite>,
    pub rack_sites: Vec<crate::configuration_wire::RackSite>,
    pub rack_materialization: Option<crate::configuration_wire::RackMaterializationReview>,
    pub files: Vec<SourceFile>,
    pub conflicts: Vec<SourceConflict>,
    pub materialization: Option<MaterializationReview>,
    pub plugins: Vec<crate::plugin_manager::CatalogEntry>,
    pub diagnostic: Option<DocumentError>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DocumentMessage {
    Event {
        document_protocol_version: String,
        view: DocumentView,
    },
    Response {
        document_protocol_version: String,
        session_id: String,
        request_id: String,
        accepted: bool,
        revision: u64,
        error: Option<DocumentError>,
    },
}
impl DocumentMessage {
    pub fn validate(&self) -> Result<(), oxitone_core::OxitoneError> {
        let version = match self {
            Self::Event {
                document_protocol_version,
                view,
            } => {
                if view.sites.len() > 4096
                    || view.automation_sites.len() > 4096
                    || view.configuration_sites.len() > 4096
                    || view.rack_sites.len() > 4096
                    || view.files.len() > 4096
                    || view.sites.iter().any(|site| site.outputs.len() > 100_000)
                {
                    return Err(crate::wire::invalid("document projection exceeds budget"));
                }
                document_protocol_version
            }
            Self::Response {
                document_protocol_version,
                ..
            } => document_protocol_version,
        };
        if version != "2.0" {
            return Err(crate::wire::invalid(
                "unsupported document protocol version",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DocumentOperation {
    AssignPlugin {
        plugin: String,
        owner: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        instance: Option<String>,
    },
    Project {
        edit: crate::project_edit::ProjectEdit,
    },
    Arrangement {
        edit: crate::playlist_edit::ArrangementEdit,
    },
    EffectOrder {
        site: String,
        owner: String,
        order: Vec<String>,
    },
    PlanMaterializeRack {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
    },
    Configuration {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<String>,
        edit: crate::configuration_wire::ConfigurationEdit,
    },
    AutomationRange {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        lane: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        clip: Option<String>,
        edit: AutomationRangeEdit,
    },
    Notes {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        placement: Option<String>,
        edits: Vec<Value>,
    },
    PlanMaterialize {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        placement: Option<String>,
        edits: Vec<Value>,
    },
    ConfirmMaterialize {
        plan_id: String,
    },
    CancelMaterialize {
        plan_id: String,
    },
    ResolveConflict {
        file_name: String,
        disk_hash: String,
        resolution: String,
    },
    Code {
        file_name: String,
        text: String,
    },
    CreateFile {
        file_name: String,
        text: String,
    },
    Undo,
    Redo,
    Save,
    Query,
    RefreshPlugins,
    VerifyPlugin {
        plugin: String,
    },
    InstallPlugin {
        package_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    UpgradePlugin {
        package_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    UninstallPlugin {
        package_name: String,
    },
    RepairPlugin {
        package_name: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRequest {
    pub document_protocol_version: String,
    pub session_id: String,
    pub request_id: String,
    pub base_revision: u64,
    pub operation: DocumentOperation,
}
