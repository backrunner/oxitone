//! A native interval overlay. Runtime evaluation has no allocations and only visits active branches.
use super::{eval::Node, EvalContext};

#[derive(Debug, Clone)]
pub(crate) struct RangeNode {
    pub base: usize,
    pub replacement: usize,
    pub start: f64,
    pub end: f64,
    pub fade: f64,
}
impl RangeNode {
    fn context(&self, ctx: &EvalContext) -> EvalContext {
        EvalContext {
            origin_beat: ctx.origin_beat - self.start,
            loop_iteration: ctx.loop_iteration,
        }
    }
    pub fn value_at(&self, nodes: &[Node], t: f64, ctx: &EvalContext) -> f64 {
        if t < self.start || t >= self.end {
            return nodes[self.base].value_at(nodes, t, ctx);
        }
        let weight = if self.fade == 0. {
            1.
        } else {
            ((t - self.start).min(self.end - t) / self.fade).clamp(0., 1.)
        };
        if weight <= 0. {
            return nodes[self.base].value_at(nodes, t, ctx);
        }
        let replacement =
            nodes[self.replacement].value_at(nodes, t - self.start, &self.context(ctx));
        if weight >= 1. {
            return replacement;
        }
        let base = nodes[self.base].value_at(nodes, t, ctx);
        base * (1. - weight) + replacement * weight
    }
    pub fn has_edge(&self, nodes: &[Node], start: f64, end: f64, ctx: &EvalContext) -> bool {
        if start >= end {
            return false;
        }
        if [
            self.start,
            self.end,
            self.start + self.fade,
            self.end - self.fade,
        ]
        .iter()
        .any(|edge| start < *edge && *edge <= end)
        {
            return true;
        }
        let inside = start.max(self.start) < end.min(self.end);
        (start < self.start && nodes[self.base].has_edge(nodes, start, end.min(self.start), ctx))
            || (end > self.end && nodes[self.base].has_edge(nodes, start.max(self.end), end, ctx))
            || (inside && self.fade > 0. && nodes[self.base].has_edge(nodes, start, end, ctx))
            || (inside
                && nodes[self.replacement].has_edge(
                    nodes,
                    start.max(self.start) - self.start,
                    end.min(self.end) - self.start,
                    &self.context(ctx),
                ))
    }
    pub fn discontinuities(
        &self,
        nodes: &[Node],
        start: f64,
        end: f64,
        ctx: &EvalContext,
        out: &mut Vec<f64>,
    ) {
        if start >= end {
            return;
        }
        for edge in [
            self.start,
            self.end,
            self.start + self.fade,
            self.end - self.fade,
        ] {
            if edge >= start && edge < end {
                out.push(edge);
            }
        }
        if self.fade > 0. {
            nodes[self.base].discontinuities(nodes, start, end, ctx, out);
        } else {
            if start < self.start {
                nodes[self.base].discontinuities(nodes, start, end.min(self.start), ctx, out);
            }
            if end > self.end {
                nodes[self.base].discontinuities(nodes, start.max(self.end), end, ctx, out);
            }
        }
        let (left, right) = (start.max(self.start), end.min(self.end));
        if left < right {
            let offset = out.len();
            nodes[self.replacement].discontinuities(
                nodes,
                left - self.start,
                right - self.start,
                &self.context(ctx),
                out,
            );
            for value in &mut out[offset..] {
                *value += self.start;
            }
        }
    }
}
