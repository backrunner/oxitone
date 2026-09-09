//! Reusable curve handles preserve source-local range boundaries and interpolation.
use crate::document_wire::AutomationPoint;
use oxitone_core::{
    wire::{AutomationSourceSpec, Curve, CurveKind},
    Beat,
};
use oxitone_transport::CompiledAutomation;

pub const TICK: f64 = 1. / 960.;
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum CurveTool {
    Points,
    #[default]
    Draw,
    Line,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Interpolation {
    #[default]
    Linear,
    Smooth,
    Step,
}
impl Interpolation {
    pub fn curve(self) -> Curve {
        match self {
            Self::Linear => Curve::Linear {},
            Self::Smooth => Curve::Smooth {},
            Self::Step => Curve::Step {},
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Smooth => "Smooth",
            Self::Step => "Step",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Linear => Self::Smooth,
            Self::Smooth => Self::Step,
            Self::Step => Self::Linear,
        }
    }
}
#[derive(Clone)]
pub struct EditableCurve {
    pub start: f64,
    pub end: f64,
    pub points: Vec<AutomationPoint>,
}
pub fn editable(source: &AutomationSourceSpec) -> Option<EditableCurve> {
    let (source, start, end) = match source {
        AutomationSourceSpec::ReplaceRange {
            replacement,
            start_beat,
            end_beat,
            fade_beats,
            ..
        } if fade_beats.is_none_or(|b| b.to_f64() == 0.) => (
            replacement.as_ref(),
            start_beat.to_f64(),
            Some(end_beat.to_f64()),
        ),
        _ => (source, 0., None),
    };
    let AutomationSourceSpec::Curve {
        interpolation,
        points,
    } = source
    else {
        return None;
    };
    if points.is_empty() {
        return None;
    }
    let default_curve = match interpolation {
        CurveKind::Step => Curve::Step {},
        CurveKind::Smooth => Curve::Smooth {},
        CurveKind::Exponential => Curve::Exponential {},
        _ => Curve::Linear {},
    };
    let points: Vec<_> = points
        .iter()
        .map(|p| AutomationPoint {
            beat: start + p.beat.to_f64(),
            value: p.value,
            curve: Some(p.curve.unwrap_or(default_curve)),
        })
        .collect();
    let end = end.unwrap_or_else(|| points.last().unwrap().beat + TICK);
    Some(EditableCurve { start, end, points })
}
pub fn compiled_points(points: &[AutomationPoint]) -> Option<CompiledAutomation> {
    CompiledAutomation::compile(
        &AutomationSourceSpec::Curve {
            interpolation: CurveKind::Linear,
            points: points
                .iter()
                .map(|p| {
                    Ok(oxitone_core::wire::AutomationPoint {
                        beat: Beat::from_f64(p.beat)?,
                        value: p.value,
                        curve: p.curve,
                    })
                })
                .collect::<Result<_, oxitone_core::OxitoneError>>()
                .ok()?,
        },
        0,
    )
    .ok()
}
pub fn move_point(curve: &mut EditableCurve, index: usize, beat: f64, value: f64) {
    let low = if index == 0 {
        curve.start
    } else {
        curve.points[index - 1].beat + TICK
    };
    let high = if index + 1 == curve.points.len() {
        curve.end
    } else {
        curve.points[index + 1].beat - TICK
    };
    if low <= high {
        curve.points[index].beat = beat.clamp(low, high);
    }
    curve.points[index].value = value.clamp(0., 1.);
}
pub fn bend_segment(points: &mut [AutomationPoint], index: usize, amount: f64) {
    if index + 1 >= points.len() {
        return;
    }
    let a = points[index].value;
    let b = points[index + 1].value;
    points[index].curve = Some(Curve::Bezier {
        out: [1. / 3., (a + (b - a) / 3. + amount).clamp(0., 1.)],
        r#in: [2. / 3., (a + (b - a) * 2. / 3. + amount).clamp(0., 1.)],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitone_transport::EvalContext;
    fn p(beat: f64, value: f64) -> AutomationPoint {
        AutomationPoint {
            beat,
            value,
            curve: None,
        }
    }
    #[test]
    fn handles_do_not_cross_neighbors_or_escape_the_edited_range() {
        let mut curve = EditableCurve {
            start: 2.,
            end: 6.,
            points: vec![p(2., 0.), p(3., 0.5), p(6., 1.)],
        };
        move_point(&mut curve, 1, 9., -2.);
        assert!(curve.points[1].beat < 6.);
        assert_eq!(curve.points[1].value, 0.);
        move_point(&mut curve, 0, -3., 2.);
        assert_eq!(curve.points[0], p(2., 1.));
    }
    #[test]
    fn tension_uses_the_native_bezier_evaluator_and_keeps_endpoints() {
        let mut points = vec![p(0., 0.2), p(4., 0.8)];
        bend_segment(&mut points, 0, 0.2);
        let compiled = compiled_points(&points).unwrap();
        assert!((compiled.value_at(0., &EvalContext::default()) - 0.2).abs() < 1e-8);
        assert_eq!(compiled.value_at(4., &EvalContext::default()), 0.8);
        assert!(compiled.value_at(2., &EvalContext::default()) > 0.6);
    }
}
