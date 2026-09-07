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
    Oscillator {
        label: Option<String>,
        wave: String,
        #[serde(rename = "morphTo")]
        morph_to: String,
        position: String,
        phase: String,
        unison: String,
        detune: String,
        spread: String,
        bank: Option<String>,
        #[serde(rename = "warpMode")]
        warp_mode: Option<String>,
        warp: Option<String>,
        octave: Option<String>,
    },
    SubOscillator {
        label: Option<String>,
        wave: String,
        octave: String,
        level: String,
    },
    FilterResponse {
        label: Option<String>,
        mode: String,
        cutoff: String,
        resonance: String,
    },
    LfoCurve {
        label: Option<String>,
        shape: String,
        rate: String,
        phase: String,
    },
    Modulation {
        label: Option<String>,
        routes: Vec<ModulationRoute>,
    },
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulationRoute {
    pub label: String,
    pub amount: String,
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
            Self::Oscillator {
                wave,
                morph_to,
                position,
                phase,
                unison,
                detune,
                spread,
                bank,
                warp_mode,
                warp,
                octave,
                ..
            } => {
                let mut ids = vec![
                    wave.as_str(),
                    morph_to,
                    position,
                    phase,
                    unison,
                    detune,
                    spread,
                ];
                ids.extend(
                    [bank, warp_mode, warp, octave]
                        .into_iter()
                        .filter_map(|v| v.as_deref()),
                );
                ids
            }
            Self::SubOscillator {
                wave,
                octave,
                level,
                ..
            } => vec![wave, octave, level],
            Self::FilterResponse {
                mode,
                cutoff,
                resonance,
                ..
            } => vec![mode, cutoff, resonance],
            Self::LfoCurve {
                shape, rate, phase, ..
            } => vec![shape, rate, phase],
            Self::Modulation { routes, .. } => routes.iter().map(|r| r.amount.as_str()).collect(),
        }
    }
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::Knob { label, .. }
            | Self::Fader { label, .. }
            | Self::Toggle { label, .. }
            | Self::Readout { label, .. }
            | Self::Choice { label, .. }
            | Self::Envelope { label, .. }
            | Self::Oscillator { label, .. }
            | Self::SubOscillator { label, .. }
            | Self::FilterResponse { label, .. }
            | Self::LfoCurve { label, .. }
            | Self::Modulation { label, .. } => label.as_deref(),
        }
    }
    pub fn visual(&self) -> bool {
        matches!(
            self,
            Self::Envelope { .. }
                | Self::Oscillator { .. }
                | Self::SubOscillator { .. }
                | Self::FilterResponse { .. }
                | Self::LfoCurve { .. }
                | Self::Modulation { .. }
        )
    }
}
