use crate::{
    ui::Preview,
    workspace::Axis,
    workspace_layout::{DockMode, EditorMode, PanelLayout, ANALYSIS_BAR},
    workspace_resize::{divider, Resize},
};
use gpui::{prelude::*, *};

pub fn panels(this: &mut Preview, window: &Window, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let bounds = this.workspace.bounds.clone();
    let measured = bounds.get().size;
    let width = f32::from(window.viewport_size().width);
    let height = if measured.height > px(0.) {
        f32::from(measured.height)
    } else {
        f32::from(window.viewport_size().height) - 190.
    };
    let layout = PanelLayout::new(
        width,
        height,
        this.workspace.editor_height,
        this.workspace.piano_fraction,
        this.show_scopes.then_some(this.workspace.scopes_height),
    );
    let mode = this.workspace.mode;
    let mut content = div()
        .relative()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                .absolute()
                .size_full(),
        );
    match mode {
        EditorMode::Split => {
            content = content.child(crate::arrangement::view(this, width, cx));
            let piano = !this.document.windows.piano_open && this.workspace.dock != DockMode::Mixer;
            let mixer = !this.document.windows.mixer_open && this.workspace.dock != DockMode::Piano;
            if this.workspace.dock_open && (piano || mixer) {
                content = content.child(divider(
                    "editor-height-divider",
                    Axis::Vertical,
                    Resize::Editor {
                        height: layout.editor,
                        max: layout.max_editor,
                    },
                    this,
                    cx,
                ));
                let mut dock = div()
                    .h(px(layout.editor))
                    .flex_shrink_0()
                    .flex()
                    .overflow_hidden();
                let dock_mode = if piano && mixer {
                    DockMode::Split
                } else if piano {
                    DockMode::Piano
                } else {
                    DockMode::Mixer
                };
                match dock_mode {
                    DockMode::Piano => dock = dock.child(crate::piano::view(this, width, cx)),
                    DockMode::Mixer => {
                        dock = dock.child(crate::mixer::view(this, layout.editor, width, cx))
                    }
                    DockMode::Split => {
                        dock = dock.child(
                            div()
                                .h(px(layout.editor))
                                .flex_shrink_0()
                                .flex()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .w(px(layout.piano))
                                        .flex_shrink_0()
                                        .min_w_0()
                                        .h_full()
                                        .flex()
                                        .child(crate::piano::view(this, layout.piano, cx)),
                                )
                                .child(divider(
                                    "editor-width-divider",
                                    Axis::Horizontal,
                                    Resize::Split {
                                        width,
                                        piano: layout.piano,
                                    },
                                    this,
                                    cx,
                                ))
                                .child(crate::mixer::view(this, layout.editor, layout.mixer, cx)),
                        )
                    }
                }
                content = content.child(dock);
            }
        }
        EditorMode::Piano => {
            content = content.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(crate::piano::view(this, width, cx)),
            )
        }
        EditorMode::Mixer => {
            content = content.child(div().flex_1().min_h_0().flex().child(crate::mixer::view(
                this,
                layout.height,
                width,
                cx,
            )))
        }
    }
    if this.show_scopes {
        content = content.child(divider(
            "analysis-height-divider",
            Axis::Vertical,
            Resize::Scopes {
                height: layout.scopes,
                max: height * 0.35,
            },
            this,
            cx,
        ));
    }
    content
        .when(this.show_scopes, |d| {
            d.child(
                div()
                    .id("analysis-disclosure")
                    .h(px(ANALYSIS_BAR))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_t_1()
                    .border_color(rgb(t.border))
                    .bg(rgb(t.panel))
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .cursor_pointer()
                    .child(if this.show_scopes { "⌄" } else { "›" })
                    .child("Analysis")
                    .child(
                        div()
                            .text_color(crate::ui::alpha(t.muted, 0.8))
                            .child("Waveform / Spectrum / Stereo"),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_scopes = !this.show_scopes;
                        cx.notify();
                    })),
            )
        })
        .when(this.show_scopes, |d| {
            d.child(crate::scopes::view(this, layout.scopes))
        })
}

pub fn dock_tabs(this: &Preview, cx: &mut Context<Preview>) -> Div {
    let t = this.theme;
    let mut tabs = t.tool_group();
    for (id, icon, tip, mode) in [
        (
            "dock-piano",
            crate::ui_icons::Icon::Piano,
            "Dock piano roll",
            DockMode::Piano,
        ),
        (
            "dock-mixer",
            crate::ui_icons::Icon::Effect,
            "Dock mixer",
            DockMode::Mixer,
        ),
        (
            "dock-split",
            crate::ui_icons::Icon::Split,
            "Dock piano and mixer",
            DockMode::Split,
        ),
    ] {
        tabs = tabs.child(
            t.icon_tool(
                id,
                icon,
                tip,
                this.workspace.dock_open && this.workspace.dock == mode,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.workspace.dock_open = !this.workspace.dock_open || this.workspace.dock != mode;
                this.workspace.dock = mode;
                this.workspace.gesture = None;
                this.document.gesture = None;
                this.document.notes.marquee = None;
                cx.notify();
            })),
        );
    }
    tabs
}

impl Preview {
    pub fn toggle_editor(&mut self, mode: EditorMode, window: &mut Window) {
        if mode == EditorMode::Split {
            self.document.windows.hidden = true;
        }
        let floating = match mode {
            EditorMode::Piano => self
                .document
                .windows
                .piano_open
                .then_some(crate::window_manager::WindowId::Piano),
            EditorMode::Mixer => self
                .document
                .windows
                .mixer_open
                .then_some(crate::window_manager::WindowId::Mixer),
            EditorMode::Split => None,
        };
        if let Some(id) = floating {
            self.document.windows.toggle_maximize(id);
            return;
        }
        self.workspace.mode = if self.workspace.mode == mode {
            EditorMode::Split
        } else {
            mode
        };
        self.workspace.gesture = None;
        self.piano.snap_menu = None;
        self.document.gesture = None;
        self.document.notes.marquee = None;
        self.document.manager.searching = false;
        match self.workspace.mode {
            EditorMode::Piano => self.piano_focus.focus(window),
            EditorMode::Mixer => self.mixer_focus.focus(window),
            EditorMode::Split => self.workspace_focus.focus(window),
        }
    }
}
