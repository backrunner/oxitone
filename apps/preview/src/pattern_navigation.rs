//! A shared pattern selection for the titlebar, rack, Browser and piano.
use crate::{
    model::ViewProject,
    ui::Preview,
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};
use oxitone_core::wire::PatternSpec;
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub struct PatternPicker {
    pub selected: Option<usize>,
    pub button: Rc<Cell<Bounds<Pixels>>>,
    pub scroll: ScrollHandle,
}

pub fn patterns(project: &ViewProject) -> Vec<&PatternSpec> {
    let mut patterns: Vec<_> = project
        .snapshot
        .patterns
        .iter()
        .filter(|pattern| {
            !project.snapshot.patterns.iter().any(|root| {
                root.parts
                    .as_ref()
                    .is_some_and(|parts| parts.iter().any(|part| part.pattern_id == pattern.id))
            }) || project
                .snapshot
                .pattern_clips
                .iter()
                .any(|clip| clip.pattern_id == pattern.id)
        })
        .collect();
    patterns.sort_by_key(|pattern| project.pattern_label(&pattern.id));
    patterns
}

impl Preview {
    pub fn active_pattern(&self) -> Option<&PatternSpec> {
        let project = self.project.as_ref()?;
        let id = project
            .snapshot
            .pattern_clips
            .iter()
            .find(|clip| Some(&clip.id) == self.selected_clip.as_ref())
            .map(|clip| &clip.pattern_id)
            .or(self.document.patterns_selected.as_ref())?;
        project.snapshot.patterns.iter().find(|p| &p.id == id)
    }
    pub fn reconcile_pattern_selection(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        if let Some(clip) = project
            .snapshot
            .pattern_clips
            .iter()
            .find(|clip| Some(&clip.id) == self.selected_clip.as_ref())
        {
            self.document.patterns_selected = Some(clip.pattern_id.clone());
            return;
        }
        let id = self
            .document
            .patterns_selected
            .as_ref()
            .filter(|id| project.snapshot.patterns.iter().any(|p| &p.id == *id))
            .cloned()
            .or_else(|| project.initial_clip().map(|c| c.pattern_id.clone()))
            .or_else(|| patterns(project).first().map(|p| p.id.clone()));
        self.selected_clip = project
            .snapshot
            .pattern_clips
            .iter()
            .find(|clip| Some(&clip.pattern_id) == id.as_ref())
            .map(|clip| clip.id.clone());
        self.document.patterns_selected = id;
    }
    pub fn select_pattern(&mut self, id: &str) {
        if self.active_pattern().is_some_and(|p| p.id == id) {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        if !project.snapshot.patterns.iter().any(|p| p.id == id) {
            return;
        }
        self.selected_clip = project
            .snapshot
            .pattern_clips
            .iter()
            .find(|clip| clip.pattern_id == id)
            .map(|clip| clip.id.clone());
        self.document.patterns_selected = Some(id.into());
        self.piano.channel = None;
        self.document.notes.clear();
        crate::pointer_capture::cancel(self);
        self.piano.fit();
    }
    pub fn step_pattern(&mut self, direction: isize) {
        let Some(project) = &self.project else {
            return;
        };
        let options = patterns(project);
        if options.is_empty() {
            return;
        }
        let current = options
            .iter()
            .position(|p| self.active_pattern().is_some_and(|v| v.id == p.id))
            .unwrap_or(0);
        let index = (current as isize + direction).rem_euclid(options.len() as isize) as usize;
        let id = options[index].id.clone();
        self.select_pattern(&id);
    }
    pub fn pattern_picker_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.pattern_picker.selected else {
            return;
        };
        let options: Vec<_> = self
            .project
            .as_ref()
            .map(|p| patterns(p).iter().map(|p| p.id.clone()).collect())
            .unwrap_or_default();
        let count = options.len();
        match event.keystroke.key.as_str() {
            "escape" | "tab" => self.pattern_picker.selected = None,
            "up" if count > 0 => self.pattern_picker.selected = Some((index + count - 1) % count),
            "down" if count > 0 => self.pattern_picker.selected = Some((index + 1) % count),
            "home" if count > 0 => self.pattern_picker.selected = Some(0),
            "end" if count > 0 => self.pattern_picker.selected = Some(count - 1),
            "enter" if count > 0 && !event.is_held => {
                self.select_pattern(&options[index.min(count - 1)]);
                self.pattern_picker.selected = None;
                self.workspace_focus.focus(window);
            }
            _ => {}
        }
        if let Some(index) = self.pattern_picker.selected {
            self.pattern_picker.scroll.scroll_to_item(index);
        }
        cx.stop_propagation();
        cx.notify();
    }
}

pub fn selector(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let label = this
        .active_pattern()
        .and_then(|p| {
            this.project
                .as_ref()
                .map(|project| project.pattern_label(&p.id))
        })
        .unwrap_or_else(|| "No patterns".into());
    let bounds = this.pattern_picker.button.clone();
    let mut group = t.tool_group();
    group = group.child(
        t.tool("pattern-picker", "", this.pattern_picker.selected.is_some())
            .w(px(148.))
            .gap_2()
            .relative()
            .child(icon(Icon::Arrange, t.accent))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(icon(Icon::ChevronDown, t.muted))
            .child(
                canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                let options = this
                    .project
                    .as_ref()
                    .map(|p| patterns(p))
                    .unwrap_or_default();
                let index = options
                    .iter()
                    .position(|p| this.active_pattern().is_some_and(|v| v.id == p.id))
                    .unwrap_or(0);
                let available = !options.is_empty();
                this.pattern_picker.selected =
                    (available && this.pattern_picker.selected.is_none()).then_some(index);
                this.pattern_picker.scroll.scroll_to_item(index);
                this.view_menu.selected = None;
                this.piano.snap_menu = None;
                this.document.tempo.input = None;
                crate::pointer_capture::cancel(this);
                this.workspace_focus.focus(window);
                cx.notify();
            })),
    );
    for (id, glyph, tip, direction) in [
        ("previous-pattern", Icon::Left, "Previous pattern", -1),
        ("next-pattern", Icon::Right, "Next pattern", 1),
    ] {
        group = group.child(
            t.icon_button(id, glyph, tip)
                .w(px(22.))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.step_pattern(direction);
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        );
    }
    group
}
