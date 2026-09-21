//! Session-only offline form. Source-backed instruments remain separate.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "vst3_model_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub bundle_path: String,
    pub class_id: String,
    pub input_channels: Option<usize>,
    pub output_channels: Option<usize>,
    pub note_input: Option<bool>,
    pub parameters: Vec<Parameter>,
    pub render: Option<Report>,
    pub preset_path: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub id: u32,
    pub name: String,
    pub value: f64,
    pub default: f64,
    pub step_count: i32,
    pub can_automate: bool,
    pub read_only: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Report {
    pub path: String,
    pub peak: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Command {
    Scan,
    Add {
        source: Source,
    },
    AddBundle {
        #[serde(rename = "bundlePath")]
        bundle_path: String,
    },
    Remove {
        plugin: String,
    },
    Edit {
        plugin: String,
        parameters: BTreeMap<String, f64>,
    },
    ControlInstance {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<String>,
        action: LiveAction,
    },
    CaptureInstance {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<String>,
    },
    StartRecording {
        site: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<String>,
        mode: crate::vst3_recording::Mode,
        #[serde(rename = "parameterIds")]
        parameter_ids: Vec<u32>,
    },
    StopRecording {
        #[serde(rename = "recordingId")]
        recording_id: String,
    },
    CancelRecording {
        #[serde(rename = "recordingId")]
        recording_id: String,
    },
    Render {
        plugin: String,
        options: RenderOptions,
    },
    Cancel,
    LoadPreset {
        path: String,
    },
    SavePreset {
        plugin: String,
        path: String,
        parameters: BTreeMap<String, f64>,
    },
    AttachRender {
        plugin: String,
        name: String,
        #[serde(rename = "startBeat")]
        start_beat: f64,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveAction {
    OpenEditor,
    CloseEditor,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub bundle_path: String,
    pub class_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_path: Option<String>,
    pub frames: u64,
    pub tail_frames: u64,
    pub sample_rate: u32,
    pub block_size: usize,
    pub tempo: f64,
    pub time_signature: [u8; 2],
    pub parameters: BTreeMap<String, f64>,
    pub events: Vec<serde_json::Value>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Bundle,
    Class,
    Input,
    Output,
    Seconds,
    Tail,
    Tempo,
    Pitch,
    Preset,
    TrackName,
    StartBeat,
    Parameter(u32),
}
#[derive(Clone)]
pub struct Draft {
    pub input: String,
    pub output: String,
    pub seconds: String,
    pub tail: String,
    pub tempo: String,
    pub pitch: String,
    pub parameters: BTreeMap<String, f64>,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            input: String::new(),
            output: String::new(),
            seconds: "1".into(),
            tail: "0.1".into(),
            tempo: "120".into(),
            pitch: "60".into(),
            parameters: BTreeMap::new(),
        }
    }
}
#[derive(Default)]
pub struct WorkbenchUi {
    pub adding: bool,
    pub files: bool,
    pub preset: String,
    pub track_name: String,
    pub start_beat: String,
    pub bundle: String,
    pub class_id: String,
    pub drafts: BTreeMap<String, Draft>,
    pub input: Option<(Field, String)>,
    pub select_all: bool,
    pub page: usize,
    pub error: Option<String>,
    pub bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String, gpui::Bounds<gpui::Pixels>>>>,
}
impl Draft {
    pub fn options(&self, catalog: &Catalog) -> Result<RenderOptions, String> {
        fn number(text: &str, min: f64, max: f64, label: &str) -> Result<f64, String> {
            text.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v >= min && *v <= max)
                .ok_or_else(|| format!("{label} must be between {min} and {max}"))
        }
        if self.output.trim().is_empty() {
            return Err("Choose a new output WAV path".into());
        }
        if catalog.input_channels.unwrap_or(0) > 0 && self.input.trim().is_empty() {
            return Err("Choose an input WAV for this effect".into());
        }
        let frames = (number(&self.seconds, 0.001, 600., "Duration")? * 48000.).round() as u64;
        let events = if catalog.note_input == Some(true) {
            let pitch = number(&self.pitch, 0., 127., "Note")?;
            if pitch.fract() != 0. {
                return Err("MIDI note must be an integer".into());
            }
            vec![
                serde_json::json!({"type":"noteOn","frame":0,"channel":0,"pitch":pitch as u8,"velocity":0.8}),
                serde_json::json!({"type":"noteOff","frame":frames-1,"channel":0,"pitch":pitch as u8,"velocity":0.0}),
            ]
        } else {
            vec![]
        };
        Ok(RenderOptions {
            path: self.output.clone(),
            input_path: (!self.input.trim().is_empty()).then(|| self.input.clone()),
            frames,
            tail_frames: (number(&self.tail, 0., 60., "Tail")? * 48000.).round() as u64,
            sample_rate: 48000,
            block_size: 128,
            tempo: number(&self.tempo, 20., 999., "Tempo")?,
            time_signature: [4, 4],
            parameters: self.parameters.clone(),
            events,
        })
    }
}
