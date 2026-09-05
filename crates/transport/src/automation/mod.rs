//! Automation source compilation and evaluation (07-automation-spec.md).
//!
//! The wire AST (`oxitone_core::wire::AutomationSourceSpec`) is validated and
//! flattened once into a fixed evaluator. Evaluation is deterministic,
//! allocation-free, and lock-free: periodic phases derive from the absolute
//! beat (never accumulated f32 phase), chance decisions are hash-seeded O(1)
//! jumpable queries, and lane outputs are clamped to `[0, 1]` exactly once.

pub mod bake;
mod chance;
mod compile;
mod curve;
mod eval;

use oxitone_core::error::OxitoneError;
use oxitone_core::wire::AutomationSourceSpec;

use crate::tempo::CompiledTempoMap;

pub use bake::{
    bake_tempo_lane, ensure_transport_invariant, find_tempo_lane, normalized_to_bpm,
    TEMPO_BAKE_GRID_BEAT, TEMPO_BAKE_MAX_SEGMENTS,
};
pub use chance::EvalContext;

/// Maximum automation AST depth (07-automation-spec.md §4).
pub const MAX_DEPTH: usize = 64;
/// Maximum automation AST node count (07-automation-spec.md §4).
pub const MAX_NODES: usize = 256;

/// Fixed evaluator for one automation source. Owns every per-node resource
/// (curve tables, chance seed prefixes) allocated at compile time; the
/// evaluation methods perform no allocation.
#[derive(Debug, Clone)]
pub struct CompiledAutomation {
    nodes: Vec<eval::Node>,
    root: usize,
}

impl CompiledAutomation {
    /// Validate `spec` (07-automation-spec.md §6) and compile it into a fixed
    /// evaluator. `project_seed` is baked into chance seed prefixes; the
    /// remaining transport state arrives per call through [`EvalContext`].
    pub fn compile(spec: &AutomationSourceSpec, project_seed: u64) -> Result<Self, OxitoneError> {
        let mut nodes = Vec::new();
        let mut compiler = compile::Compiler {
            nodes: &mut nodes,
            project_seed,
        };
        let root = compiler.push(spec, 1, "$.source".to_owned(), "0".to_owned())?;
        Ok(Self { nodes, root })
    }

    /// Raw (unclamped) source value at a Project beat.
    fn raw_value_at(&self, beat: f64, ctx: &EvalContext) -> f64 {
        self.nodes[self.root].value_at(&self.nodes, beat, ctx)
    }

    /// Lane output at a Project beat: finite and clamped to `[0, 1]`
    /// (07-automation-spec.md §1). Non-finite intermediate results map to 0.
    pub fn value_at(&self, beat: f64, ctx: &EvalContext) -> f64 {
        let value = self.raw_value_at(beat, ctx);
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Audio-rate block evaluation: every output frame maps its absolute
    /// sample frame back to a beat through the compiled tempo map — no f32
    /// phase accumulation (03-audio-runtime-spec.md §时间和调度).
    pub fn fill_segment(
        &self,
        frame_start: u64,
        frames: usize,
        tempo: &CompiledTempoMap,
        ctx: &EvalContext,
        out: &mut [f32],
    ) {
        assert!(out.len() >= frames, "output slice smaller than frame count");
        for (index, slot) in out.iter_mut().enumerate().take(frames) {
            let beat = tempo.frame_to_beat(frame_start + index as u64).to_f64();
            *slot = self.value_at(beat, ctx) as f32;
        }
    }

    /// Control-rate evaluation at a sample frame (one value per block start).
    pub fn value_at_frame(&self, frame: u64, tempo: &CompiledTempoMap, ctx: &EvalContext) -> f32 {
        self.value_at(tempo.frame_to_beat(frame).to_f64(), ctx) as f32
    }

    /// Whether an edge occurs in (start, end]; bounded, allocation-free.
    pub fn has_edge(&self, start: f64, end: f64, ctx: &EvalContext) -> bool {
        self.nodes[self.root].has_edge(&self.nodes, start, end, ctx)
    }

    /// Sorted discontinuity beats (gate/square edges, curve control points,
    /// chance decision points) inside `[start, end)`. The compiler splits
    /// segments at these beats so changes land on exact sample frames.
    pub fn discontinuities(&self, start: f64, end: f64, ctx: &EvalContext) -> Vec<f64> {
        let mut out = Vec::new();
        self.nodes[self.root].discontinuities(&self.nodes, start, end, ctx, &mut out);
        out.sort_by(f64::total_cmp);
        out.dedup();
        out
    }
}
