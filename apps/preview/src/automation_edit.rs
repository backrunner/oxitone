use crate::{
    automation_curve::{self, CurveTool, EditableCurve, TICK},
    document_wire::{AutomationPoint, AutomationRangeEdit, AutomationSite, DocumentOperation},
    piano_layout::Snap,
    ui::Preview,
};
use gpui::*;

pub use crate::automation_state::AutomationUi;
use crate::automation_state::{AutomationGesture, CurveAction};

impl Preview {
    pub fn automation_lane(&self) -> Option<&oxitone_core::wire::AutomationLaneSpec> {
        let project = self.project.as_ref()?;
        project
            .snapshot
            .automation
            .iter()
            .find(|lane| Some(&lane.id) == self.document.automation.selected.as_ref())
            .or_else(|| project.snapshot.automation.first())
    }
    pub fn automation_site(&self) -> Option<&AutomationSite> {
        let lane = self.automation_lane()?;
        let view = self.document.view.as_ref()?;
        if self.document.automation.shared {
            view.automation_sites
                .iter()
                .find(|site| site.scope == "definition" && site.lanes.contains(&lane.id))
        } else {
            view.automation_sites
                .iter()
                .find(|site| site.scope == "reference" && site.lanes == [lane.id.clone()])
                .or_else(|| {
                    view.automation_sites
                        .iter()
                        .find(|site| site.lanes == [lane.id.clone()])
                })
        }
    }

