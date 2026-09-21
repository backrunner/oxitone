//! Recording projection belongs to the document; parameter arming is window-local.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Touch,
    Write,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Starting,
    Recording,
    Stopping,
    Captured,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    pub instance_id: String,
    pub mode: Mode,
    pub parameter_ids: Vec<u32>,
    pub status: Status,
    pub error: Option<crate::document_wire::DocumentError>,
}
impl Recording {
    pub fn valid(&self) -> bool {
        !self.id.is_empty()
            && self.id.len() <= 256
            && !self.instance_id.is_empty()
            && self.instance_id.len() <= 256
            && !self.parameter_ids.is_empty()
            && self.parameter_ids.len() <= 32
            && self.parameter_ids.iter().collect::<BTreeSet<_>>().len() == self.parameter_ids.len()
    }
}
#[derive(Default)]
pub struct Selection {
    pub expanded: bool,
    pub mode: Mode,
    pub parameters: BTreeSet<u32>,
    pub page: usize,
}
