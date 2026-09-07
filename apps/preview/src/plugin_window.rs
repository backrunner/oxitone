//! Detail windows observe accepted snapshots, and never own an audio backend.
use crate::{
    model::ViewProject,
    plugin_details::{self, DetailTarget, PluginDetails},
    theme::Theme,
    ui::Preview,
};
use gpui::{prelude::*, *};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Parameters,
    Resources,
    Plugin,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ParameterFilter {
    All,
    Source,
    Automated,
}

pub struct PluginWindow {
    pub target: DetailTarget,
    pub project: Arc<ViewProject>,
    pub details: Option<PluginDetails>,
    pub theme: Theme,
    pub tab: DetailTab,
    pub filter: ParameterFilter,
    pub scroll: ScrollHandle,
    pub scroll_drag: Option<(f32, f32, f32)>,
    pub copied: bool,
    pub parameter_specs: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl PluginWindow {
    fn new(
        target: DetailTarget,
        project: Arc<ViewProject>,
        owner: Entity<Preview>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let project_changes = cx.observe_in(&owner, window, |this, owner, window, cx| {
            if let Some(project) = &owner.read(cx).project {
                if !Arc::ptr_eq(project, &this.project) {
                    this.refresh(project.clone(), window);
                    cx.notify();
                }
            }
        });
        let appearance_changes = cx.observe_window_appearance(window, |this, window, cx| {
            this.theme = Theme::from_appearance(window.appearance());
            cx.notify();
        });
        let focus = cx.focus_handle();
        focus.focus(window);
        Self {
            details: plugin_details::resolve(&project, &target),
            target,
            project,
            theme: Theme::from_appearance(window.appearance()),
            tab: DetailTab::Parameters,
            filter: ParameterFilter::All,
            scroll: ScrollHandle::new(),
            scroll_drag: None,
            copied: false,
            parameter_specs: false,
            focus,
            _subscriptions: vec![project_changes, appearance_changes],
        }
    }
    fn refresh(&mut self, project: Arc<ViewProject>, window: &mut Window) {
        self.details = plugin_details::resolve(&project, &self.target);
        self.project = project;
        self.copied = false;
        window.set_window_title(&self.title());
    }
    pub fn title(&self) -> String {
        self.details.as_ref().map_or_else(
            || format!("{} · Not attached", self.target.label()),
            |d| format!("{} · {} · {}", d.owner_name, d.target.label(), d.name),
        )
    }
}

impl Preview {
    pub fn open_plugin(
        &mut self,
        target: DetailTarget,
        cx: &mut Context<Self>,
    ) -> Option<WindowHandle<PluginWindow>> {
        self.plugin_windows
            .retain(|_, handle| handle.read(cx).is_ok());
        if let Some(handle) = self.plugin_windows.get(&target).copied() {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return Some(handle);
            }
        }
        let project = self.project.clone()?;
        plugin_details::resolve(&project, &target)?;
        let owner = cx.entity();
        let key = target.clone();
        let mut bounds = Bounds::centered(None, size(px(700.), px(660.)), cx);
        let offset = px((self.plugin_windows.len() % 6) as f32 * 24.);
        bounds.origin += point(offset, offset);
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxitone · Plugin Details".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(18.), px(21.))),
                }),
                window_min_size: Some(size(px(540.), px(400.))),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let detail = PluginWindow::new(target, project, owner, window, cx);
                    window.set_window_title(&detail.title());
                    detail
                })
            },
        ) {
            Ok(handle) => {
                self.plugin_windows.insert(key, handle);
                Some(handle)
            }
            Err(error) => {
                self.diagnostic = Some(crate::model::Diagnostic {
                    code: "PreviewWindowFailed".into(),
                    message: error.to_string(),
                    path: None,
                });
                cx.notify();
                None
            }
        }
    }
}

impl Render for PluginWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id("plugin-details")
            .size_full()
            .flex()
            .flex_col()
            .font_family("Helvetica Neue")
            .bg(rgb(theme.bg))
            .text_color(rgb(theme.text))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "escape" || (key == "w" && event.keystroke.modifiers.platform) {
                    window.remove_window();
                    cx.stop_propagation();
                    return;
                }
                let delta = match key {
                    "up" => Some(32.),
                    "down" => Some(-32.),
                    "pageup" => Some(f32::from(this.scroll.bounds().size.height)),
                    "pagedown" => Some(-f32::from(this.scroll.bounds().size.height)),
                    "home" => Some(f32::from(this.scroll.max_offset().height)),
                    "end" => Some(-f32::from(this.scroll.max_offset().height)),
                    _ => None,
                };
                if let Some(delta) = delta {
                    this.scroll.set_offset(point(
                        px(0.),
                        (this.scroll.offset().y + px(delta))
                            .clamp(-this.scroll.max_offset().height, px(0.)),
                    ));
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.scroll_drag = None;
                }
                if let Some((pointer, offset, scale)) = this.scroll_drag {
                    let next = (offset - (f32::from(event.position.y) - pointer) * scale)
                        .clamp(-f32::from(this.scroll.max_offset().height), 0.);
                    this.scroll.set_offset(point(px(0.), px(next)));
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.scroll_drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.scroll_drag = None),
            )
            .child(crate::plugin_window_view::header(self, window))
            .child(crate::plugin_window_view::view(self, cx))
            .child(
                div()
                    .h(px(28.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .text_size(px(10.))
                    .text_color(rgb(theme.muted))
                    .border_t_1()
                    .border_color(rgb(theme.border))
                    .child(format!(
                        "READ ONLY · Revision {} · Synced from code",
                        self.project.snapshot.revision
                    )),
            )
    }
}