    fn automation_position(&self, position: Point<Pixels>, fine: bool) -> Option<AutomationPoint> {
        let state = &self.document.automation;
        let bounds = state.bounds.get();
        let width = f64::from(f32::from(bounds.size.width));
        let height = f64::from(f32::from(bounds.size.height));
        if width <= 0. || height <= 0. {
            return None;
        }
        let x = f64::from(f32::from(position.x - bounds.origin.x)) / width;
        let y = f64::from(f32::from(position.y - bounds.origin.y)) / height;
        let snap = if fine { Snap::Free } else { state.snap };
        Some(AutomationPoint {
            beat: snap
                .round(state.offset + x.clamp(0., 1.) * state.beats)
                .max(0.),
            value: (1. - y).clamp(0., 1.),
            curve: Some(state.interpolation.curve()),
        })
    }
    pub fn press_automation(&mut self, position: Point<Pixels>) {
        self.press_curve(&MouseDownEvent {
            position,
            ..Default::default()
        });
    }
    pub fn press_curve(&mut self, event: &MouseDownEvent) {
        if !self.document_ready() {
            return;
        }
        let Some(site) = self.automation_site().cloned() else {
            return;
        };
        let Some(point) = self.automation_position(event.position, event.modifiers.shift) else {
            return;
        };
        let state = &self.document.automation;
        let existing = automation_curve::editable(&site.source);
        let bounds = state.bounds.get();
        let width = f64::from(f32::from(bounds.size.width));
        let height = f64::from(f32::from(bounds.size.height));
        let hit = existing.as_ref().and_then(|curve| {
            curve.points.iter().position(|p| {
                (((p.beat - state.offset) / state.beats * width)
                    - f64::from(f32::from(event.position.x - bounds.origin.x)))
                .abs()
                    <= 8.
                    && ((1. - p.value) * height
                        - f64::from(f32::from(event.position.y - bounds.origin.y)))
                    .abs()
                        <= 8.
            })
        });
        let mut curve = existing.clone().unwrap_or_else(|| EditableCurve {
            start: point.beat,
            end: point.beat + state.snap.step(),
            points: vec![],
        });
        if matches!(
            site.source,
            oxitone_core::wire::AutomationSourceSpec::Curve { .. }
        ) {
            curve.end = curve.end.max(state.offset + state.beats);
        }
        let original = curve.points.clone();
        let action;
        if event.button == MouseButton::Right {
            let Some(index) = hit else {
                return;
            };
            if curve.points.len() <= 2 || index == 0 || index + 1 == curve.points.len() {
                return;
            }
            curve.points.remove(index);
            action = CurveAction::Delete;
        } else if let Some(index) = hit {
            action = CurveAction::Point(index);
        } else if event.modifiers.alt {
            let Some(index) = curve
                .points
                .windows(2)
                .position(|pair| point.beat > pair[0].beat && point.beat < pair[1].beat)
            else {
                return;
            };
            action = CurveAction::Bend(index, point.value);
        } else if state.tool == CurveTool::Points
            && existing.is_some()
            && point.beat >= curve.start
            && point.beat <= curve.end
        {
            if curve.points.len() >= 4096 {
                return;
            }
            let index = curve.points.partition_point(|p| p.beat < point.beat);
            if curve
                .points
                .iter()
                .any(|p| (p.beat - point.beat).abs() < TICK)
            {
                return;
            }
            curve.points.insert(index, point.clone());
            action = CurveAction::Point(index);
        } else {
            curve = EditableCurve {
                start: point.beat,
                end: point.beat + state.snap.step(),
                points: vec![point.clone()],
            };
            action = if state.tool == CurveTool::Draw {
                CurveAction::Draw
            } else {
                CurveAction::Line(point.clone())
            };
        }
        let mut gesture = AutomationGesture {
            site: site.handle,
            lane: (!state.shared && site.scope == "reference").then(|| site.lanes[0].clone()),
            curve,
            action,
            original,
            preview: None,
        };
        gesture.refresh();
        self.document.automation.gesture = Some(gesture);
        if event.button == MouseButton::Right {
            self.finish_automation();
        }
    }
    pub fn move_automation(&mut self, position: Point<Pixels>) {
        self.move_curve(position, false);
    }
    pub fn move_curve(&mut self, position: Point<Pixels>, fine: bool) {
        let Some(point) = self.automation_position(position, fine) else {
            return;
        };
        let state = &mut self.document.automation;
        let Some(gesture) = &mut state.gesture else {
            return;
        };
        match &gesture.action {
            CurveAction::Point(index) => {
                automation_curve::move_point(&mut gesture.curve, *index, point.beat, point.value)
            }
            CurveAction::Bend(index, start) => {
                gesture.curve.points.clone_from(&gesture.original);
                automation_curve::bend_segment(
                    &mut gesture.curve.points,
                    *index,
                    (point.value - start) * 1.5,
                );
            }
            CurveAction::Line(start) => {
                gesture.curve.points = if point.beat == start.beat {
                    vec![point.clone()]
                } else if point.beat > start.beat {
                    vec![start.clone(), point.clone()]
                } else {
                    vec![point.clone(), start.clone()]
                };
            }
            CurveAction::Draw => {
                let index = gesture
                    .curve
                    .points
                    .partition_point(|p| p.beat < point.beat);
                if gesture
                    .curve
                    .points
                    .get(index)
                    .is_some_and(|p| p.beat == point.beat)
                {
                    gesture.curve.points[index] = point.clone();
                } else if gesture.curve.points.len() < 4096 {
                    gesture.curve.points.insert(index, point.clone());
                }
            }
            CurveAction::Delete => return,
        }
        if matches!(gesture.action, CurveAction::Draw | CurveAction::Line(_)) {
            gesture.curve.start = gesture.curve.points[0].beat;
            gesture.curve.end = gesture.curve.points.last().unwrap().beat + state.snap.step();
        }
        gesture.refresh();
    }
    pub fn finish_automation(&mut self) {
        let Some(gesture) = self.document.automation.gesture.take() else {
            return;
        };
        if gesture.curve.points.is_empty() || gesture.curve.points == gesture.original {
            return;
        }
        let curve = &gesture.curve;
        let points = curve
            .points
            .iter()
            .map(|point| AutomationPoint {
                beat: (point.beat - curve.start).max(0.),
                value: point.value,
                curve: point.curve,
            })
            .collect();
        self.document_request(DocumentOperation::AutomationRange {
            site: gesture.site.clone(),
            lane: gesture.lane.clone(),
            clip: None,
            edit: AutomationRangeEdit {
                start: curve.start,
                end: curve.end,
                points,
            },
        });
        if self.document.pending.is_some() {
            self.document.automation.pending = Some(gesture);
        }
    }
    pub fn scroll_automation(&mut self, event: &ScrollWheelEvent) {
        if self.document.automation.gesture.is_some() {
            return;
        }
        let state = &mut self.document.automation;
        let delta = event.delta.pixel_delta(px(20.));
        let bounds = state.bounds.get();
        let width = f32::from(bounds.size.width).max(1.);
        if event.modifiers.platform || event.modifiers.control {
            let fraction =
                (f32::from(event.position.x - bounds.origin.x) / width).clamp(0., 1.) as f64;
            let anchor = state.offset + fraction * state.beats;
            state.beats =
                (state.beats * (-f64::from(f32::from(delta.y)) / 220.).exp()).clamp(0.25, 1024.);
            state.offset = (anchor - fraction * state.beats).max(0.);
        } else {
            state.offset = (state.offset
                - f64::from(f32::from(delta.x + delta.y) / width) * state.beats)
                .max(0.);
        }
    }
}
