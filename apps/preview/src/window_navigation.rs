use crate::{ui::Preview, workspace_layout::EditorMode};
use gpui::{prelude::*, *};
impl Preview {
    pub fn editor_tabs(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let mut tabs = t.tool_group();
        let front = self.document.windows.front();
        use crate::window_manager::WindowId;
        for (id, label, mode) in [
            ("workspace-tab", "Arrange", EditorMode::Split),
            ("piano-tab", "Piano roll", EditorMode::Piano),
            ("mixer-tab", "Mixer", EditorMode::Mixer),
        ] {
            tabs = tabs.child(
                t.tool(
                    id,
                    label,
                    match mode {
                        EditorMode::Split => front.is_none() && self.workspace.mode == mode,
                        EditorMode::Piano => {
                            front == Some(WindowId::Piano)
                                || (front.is_none() && self.workspace.mode == mode)
                        }
                        EditorMode::Mixer => {
                            front == Some(WindowId::Mixer)
                                || (front.is_none() && self.workspace.mode == mode)
                        }
                    },
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if mode == EditorMode::Split {
                        this.document.windows.hidden = true;
                        this.workspace.mode = mode;
                        this.workspace_focus.focus(window);
                    } else {
                        this.float_editor(mode);
                    }
                    cx.notify();
                })),
            );
        }
        if self.document.view.is_some() {
            tabs = tabs
                .child(div().w(px(1.)).h(px(16.)).mx_2().bg(rgb(t.border)))
                .child(
                    t.tool(
                        "automation-editor",
                        "Automation",
                        front == Some(WindowId::Automation),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.document.automation.open = true;
                        this.document
                            .windows
                            .focus(crate::window_manager::WindowId::Automation);
                        this.document.manager.searching = false;
                        this.document.gesture = None;
                        this.workspace.gesture = None;
                        this.workspace_focus.focus(window);
                        cx.notify();
                    })),
                )
                .child(
                    t.tool(
                        "plugin-manager",
                        "Plugins",
                        front == Some(WindowId::Plugins),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.document.manager.open = true;
                        this.document
                            .windows
                            .focus(crate::window_manager::WindowId::Plugins);
                        this.document.gesture = None;
                        this.workspace.gesture = None;
                        this.workspace_focus.focus(window);
                        cx.notify();
                    })),
                )
                .child(
                    t.tool(
                        "pattern-manager",
                        "Browser",
                        front == Some(WindowId::Patterns),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.document.patterns_open = true;
                        this.document
                            .windows
                            .focus(crate::window_manager::WindowId::Patterns);
                        if this.document.patterns_selected.is_none() {
                            this.document.patterns_selected =
                                this.selected_clip.as_ref().and_then(|id| {
                                    this.project.as_ref().and_then(|project| {
                                        project
                                            .snapshot
                                            .pattern_clips
                                            .iter()
                                            .find(|clip| &clip.id == id)
                                            .map(|clip| clip.pattern_id.clone())
                                    })
                                });
                        }
                        this.document.gesture = None;
                        this.workspace.gesture = None;
                        this.workspace_focus.focus(window);
                        cx.notify();
                    })),
                );
        }
        tabs
    }
}
