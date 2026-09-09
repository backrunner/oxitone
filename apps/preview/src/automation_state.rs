use crate::{
    automation_curve::{self, CurveTool, EditableCurve, Interpolation},
    document_wire::AutomationPoint,
    piano_layout::Snap,
};
use gpui::*;
use oxitone_transport::CompiledAutomation;
use std::{cell::Cell, rc::Rc, sync::Arc};

pub struct AutomationUi {
    pub open: bool,
    pub selected: Option<String>,
    pub shared: bool,
    pub beats: f64,
    pub offset: f64,
    pub snap: Snap,
    pub tool: CurveTool,
    pub interpolation: Interpolation,
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    pub compiled: Option<(String, Arc<CompiledAutomation>)>,
    pub gesture: Option<AutomationGesture>,
    pub pending: Option<AutomationGesture>,
}
impl Default for AutomationUi {
    fn default() -> Self {
        Self {
            open: false,
            selected: None,
            shared: false,
            beats: 8.,
            offset: 0.,
            snap: Snap::Quarter,
            tool: CurveTool::default(),
            interpolation: Interpolation::default(),
            bounds: Rc::default(),
            compiled: None,
            gesture: None,
            pending: None,
        }
    }
}
#[derive(Clone)]
pub enum CurveAction {
    Draw,
    Line(AutomationPoint),
    Point(usize),
    Bend(usize, f64),
    Delete,
}
#[derive(Clone)]
pub struct AutomationGesture {
    pub site: String,
    pub lane: Option<String>,
    pub curve: EditableCurve,
    pub action: CurveAction,
    pub original: Vec<AutomationPoint>,
    pub preview: Option<Arc<CompiledAutomation>>,
}
impl AutomationGesture {
    pub(super) fn refresh(&mut self) {
        self.preview = automation_curve::compiled_points(&self.curve.points).map(Arc::new);
    }
}
