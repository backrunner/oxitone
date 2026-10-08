//! Opt-in README framing of the real workspace; only view preferences change.
use crate::{ui::Preview, workspace_layout::DockMode};
use gpui::*;

pub fn prepare(view: &mut Preview, cx: &mut Context<Preview>) {
    if std::env::var("OXITONE_PREVIEW_CAPTURE_LAYOUT").as_deref() != Ok("readme") {
        return;
    }
    let pattern = view.project.as_ref().and_then(|project| {
        crate::pattern_navigation::patterns(project)
            .get(1)
            .map(|pattern| pattern.id.clone())
    });
    if let Some(pattern) = pattern {
        view.select_pattern(&pattern);
    }
    view.document.patterns_open = false;
    view.workspace.dock_open = true;
    view.workspace.dock = DockMode::Split;
    view.workspace.editor_height = 430.;
    cx.notify();
}
