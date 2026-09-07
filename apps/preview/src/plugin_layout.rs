//! Versioned, bounded native panel descriptions. No executable UI or DSP handles.
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layout {
    pub ui_version: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub title: String,
    pub size: PanelSize,
    pub pages: Vec<Page>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelSize {
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub groups: Vec<Group>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub title: String,
    pub columns: u32,
    pub controls: Vec<Control>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Control {
    Knob {
        parameter: String,
        label: Option<String>,
    },
    Fader {
        parameter: String,
        label: Option<String>,
    },
    Toggle {
        parameter: String,
        label: Option<String>,
    },
    Readout {
        parameter: String,
        label: Option<String>,
    },
    Choice {
        parameter: String,
        label: Option<String>,
        options: Vec<Choice>,
    },
    Envelope {
        label: Option<String>,
        attack: String,
        decay: String,
        sustain: String,
        release: String,
    },
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: f64,
    pub label: String,
}
impl Control {
    pub fn bindings(&self) -> Vec<&str> {
        match self {
            Self::Knob { parameter, .. }
            | Self::Fader { parameter, .. }
            | Self::Toggle { parameter, .. }
            | Self::Readout { parameter, .. }
            | Self::Choice { parameter, .. } => vec![parameter],
            Self::Envelope {
                attack,
                decay,
                sustain,
                release,
                ..
            } => vec![attack, decay, sustain, release],
        }
    }
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::Knob { label, .. }
            | Self::Fader { label, .. }
            | Self::Toggle { label, .. }
            | Self::Readout { label, .. }
            | Self::Choice { label, .. }
            | Self::Envelope { label, .. } => label.as_deref(),
        }
    }
}
