//! View navigation changes only the local workspace and internal-window state.
use crate::{
    ui::Preview,
    ui_icons::{icon, Icon},
    window_manager::WindowId,
    workspace_layout::EditorMode,
};
use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub enum View {
    Arrange,
    Piano,
    Mixer,
    Patterns,
    Automation,
    Plugins,
    Browser,
    Source,
    About,
}
pub const VIEWS: [View; 9] = [
    View::Arrange,
    View::Piano,
    View::Mixer,
    View::Patterns,
    View::Automation,
    View::Plugins,
    View::Browser,
    View::Source,
    View::About,
];
impl View {
    pub fn label(self) -> &'static str {
        match self {
            Self::Arrange => "Arrange",
            Self::Piano => "Piano",
            Self::Mixer => "Mixer",
            Self::Patterns => "Patterns",
            Self::Automation => "Automation",
            Self::Plugins => "Plugins",
            Self::Browser => "Browser",
            Self::Source => "Source code",
            Self::About => "About Oxitone",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Self::Arrange => Icon::Arrange,
            Self::Piano => Icon::Piano,
            Self::Mixer => Icon::Effect,
            Self::Patterns => Icon::Arrange,
            Self::Automation => Icon::Curve,
            Self::Plugins => Icon::Wave,
            Self::Browser => Icon::Library,
            Self::Source => Icon::Code,
            Self::About => Icon::Info,
        }
    }
    pub fn active(self, this: &Preview) -> bool {
        let front = this.document.windows.front();
        match self {
            Self::Arrange => front.is_none() && this.workspace.mode == EditorMode::Split,
            Self::Piano => {
                front == Some(WindowId::Piano)
                    || (front.is_none() && this.workspace.mode == EditorMode::Piano)
            }
            Self::Mixer => {
                front == Some(WindowId::Mixer)
                    || (front.is_none() && this.workspace.mode == EditorMode::Mixer)
            }
            Self::Automation => front == Some(WindowId::Automation),
            Self::Patterns => front == Some(WindowId::Patterns),
            Self::Plugins => front == Some(WindowId::Plugins),
            Self::Browser => {
                front == Some(WindowId::Browser)
                    || (this.document.browser_open && this.workspace.browser_docked)
            }
            Self::About => this.show_about,
            Self::Source => this.document.show_code,
        }
    }
}
impl Preview {
    pub fn view_options(&self) -> Vec<View> {
        VIEWS
            .into_iter()
            .filter(|view| {
                self.document.view.is_some()
                    || !matches!(view, View::Automation | View::Plugins | View::Source)
            })
            .collect()
    }
    pub fn open_view(&mut self, view: View, window: &mut Window) {
        self.view_menu.selected = None;
        self.pattern_picker.selected = None;
        self.piano.snap_menu = None;
        crate::pointer_capture::cancel(self);
        self.document.manager.searching = false;
        match view {
            View::Arrange => {
                self.document.windows.hidden = true;
                self.workspace.mode = EditorMode::Split;
            }
            View::Piano => self.float_editor(EditorMode::Piano),
            View::Mixer => self.float_editor(EditorMode::Mixer),
            View::Patterns => {
                self.document.patterns_open = true;
                self.document.windows.focus(WindowId::Patterns);
            }
            View::About => self.show_about = true,
            View::Source => self.document.show_code = !self.document.show_code,
            View::Automation => {
                self.document.automation.open = true;
                self.document.windows.focus(WindowId::Automation);
            }
            View::Plugins => {
                self.document.manager.open = true;
                self.document.windows.focus(WindowId::Plugins);
            }
            View::Browser => {
                self.document.browser_open = true;
                if !self.workspace.browser_docked {
                    self.document.windows.focus(WindowId::Browser);
                }
                if self.document.patterns_selected.is_none() {
                    self.document.patterns_selected = self.selected_clip.as_ref().and_then(|id| {
                        self.project.as_ref().and_then(|project| {
                            project
                                .snapshot
                                .pattern_clips
                                .iter()
                                .find(|clip| &clip.id == id)
                                .map(|clip| clip.pattern_id.clone())
                        })
                    });
                }
            }
        }
        self.workspace_focus.focus(window);
    }
    pub fn header_tools(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let button = self.view_menu.button.clone();
        div().flex().items_center().gap_1().flex_shrink_0().child(
            t.icon_tool(
                "view-menu-button",
                Icon::Split,
                "Views",
                self.view_menu.selected.is_some(),
            )
            .w(px(44.))
            .gap(px(2.))
            .relative()
            .child(icon(Icon::ChevronDown, t.muted))
            .child(
                canvas(move |bounds, _, _| button.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.view_menu.selected = if this.view_menu.selected.is_some() {
                    None
                } else {
                    Some(
                        this.view_options()
                            .iter()
                            .position(|v| v.active(this))
                            .unwrap_or(0),
                    )
                };
                this.piano.snap_menu = None;
                this.pattern_picker.selected = None;
                crate::pointer_capture::cancel(this);
                this.workspace_focus.focus(window);
                cx.notify();
            })),
        )
    }
}
