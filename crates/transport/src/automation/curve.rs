//! Compiled `curve`/`polyline`/`line` sources: piecewise segments with
//! step/linear/smooth/exponential interpolation plus monotone time-axis
//! cubic Bezier solved by bounded iteration (07-automation-spec.md §2).

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{AutomationPoint, Curve, CurveKind};

/// A validated control point with an f64 beat (the rational wire value is
/// exact for scheduling; evaluation uses the f64 projection).
#[derive(Debug, Clone, Copy)]
pub struct CurvePoint {
    pub beat: f64,
    pub value: f64,
    pub curve: Option<Curve>,
}

/// Immutable compiled curve; points are non-empty and strictly increasing.
#[derive(Debug, Clone)]
pub struct CurveData {
    pub interpolation: CurveKind,
    pub points: Vec<CurvePoint>,
}

impl CurveData {
    pub fn compile(
        interpolation: CurveKind,
        points: &[AutomationPoint],
        path: &str,
    ) -> Result<Self, OxitoneError> {
        if points.is_empty() {
            return Err(OxitoneError::with_path(
                codes::AUTOMATION_POINTS,
                "curve needs at least one point",
                format!("{path}.points"),
            ));
        }
        let mut compiled = Vec::with_capacity(points.len());
        for (index, point) in points.iter().enumerate() {
            let point_path = format!("{path}.points[{index}]");
            if !point.value.is_finite() {
                return Err(OxitoneError::with_path(
                    codes::AUTOMATION_NON_FINITE,
                    format!("curve value must be finite, got {}", point.value),
                    format!("{point_path}.value"),
                ));
            }
            if !(0.0..=1.0).contains(&point.value) {
                return Err(OxitoneError::with_path(
                    codes::AUTOMATION_RANGE,
                    format!("curve value must be in 0..1, got {}", point.value),
                    format!("{point_path}.value"),
                ));
            }
            if index > 0 && crate::beat_cmp(point.beat, points[index - 1].beat).is_le() {
                return Err(OxitoneError::with_path(
                    codes::AUTOMATION_POINTS,
                    "curve point beats must strictly increase",
                    format!("{point_path}.beat"),
                ));
            }
            if let Some(Curve::Bezier { out, r#in }) = point.curve {
                for (name, control) in [("out", out), ("in", r#in)] {
                    if !control.iter().all(|v| v.is_finite()) {
                        return Err(OxitoneError::with_path(
                            codes::AUTOMATION_NON_FINITE,
                            "bezier control points must be finite",
                            format!("{point_path}.curve.{name}"),
                        ));
                    }
                    if !(0.0..=1.0).contains(&control[0]) {
                        return Err(OxitoneError::with_path(
                            codes::AUTOMATION_RANGE,
                            "bezier control x must be a 0..1 segment fraction",
                            format!("{point_path}.curve.{name}[0]"),
                        ));
                    }
                }
            }
            compiled.push(CurvePoint {
                beat: point.beat.to_f64(),
                value: point.value,
                curve: point.curve,
            });
        }
        let data = Self {
            interpolation,
            points: compiled,
        };
        for index in 0..data.points.len().saturating_sub(1) {
            if data.segment_kind(index) == CurveKind::Exponential
                && (data.points[index].value <= 0.0 || data.points[index + 1].value <= 0.0)
            {
                return Err(OxitoneError::with_path(
                    codes::AUTOMATION_EXPONENTIAL_ZERO,
                    "exponential interpolation requires both endpoint values > 0",
                    format!("{path}.points[{index}].value"),
                ));
            }
        }
        Ok(data)
    }

    fn segment_kind(&self, index: usize) -> CurveKind {
        self.points[index]
            .curve
            .map_or(self.interpolation, |curve| curve.kind())
    }

    /// Left-closed/right-open evaluation: before the first point holds the
    /// first value, at/after the last point holds the last value.
    pub fn value_at(&self, t: f64) -> f64 {
        let first = self.points[0];
        if t < first.beat {
            return first.value;
        }
        let index = self.points.partition_point(|point| point.beat <= t) - 1;
        let from = self.points[index];
        let Some(&to) = self.points.get(index + 1) else {
            return from.value;
        };
        let x = (t - from.beat) / (to.beat - from.beat);
        match from.curve.map_or(self.interpolation, |curve| curve.kind()) {
            CurveKind::Step => from.value,
            CurveKind::Linear => from.value + (to.value - from.value) * x,
            CurveKind::Smooth => {
                let s = x * x * (3.0 - 2.0 * x);
                from.value + (to.value - from.value) * s
            }
            CurveKind::Exponential => from.value * (to.value / from.value).powf(x),
            CurveKind::Bezier => match from.curve {
                Some(Curve::Bezier { out, r#in }) => bezier_value(from, to, out, r#in, t),
                // Source-level `bezier` without per-point control data has no
                // defined shape; fall back to linear (documented deviation).
                _ => from.value + (to.value - from.value) * x,
            },
        }
    }

    pub fn has_edge(&self, start: f64, end: f64) -> bool {
        self.points.iter().any(|p| p.beat > start && p.beat <= end)
    }

    /// Control point beats inside `[start, end)` (compile-time use).
    pub fn discontinuities(&self, start: f64, end: f64, out: &mut Vec<f64>) {
        for point in &self.points {
            if point.beat >= start && point.beat < end {
                out.push(point.beat);
            }
        }
    }
}

/// Monotone time-axis cubic Bezier: control x fractions are relative to the
/// segment; control y are absolute values. `x(u)` is monotonic because the
/// control x values stay inside the segment, so a bounded Newton+bisection
/// solve for `x(u) = t` is exact to f64 noise.
fn bezier_value(from: CurvePoint, to: CurvePoint, out: [f64; 2], r#in: [f64; 2], t: f64) -> f64 {
    let span = to.beat - from.beat;
    let cx = [
        from.beat,
        from.beat + out[0] * span,
        from.beat + r#in[0] * span,
        to.beat,
    ];
    let cy = [from.value, out[1], r#in[1], to.value];
    let cubic = |c: [f64; 4], u: f64| {
        let v = 1.0 - u;
        v * v * v * c[0] + 3.0 * v * v * u * c[1] + 3.0 * v * u * u * c[2] + u * u * u * c[3]
    };
    let mut u = (t - from.beat) / span;
    for _ in 0..8 {
        let dx = 3.0 * (1.0 - u) * (1.0 - u) * (cx[1] - cx[0])
            + 6.0 * (1.0 - u) * u * (cx[2] - cx[1])
            + 3.0 * u * u * (cx[3] - cx[2]);
        let error = cubic(cx, u) - t;
        if dx.abs() < 1e-12 {
            break;
        }
        u = (u - error / dx).clamp(0.0, 1.0);
        if error.abs() < 1e-12 * span.max(1.0) {
            break;
        }
    }
    // Bounded bisection refinement guards the flat-derivative cases.
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..32 {
        let mid = 0.5 * (lo + hi);
        if cubic(cx, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    cubic(cy, 0.5 * (lo + hi))
}
