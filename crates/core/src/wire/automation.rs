//! Automation source AST wire type (04-api-contracts.md,
//! 07-automation-spec.md). Recursive via `Box`; the depth 64 / node 256
//! budget is enforced by the authoring layer and the graph validator.

use serde::{Deserialize, Serialize};

use super::basic::{AutomationPoint, CurveKind};
use crate::beat::Beat;
use crate::error::{codes, OxitoneError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaveKind {
    Sine,
    Cos,
    Triangle,
    Saw,
    Ramp,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RandomPhase {
    Absolute,
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnaryOp {
    Clamp,
    Invert,
    Quantize,
    Scale,
    Offset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BinaryOp {
    Mix,
    Add,
    Multiply,
    Min,
    Max,
}

/// `chance` frequency selector: exactly one of `rate` (decisions per beat;
/// the authoring `frequency` alias normalizes to this) or `intervalBeats`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChanceSpec {
    pub probability: f64,
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smooth_beats: Option<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_phase: Option<RandomPhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_beats: Option<Beat>,
}

impl ChanceSpec {
    pub fn validate(&self) -> Result<(), OxitoneError> {
        let selected = match (self.rate, self.interval_beats) {
            (Some(rate), None) => {
                if !rate.is_finite() || rate <= 0.0 {
                    None
                } else {
                    Some(())
                }
            }
            (None, Some(_)) => Some(()),
            _ => None,
        };
        selected.ok_or_else(|| {
            OxitoneError::new(
                codes::AUTOMATION_CHANCE_FREQUENCY,
                "chance requires exactly one positive finite rate or intervalBeats",
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AutomationSourceSpec {
    Constant {
        value: f64,
    },
    Curve {
        interpolation: CurveKind,
        points: Vec<AutomationPoint>,
    },
    Gate {
        period_beats: Beat,
        duty: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<Beat>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        off: Option<f64>,
    },
    Chance(ChanceSpec),
    Wave {
        wave: WaveKind,
        period_beats: Beat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<Beat>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pulse_width: Option<f64>,
    },
    Map {
        input: Box<AutomationSourceSpec>,
        min: f64,
        max: f64,
    },
    Unary {
        op: UnaryOp,
        input: Box<AutomationSourceSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        steps: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    Binary {
        op: BinaryOp,
        left: Box<AutomationSourceSpec>,
        right: Box<AutomationSourceSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<f64>,
    },
}

impl AutomationSourceSpec {
    /// Structural checks that do not need graph context.
    pub fn validate(&self) -> Result<(), OxitoneError> {
        match self {
            AutomationSourceSpec::Chance(spec) => spec.validate(),
            AutomationSourceSpec::Map { input, .. } => input.validate(),
            AutomationSourceSpec::Unary { input, .. } => input.validate(),
            AutomationSourceSpec::Binary { left, right, .. } => {
                left.validate().and_then(|()| right.validate())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chance_requires_exactly_one_selector() {
        let base = ChanceSpec {
            probability: 0.5,
            seed: 1,
            smooth_beats: None,
            random_phase: None,
            rate: None,
            interval_beats: None,
        };
        assert!(base.validate().is_err());
        let both = ChanceSpec {
            rate: Some(2.0),
            interval_beats: Some(Beat::new(1, 2).unwrap()),
            ..base.clone()
        };
        assert_eq!(
            both.validate().unwrap_err().code,
            codes::AUTOMATION_CHANCE_FREQUENCY
        );
        let ok = ChanceSpec {
            rate: Some(2.0),
            ..base
        };
        assert!(ok.validate().is_ok());
    }
}
