//! `chance` sources: deterministic sample-and-hold on a beat grid with
//! hash-seeded O(1) jumpable decisions (07-automation-spec.md §3). No PRNG
//! state is advanced sequentially, so seek/loop queries are order-independent.
//!
//! Allocation-free hot path: the `hash64-v1` canonical input
//! (`n:<u64>|...|s:<path>`) is materialized once at compile time as a byte
//! prefix; per-decision draws append `|n:<index>` into a stack buffer and run
//! SHA-256 without any heap traffic.

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::pcg32::{hash64, Hash64Part, Pcg32};
use oxitone_core::wire::{ChanceSpec, RandomPhase};
use sha2::{Digest, Sha256};

/// Evaluation context for transport-dependent sources. Only
/// `randomPhase: 'restart'` chance nodes read these fields; absolute sources
/// anchor to Project beat 0 and ignore the loop iteration.
#[derive(Debug, Clone, Copy, Default)]
pub struct EvalContext {
    pub origin_beat: f64,
    pub loop_iteration: u64,
}

/// hash64-v1 over pre-encoded canonical bytes (allocation-free).
fn hash64_bytes(bytes: &[u8]) -> u64 {
    let digest = Sha256::digest(bytes);
    u64::from_be_bytes(digest[..8].try_into().expect("sha256 digest is 32 bytes"))
}

/// Append the decimal form of `value` to `buf`; returns the new length.
fn push_u64_dec(buf: &mut [u8; 160], mut len: usize, value: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut count = 0;
    let mut v = value;
    loop {
        digits[count] = b'0' + (v % 10) as u8;
        v /= 10;
        count += 1;
        if v == 0 {
            break;
        }
    }
    for index in 0..count {
        buf[len] = digits[count - 1 - index];
        len += 1;
    }
    len
}

fn push_tag(buf: &mut [u8; 160], mut len: usize, tag: &[u8]) -> usize {
    buf[len..len + tag.len()].copy_from_slice(tag);
    len += tag.len();
    len
}

/// Immutable compiled chance node. `seed_prefix` is the canonical
/// `n:<projectSeed>|n:<laneSeed>|s:<canonicalSourcePath>` byte string; the
/// canonical source path is the stable child-index path of the node in the
/// AST (e.g. `0.1.0`), assigned at compile time — never a pointer or a
/// traversal-cache index.
#[derive(Debug, Clone)]
pub struct ChanceNode {
    probability: f64,
    interval: f64,
    smooth: f64,
    random_phase: RandomPhase,
    seed_prefix: String,
    absolute_seed: u64,
}

impl ChanceNode {
    pub fn compile(
        spec: &ChanceSpec,
        project_seed: u64,
        source_path: &str,
        path: &str,
    ) -> Result<Self, OxitoneError> {
        spec.validate()
            .map_err(|error| OxitoneError::with_path(error.code, error.message, path.to_owned()))?;
        if !spec.probability.is_finite() {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_NON_FINITE,
                "chance probability must be finite",
                format!("{path}.probability"),
            ));
        }
        if !(0.0..=1.0).contains(&spec.probability) {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_RANGE,
                format!(
                    "chance probability must be in 0..1, got {}",
                    spec.probability
                ),
                format!("{path}.probability"),
            ));
        }
        let interval = match (spec.rate, spec.interval_beats) {
            (Some(rate), None) => 1.0 / rate,
            (None, Some(beat)) => beat.to_f64(),
            _ => unreachable!("ChanceSpec::validate enforces exactly one selector"),
        };
        if !interval.is_finite() || interval <= 0.0 {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_CHANCE_FREQUENCY,
                "chance interval must be finite and positive",
                path,
            ));
        }
        let smooth = spec.smooth_beats.map_or(0.0, |beat| beat.to_f64());
        if !smooth.is_finite() || smooth < 0.0 {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_RANGE,
                format!("chance smoothBeats must be >= 0, got {smooth}"),
                format!("{path}.smoothBeats"),
            ));
        }
        let seed_prefix = format!("n:{project_seed}|n:{}|s:{source_path}", spec.seed);
        let absolute_seed = hash64(&[
            Hash64Part::Int(project_seed),
            Hash64Part::Int(spec.seed),
            Hash64Part::Str(source_path.to_owned()),
        ]);
        Ok(Self {
            probability: spec.probability,
            interval,
            smooth: smooth.min(interval),
            random_phase: spec.random_phase.unwrap_or(RandomPhase::Absolute),
            seed_prefix,
            absolute_seed,
        })
    }

    fn origin(&self, ctx: &EvalContext) -> f64 {
        match self.random_phase {
            RandomPhase::Absolute => 0.0,
            RandomPhase::Restart => ctx.origin_beat,
        }
    }

    /// `hash64(projectSeed, laneSeed, canonicalSourcePath, loopIteration?)`;
    /// the loop iteration only participates for `restart` sources.
    fn effective_seed(&self, ctx: &EvalContext) -> u64 {
        match self.random_phase {
            RandomPhase::Absolute => self.absolute_seed,
            RandomPhase::Restart => {
                let mut buf = [0u8; 160];
                let len = push_tag(&mut buf, 0, b"|n:");
                let len = push_u64_dec(&mut buf, len, ctx.loop_iteration);
                let mut hash = Sha256::new();
                hash.update(self.seed_prefix.as_bytes());
                hash.update(&buf[..len]);
                let digest = hash.finalize();
                u64::from_be_bytes(digest[..8].try_into().expect("sha256 digest"))
            }
        }
    }

    /// Decision value (0 or 1) for a grid index; O(1), jumpable, and free of
    /// allocation. probability 0/1 short-circuit without touching the PRNG.
    fn decision(&self, ctx: &EvalContext, index: i64) -> f64 {
        if self.probability <= 0.0 {
            return 0.0;
        }
        if self.probability >= 1.0 {
            return 1.0;
        }
        let mut buf = [0u8; 160];
        let mut len = push_tag(&mut buf, 0, b"n:");
        len = push_u64_dec(&mut buf, len, self.effective_seed(ctx));
        len = push_tag(&mut buf, len, b"|n:");
        len = push_u64_dec(&mut buf, len, index as u64);
        let draw_seed = hash64_bytes(&buf[..len]);
        let r = Pcg32::new(draw_seed).next_f64();
        if r < self.probability {
            1.0
        } else {
            0.0
        }
    }

    pub fn value_at(&self, t: f64, ctx: &EvalContext) -> f64 {
        let origin = self.origin(ctx);
        let index = ((t - origin) / self.interval).floor() as i64;
        if self.smooth > 0.0 {
            let dt = t - (origin + index as f64 * self.interval);
            if dt < self.smooth {
                let from = self.decision(ctx, index - 1);
                let to = self.decision(ctx, index);
                return from + (to - from) * (dt / self.smooth);
            }
        }
        self.decision(ctx, index)
    }

    pub fn has_edge(&self, start: f64, end: f64, ctx: &EvalContext) -> bool {
        let origin = self.origin(ctx);
        ((start - origin) / self.interval).floor() != ((end - origin) / self.interval).floor()
    }

    /// Decision point beats inside `[start, end)` (compile-time use).
    pub fn discontinuities(&self, start: f64, end: f64, ctx: &EvalContext, out: &mut Vec<f64>) {
        let origin = self.origin(ctx);
        let first = ((start - origin) / self.interval).ceil() as i64;
        let last = ((end - origin) / self.interval).ceil() as i64;
        for index in first..last {
            let beat = origin + index as f64 * self.interval;
            if beat >= start && beat < end {
                out.push(beat);
            }
        }
    }
}
