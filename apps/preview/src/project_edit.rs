//! Typed document operations; these never mutate the native render graph directly.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MixValues {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pan: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solo: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectEdit {
    EffectOrder {
        owner: String,
        index: usize,
        order: Vec<usize>,
    },
    Channel {
        index: usize,
        values: MixValues,
    },
    Bus {
        index: usize,
        values: MixValues,
    },
    Track {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mute: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        solo: Option<bool>,
    },
    Tempo {
        bpm: f64,
    },
    Instrument {
        index: usize,
        config: serde_json::Value,
    },
    Effect {
        owner: String,
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        slot: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        config: Option<serde_json::Value>,
    },
}
