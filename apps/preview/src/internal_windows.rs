//! Window content composition, separate from geometry and chrome.
use crate::{
    ui::Preview,
    window_manager::WindowId,
    workspace_layout::{DockMode, EditorMode},
};
use gpui::{prelude::*, *};

pub fn overlay(this: &mut Preview, cx: &mut Context<Preview>) -> Div {
    let mut layer = div().absolute().inset_0();
    if this.document.windows.hidden {
        return layer;
    }
    let mut open = Vec::new();
    if this.document.patterns_open {
        open.push(WindowId::Patterns);
    }
    if this.document.automation.open {
        open.push(WindowId::Automation);
    }
    if this.document.manager.open {
        open.push(WindowId::Plugins);
    }
    if this.document.configuration.open {
        open.push(WindowId::Configuration);
    }
    if this.document.windows.piano_open {
        open.push(WindowId::Piano);
    }
    if this.document.windows.mixer_open {
        open.push(WindowId::Mixer);
    }
    open.extend(
        this.plugin_windows
            .values()
            .map(|entity| WindowId::Plugin(entity.entity_id().as_u64())),
    );
    open.sort_by_key(|id| this.document.windows.state(*id).z);
    this.document.windows.visible = open.clone();
    for id in open {
        let bounds = this.document.windows.bounds(id);
        let width = (bounds.width - 2.).max(1.);
        let height = (bounds.height - 37.).max(1.);
        let (title, content): (String, AnyElement) = match id {
            WindowId::Patterns => (
                "Browser".into(),
                crate::pattern_manager::view(this, cx).into_any_element(),
            ),
            WindowId::Automation => (
                "Automation".into(),
                this.automation_panel(cx).into_any_element(),
            ),
            WindowId::Plugins => ("Plugins".into(), this.plugin_manager(cx).into_any_element()),
            WindowId::Configuration => (
                this.configuration_title(),
                this.configuration_editor(cx).into_any_element(),
            ),
            WindowId::Piano => (
                format!(
                    "Piano roll · {}",
                    this.piano_pattern().map_or_else(
                        || "Pattern".into(),
                        |p| this.project.as_ref().unwrap().pattern_label(&p.id)
                    )
                ),
                crate::piano::view(this, width, cx).into_any_element(),
            ),
            WindowId::Mixer => (
                "Mixer".into(),
                crate::mixer::view(this, height, width, cx).into_any_element(),
            ),
            WindowId::Plugin(serial) => {
                let entity = this
                    .plugin_windows
                    .values()
                    .find(|entity| entity.entity_id().as_u64() == serial)
                    .unwrap()
                    .clone();
                let title = entity.read(cx).title();
                entity.update(cx, |plugin, cx| {
                    if plugin.width != width {
                        plugin.width = width;
                        cx.notify();
                    }
                });
                (title, entity.into_any_element())
            }
        };
        layer = layer.child(crate::internal_window_frame::view(
            this, id, title, content, cx,
        ));
    }
    layer
}

impl Preview {
    pub fn close_internal(&mut self, id: WindowId, window: &mut Window) {
        match id {
            WindowId::Patterns => self.document.patterns_open = false,
            WindowId::Automation => self.document.automation.open = false,
            WindowId::Plugins => {
                self.document.manager.open = false;
                self.document.manager.searching = false;
            }
            WindowId::Configuration => {
                self.document.configuration.open = false;
                self.document.configuration.input = None;
            }
            WindowId::Piano => self.document.windows.piano_open = false,
            WindowId::Mixer => self.document.windows.mixer_open = false,
            WindowId::Plugin(serial) => {
                self.plugin_windows
                    // Closing a panel cancels its unsubmitted pointer gesture.
                    .retain(|_, entity| entity.entity_id().as_u64() != serial);
                self.document.plugin.cancel();
                self.document.windows.forget(id);
            }
        }
        self.document.windows.visible.retain(|key| *key != id);
        self.document.windows.end_drag();
        if let Some(front) = self.document.windows.front() {
            self.document.windows.focus(front);
            match front {
                WindowId::Piano => self.piano_focus.focus(window),
                WindowId::Mixer => self.mixer_focus.focus(window),
                _ => self.workspace_focus.focus(window),
            }
        } else {
            self.workspace_focus.focus(window);
        }
    }

    pub fn float_editor(&mut self, mode: EditorMode) {
        let id = match mode {
            EditorMode::Piano => {
                self.document.windows.piano_open = true;
                WindowId::Piano
            }
            EditorMode::Mixer => {
                self.document.windows.mixer_open = true;
                WindowId::Mixer
            }
            EditorMode::Split => return,
        };
        self.workspace.mode = EditorMode::Split;
        self.document.windows.focus(id);
    }

    pub fn dock_editor(&mut self, id: WindowId, window: &mut Window) {
        self.close_internal(id, window);
        self.workspace.mode = EditorMode::Split;
        self.workspace.dock = if id == WindowId::Piano {
            DockMode::Piano
        } else {
            DockMode::Mixer
        };
        self.workspace.dock_open = true;
    }
}
