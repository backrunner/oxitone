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
    Panel,
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
    pub panel: Option<Arc<crate::plugin_layout::Layout>>,
    pub page: String,
    pub stacked_waveforms: bool,
    pub sync_status: String,
    owner: WeakEntity<Preview>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl PluginWindow {
    pub(crate) fn new(
        target: DetailTarget,
        project: Arc<ViewProject>,
        owner: Entity<Preview>,
        status: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let project_changes = cx.observe_in(&owner, window, |this, owner, window, cx| {
            let owner = owner.read(cx);
            let status = sync_status(owner);
            let mut changed = status != this.sync_status;
            this.sync_status = status;
            if let Some(project) = &owner.project {
                if !Arc::ptr_eq(project, &this.project) {
                    this.refresh(project.clone(), window);
                    changed = true;
                }
            }
            if changed {
                cx.notify();
            }
        });
        let appearance_changes = cx.observe_window_appearance(window, |this, window, cx| {
            this.theme = Theme::from_appearance(window.appearance());
            cx.notify();
        });
        let focus = cx.focus_handle();
        focus.focus(window);
        let details = plugin_details::resolve(&project, &target);
        let panel = resolve_panel(&project, details.as_ref());
        Self {
            stacked_waveforms: true,
            page: panel
                .as_ref()
                .and_then(|p| p.pages.first())
                .map_or_else(String::new, |p| p.id.clone()),
            panel,
            sync_status: status,
            details,
            target,
            project,
            theme: Theme::from_appearance(window.appearance()),
            tab: DetailTab::Panel,
            filter: ParameterFilter::All,
            scroll: ScrollHandle::new(),
            scroll_drag: None,
            copied: false,
            parameter_specs: false,
            owner: owner.downgrade(),
            focus,
            _subscriptions: vec![project_changes, appearance_changes],
        }
    }
    fn refresh(&mut self, project: Arc<ViewProject>, window: &mut Window) {
        self.details = plugin_details::resolve(&project, &self.target);
        let panel = resolve_panel(&project, self.details.as_ref());
        if let Some(next) = &panel {
            if !window.is_fullscreen()
                && self.panel.as_ref().is_some_and(|old| {
                    old.size.width != next.size.width || old.size.height != next.size.height
                })
            {
                window.resize(size(
                    px(next.size.width as f32),
                    px(next.size.height as f32),
                ));
            }
        }
        let same_identity = self
            .panel
            .as_ref()
            .zip(panel.as_ref())
            .is_some_and(|(old, new)| {
                old.plugin_id == new.plugin_id && old.plugin_version == new.plugin_version
            });
        if !same_identity
            || !panel
                .as_ref()
                .is_some_and(|p| p.pages.iter().any(|p| p.id == self.page))
        {
            self.page = panel
                .as_ref()
                .and_then(|p| p.pages.first())
                .map_or_else(String::new, |p| p.id.clone());
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.panel = panel;
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
                if let Some(shortcut) = crate::shortcuts::playback(&event.keystroke) {
                    if !event.is_held || shortcut.repeats() {
                        let _ = this.owner.update(cx, |owner, cx| {
                            owner.playback_shortcut(shortcut);
                            cx.notify();
                        });
                    }
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
            .child(crate::plugin_window_view::view(
                self,
                f32::from(window.viewport_size().width),
                cx,
            ))
            .child(
                div()
                    .h(px(24.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .text_size(px(10.))
                    .text_color(rgb(theme.muted))
                    .border_t_1()
                    .border_color(rgb(theme.border))
                    .child(format!(
                        "{} · r{} · Source values{} · Read only",
                        self.sync_status,
                        self.project.snapshot.revision,
                        if self
                            .details
                            .as_ref()
                            .is_some_and(|d| d.parameters.iter().any(|p| !p.automation.is_empty()))
                        {
                            " · • Automated"
                        } else {
                            ""
                        }
                    )),
            )
    }
}

pub(crate) fn resolve_panel(
    project: &ViewProject,
    details: Option<&PluginDetails>,
) -> Option<Arc<crate::plugin_layout::Layout>> {
    let details = details?;
    let descriptor = &details.info.descriptor;
    Some(
        project
            .panels
            .layouts
            .get(&(
                descriptor.plugin_id.clone(),
                descriptor.plugin_version.clone(),
            ))
            .cloned()
            .unwrap_or_else(|| Arc::new(crate::plugin_layout_builtin::panel(details))),
    )
}
pub(crate) fn sync_status(owner: &Preview) -> String {
    if let Some(error) = &owner.diagnostic {
        format!("Last good · {}", error.code)
    } else if owner.status == "Building code" {
        "Building · Last good".into()
    } else {
        "Synced".into()
    }
}
