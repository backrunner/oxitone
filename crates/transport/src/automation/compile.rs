//! AST flattening and validation: walks the wire spec once, enforcing the
//! depth 64 / node 256 budget and every per-node domain rule
//! (07-automation-spec.md §6), and emits post-order `Node`s (children before
//! parents) so evaluation never touches the boxed AST.

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{AutomationSourceSpec, UnaryOp};

use super::chance::ChanceNode;
use super::curve::CurveData;
use super::eval::Node;
use super::{MAX_DEPTH, MAX_NODES};

pub(crate) struct Compiler<'a> {
    pub nodes: &'a mut Vec<Node>,
    pub project_seed: u64,
}

impl Compiler<'_> {
    /// Flatten `spec` post-order (children first) and return the node index.
    /// `err_path` is the JSON path used in errors; `source_path` is the
    /// stable child-index path (root `0`) used for chance seeding.
    pub fn push(
        &mut self,
        spec: &AutomationSourceSpec,
        depth: usize,
        err_path: String,
        source_path: String,
    ) -> Result<usize, OxitoneError> {
        let path = &err_path;
        if depth > MAX_DEPTH {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_DEPTH_LIMIT,
                format!("automation AST depth exceeds {MAX_DEPTH}"),
                err_path,
            ));
        }
        if self.nodes.len() >= MAX_NODES {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_NODE_LIMIT,
                format!("automation AST node count exceeds {MAX_NODES}"),
                err_path,
            ));
        }
        let node = match spec {
            AutomationSourceSpec::Constant { value } => {
                check_finite(*value, &format!("{path}.value"))?;
                Node::Constant(*value)
            }
            AutomationSourceSpec::Curve {
                interpolation,
                points,
            } => Node::Curve(CurveData::compile(*interpolation, points, path)?),
            AutomationSourceSpec::Gate {
                period_beats,
                duty,
                phase,
                on,
                off,
            } => {
                check_positive(period_beats.to_f64(), &format!("{path}.periodBeats"))?;
                check_unit(*duty, &format!("{path}.duty"))?;
                let on = on.unwrap_or(1.0);
                let off = off.unwrap_or(0.0);
                check_unit(on, &format!("{path}.on"))?;
                check_unit(off, &format!("{path}.off"))?;
                Node::Gate {
                    period: period_beats.to_f64(),
                    duty: *duty,
                    phase: phase.map_or(0.0, |beat| beat.to_f64()),
                    on,
                    off,
                }
            }
            AutomationSourceSpec::Wave {
                wave,
                period_beats,
                phase,
                min,
                max,
                pulse_width,
            } => {
                check_positive(period_beats.to_f64(), &format!("{path}.periodBeats"))?;
                let min = min.unwrap_or(0.0);
                let max = max.unwrap_or(1.0);
                let pulse_width = pulse_width.unwrap_or(0.5);
                check_unit(min, &format!("{path}.min"))?;
                check_unit(max, &format!("{path}.max"))?;
                check_unit(pulse_width, &format!("{path}.pulseWidth"))?;
                if min > max {
                    return Err(OxitoneError::with_path(
                        codes::AUTOMATION_RANGE,
                        format!("wave min must be <= max, got {min} > {max}"),
                        format!("{path}.min"),
                    ));
                }
                Node::Wave {
                    wave: *wave,
                    period: period_beats.to_f64(),
                    phase: phase.map_or(0.0, |beat| beat.to_f64()),
                    min,
                    max,
                    pulse_width,
                }
            }
            AutomationSourceSpec::Chance(spec) => Node::Chance(ChanceNode::compile(
                spec,
                self.project_seed,
                &source_path,
                path,
            )?),
            AutomationSourceSpec::Map { input, min, max } => {
                check_unit(*min, &format!("{path}.min"))?;
                check_unit(*max, &format!("{path}.max"))?;
                if min > max {
                    return Err(OxitoneError::with_path(
                        codes::AUTOMATION_RANGE,
                        "map min must be <= max",
                        format!("{path}.min"),
                    ));
                }
                let input = self.push(
                    input,
                    depth + 1,
                    format!("{path}.input"),
                    format!("{source_path}.0"),
                )?;
                Node::Map {
                    input,
                    min: *min,
                    max: *max,
                }
            }
            AutomationSourceSpec::Unary {
                op,
                input,
                steps,
                amount,
                min,
                max,
            } => {
                let steps = steps.unwrap_or(2);
                let amount = amount.unwrap_or(match op {
                    UnaryOp::Scale => 1.0,
                    _ => 0.0,
                });
                let min = min.unwrap_or(0.0);
                let max = max.unwrap_or(1.0);
                if *op == UnaryOp::Quantize && steps < 2 {
                    return Err(OxitoneError::with_path(
                        codes::AUTOMATION_RANGE,
                        "quantize steps must be an integer > 1",
                        format!("{path}.steps"),
                    ));
                }
                check_finite(amount, &format!("{path}.amount"))?;
                check_unit(min, &format!("{path}.min"))?;
                check_unit(max, &format!("{path}.max"))?;
                let input = self.push(
                    input,
                    depth + 1,
                    format!("{path}.input"),
                    format!("{source_path}.0"),
                )?;
                Node::Unary {
                    op: *op,
                    input,
                    steps,
                    amount,
                    min,
                    max,
                }
            }
            AutomationSourceSpec::Binary {
                op,
                left,
                right,
                amount,
            } => {
                let amount = amount.unwrap_or(0.5);
                check_unit(amount, &format!("{path}.amount"))?;
                let left = self.push(
                    left,
                    depth + 1,
                    format!("{path}.left"),
                    format!("{source_path}.0"),
                )?;
                let right = self.push(
                    right,
                    depth + 1,
                    format!("{path}.right"),
                    format!("{source_path}.1"),
                )?;
                Node::Binary {
                    op: *op,
                    left,
                    right,
                    amount,
                }
            }
        };
        self.nodes.push(node);
        Ok(self.nodes.len() - 1)
    }
}

fn check_finite(value: f64, path: &str) -> Result<(), OxitoneError> {
    if !value.is_finite() {
        return Err(OxitoneError::with_path(
            codes::AUTOMATION_NON_FINITE,
            format!("value must be finite, got {value}"),
            path,
        ));
    }
    Ok(())
}

fn check_unit(value: f64, path: &str) -> Result<(), OxitoneError> {
    check_finite(value, path)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(OxitoneError::with_path(
            codes::AUTOMATION_RANGE,
            format!("value must be in 0..1, got {value}"),
            path,
        ));
    }
    Ok(())
}

fn check_positive(value: f64, path: &str) -> Result<(), OxitoneError> {
    check_finite(value, path)?;
    if value <= 0.0 {
        return Err(OxitoneError::with_path(
            codes::AUTOMATION_PERIOD,
            format!("period/duration must be > 0, got {value}"),
            path,
        ));
    }
    Ok(())
}
