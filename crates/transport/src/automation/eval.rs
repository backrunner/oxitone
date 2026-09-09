//! Compiled automation evaluator: the wire AST is flattened into a fixed
//! `Vec<Node>` (children before parents) at compile time, so evaluation is
//! recursion-bounded, allocation-free, and lock-free. All periodic functions
//! use `u = euclid_mod(t - phase, period) / period` with left-closed/
//! right-open intervals (07-automation-spec.md §1-§4).

use oxitone_core::wire::{BinaryOp, UnaryOp, WaveKind};

use super::chance::{ChanceNode, EvalContext};
use super::curve::CurveData;

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Range(super::range::RangeNode),
    Constant(f64),
    Curve(CurveData),
    Gate {
        period: f64,
        duty: f64,
        phase: f64,
        on: f64,
        off: f64,
    },
    Wave {
        wave: WaveKind,
        period: f64,
        phase: f64,
        min: f64,
        max: f64,
        pulse_width: f64,
    },
    Chance(ChanceNode),
    Map {
        input: usize,
        min: f64,
        max: f64,
    },
    Unary {
        op: UnaryOp,
        input: usize,
        steps: u32,
        amount: f64,
        min: f64,
        max: f64,
    },
    Binary {
        op: BinaryOp,
        left: usize,
        right: usize,
        amount: f64,
    },
}

impl Node {
    pub(crate) fn value_at(&self, nodes: &[Node], t: f64, ctx: &EvalContext) -> f64 {
        match self {
            Node::Range(range) => range.value_at(nodes, t, ctx),
            Node::Constant(value) => *value,
            Node::Curve(curve) => curve.value_at(t),
            Node::Gate {
                period,
                duty,
                phase,
                on,
                off,
            } => {
                let u = (t - phase).rem_euclid(*period) / period;
                if u < *duty {
                    *on
                } else {
                    *off
                }
            }
            Node::Wave {
                wave,
                period,
                phase,
                min,
                max,
                pulse_width,
            } => {
                let u = (t - phase).rem_euclid(*period) / period;
                let raw = match wave {
                    WaveKind::Sine => 0.5 + 0.5 * (2.0 * std::f64::consts::PI * u).sin(),
                    WaveKind::Cos => 0.5 + 0.5 * (2.0 * std::f64::consts::PI * u).cos(),
                    WaveKind::Triangle => 1.0 - (2.0 * u - 1.0).abs(),
                    WaveKind::Saw => u,
                    WaveKind::Ramp => 1.0 - u,
                    WaveKind::Square => {
                        if u < *pulse_width {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };
                min + raw * (max - min)
            }
            Node::Chance(chance) => chance.value_at(t, ctx),
            Node::Map { input, min, max } => {
                let x = nodes[*input].value_at(nodes, t, ctx);
                min + x * (max - min)
            }
            Node::Unary {
                op,
                input,
                steps,
                amount,
                min,
                max,
            } => {
                let x = nodes[*input].value_at(nodes, t, ctx);
                match op {
                    UnaryOp::Clamp => x.max(*min).min(*max),
                    UnaryOp::Invert => 1.0 - x,
                    UnaryOp::Quantize => {
                        let levels = f64::from(*steps) - 1.0;
                        (x * levels).round() / levels
                    }
                    UnaryOp::Scale => x * amount,
                    UnaryOp::Offset => x + amount,
                }
            }
            Node::Binary {
                op,
                left,
                right,
                amount,
            } => {
                let a = nodes[*left].value_at(nodes, t, ctx);
                let b = nodes[*right].value_at(nodes, t, ctx);
                match op {
                    BinaryOp::Mix => (1.0 - amount) * a + amount * b,
                    BinaryOp::Add => a + b,
                    BinaryOp::Multiply => a * b,
                    BinaryOp::Min => a.min(b),
                    BinaryOp::Max => a.max(b),
                }
            }
        }
    }

    pub(crate) fn has_edge(&self, nodes: &[Node], start: f64, end: f64, ctx: &EvalContext) -> bool {
        let periodic = |period: f64, phase: f64| {
            ((start - phase) / period).floor() != ((end - phase) / period).floor()
        };
        match self {
            Node::Range(range) => range.has_edge(nodes, start, end, ctx),
            Node::Gate {
                period,
                phase,
                duty,
                ..
            } => periodic(*period, *phase) || periodic(*period, phase + duty * period),
            Node::Wave {
                wave: WaveKind::Square,
                period,
                phase,
                pulse_width,
                ..
            } => periodic(*period, *phase) || periodic(*period, phase + pulse_width * period),
            Node::Curve(curve) => curve.has_edge(start, end),
            Node::Chance(chance) => chance.has_edge(start, end, ctx),
            Node::Map { input, .. } | Node::Unary { input, .. } => {
                nodes[*input].has_edge(nodes, start, end, ctx)
            }
            Node::Binary { left, right, .. } => {
                nodes[*left].has_edge(nodes, start, end, ctx)
                    || nodes[*right].has_edge(nodes, start, end, ctx)
            }
            _ => false,
        }
    }

    pub(crate) fn discontinuities(
        &self,
        nodes: &[Node],
        start: f64,
        end: f64,
        ctx: &EvalContext,
        out: &mut Vec<f64>,
    ) {
        match self {
            Node::Range(range) => range.discontinuities(nodes, start, end, ctx, out),
            Node::Constant(_) => {}
            Node::Wave {
                wave,
                period,
                phase,
                pulse_width,
                ..
            } => {
                if matches!(wave, WaveKind::Square) {
                    push_periodic_edges(out, start, end, *period, *phase, *pulse_width);
                }
            }
            Node::Gate {
                period,
                duty,
                phase,
                ..
            } => push_periodic_edges(out, start, end, *period, *phase, *duty),
            Node::Curve(curve) => curve.discontinuities(start, end, out),
            Node::Chance(chance) => chance.discontinuities(start, end, ctx, out),
            Node::Map { input, .. } => nodes[*input].discontinuities(nodes, start, end, ctx, out),
            Node::Unary { input, .. } => nodes[*input].discontinuities(nodes, start, end, ctx, out),
            Node::Binary { left, right, .. } => {
                nodes[*left].discontinuities(nodes, start, end, ctx, out);
                nodes[*right].discontinuities(nodes, start, end, ctx, out);
            }
        }
    }
}

/// Period starts and pulse edges of a gate/square inside `[start, end)`.
fn push_periodic_edges(
    out: &mut Vec<f64>,
    start: f64,
    end: f64,
    period: f64,
    phase: f64,
    width: f64,
) {
    let first = ((start - phase) / period).ceil() as i64;
    let last = ((end - phase) / period).ceil() as i64;
    for k in first..=last {
        for edge in [
            phase + k as f64 * period,
            phase + (k as f64 + width) * period,
        ] {
            if edge >= start && edge < end {
                out.push(edge);
            }
        }
    }
}
