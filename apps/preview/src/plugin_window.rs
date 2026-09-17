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
    pub identity: String,
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
    pub choice_open: Option<String>,
    pub expanded_groups: std::collections::HashSet<String>,
    pub plots: Arc<Vec<crate::plugin_plot::Plot>>,
    pub parameter_bounds:
        std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, Bounds<Pixels>>>>,
    edit_stamp: u64,
    projected: bool,
    source_ready: bool,
    pub sync_status: String,
    pub(super) owner: WeakEntity<Preview>,
    pub width: f32,
    pub request_focus: bool,
    pub source_button: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub(super) focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl PluginWindow {
    pub fn focus(&self, window: &mut Window) {
        self.focus.focus(window);
    }
    pub(crate) fn new(
        target: DetailTarget,
        project: Arc<ViewProject>,
        owner: Entity<Preview>,
        status: String,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let project_changes = cx.observe(&owner, |this, owner, cx| {
            let owner = owner.read(cx);
            let status = sync_status(owner);
            let mut changed = status != this.sync_status
                || this.theme.bg != owner.theme.bg
                || this.source_ready != owner.document_ready();
            this.source_ready = owner.document_ready();
            let mut parameters_changed = false;
            this.theme = owner.theme;
            this.sync_status = status;
            if let Some(project) = &owner.project {
                if !Arc::ptr_eq(project, &this.project) {
                    this.refresh(project.clone());
                    this.theme = owner.theme;
                    changed = true;
                    parameters_changed = true;
                }
            }
            if this.edit_stamp != owner.document.plugin.stamp {
                this.edit_stamp = owner.document.plugin.stamp;
                parameters_changed |= this.projected
                    || owner
                        .document
                        .plugin
                        .change()
                        .is_some_and(|c| c.identity == this.identity);
            }
            if parameters_changed {
                this.project_edit(owner);
            }
            if changed || parameters_changed {
                cx.notify();
            }
        });
        let focus = cx.focus_handle();
        let details = plugin_details::resolve(&project, &target);
        let panel = resolve_panel(&project, details.as_ref());
        let plots = Arc::new(
            details
                .as_ref()
                .map_or_else(Vec::new, |d| crate::plugin_plot::build(d, &project)),
        );
        Self {
            identity: crate::plugin_identity::key(&project, &target),
            plots,
            parameter_bounds: Default::default(),
            edit_stamp: 0,
            projected: false,
            source_ready: false,
            stacked_waveforms: true,
            choice_open: None,
            expanded_groups: Default::default(),
            page: panel
                .as_ref()
                .and_then(|p| p.pages.first())
                .map_or_else(String::new, |p| p.id.clone()),
            panel,
            sync_status: status,
            details,
            target,
            project,
            theme,
            width: 820.,
            request_focus: true,
            source_button: Default::default(),
            tab: DetailTab::Panel,
            filter: ParameterFilter::All,
            scroll: ScrollHandle::new(),
            scroll_drag: None,
            copied: false,
            parameter_specs: false,
            owner: owner.downgrade(),
            focus,
            _subscriptions: vec![project_changes],
        }
    }
    fn refresh(&mut self, project: Arc<ViewProject>) {
        self.details = crate::plugin_identity::follow(&project, &self.target, &self.identity)
            .and_then(|target| {
                self.target = target;
                plugin_details::resolve(&project, &self.target)
            });
        let panel = resolve_panel(&project, self.details.as_ref());
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
            self.choice_open = None;
            self.expanded_groups.clear();
            self.page = panel
                .as_ref()
                .and_then(|p| p.pages.first())
                .map_or_else(String::new, |p| p.id.clone());
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.panel = panel;
        self.project = project;
        self.copied = false;
    }
    fn project_edit(&mut self, owner: &Preview) {
        self.projected = false;
        self.details = crate::plugin_identity::follow(&self.project, &self.target, &self.identity)
            .and_then(|target| plugin_details::resolve(&self.project, &target));
        if let Some(details) = &mut self.details {
            if let Some(change) = owner
                .document
                .plugin
                .change()
                .filter(|c| c.identity == self.identity)
                .filter(|_| owner.document.plugin.gesture.is_some() || owner.presentation_active())
            {
                if let Some(parameter) = details
                    .parameters
                    .iter_mut()
                    .find(|p| p.host == change.host && p.spec.id == change.parameter)
                {
                    parameter.value = change.value;
                    parameter.explicit = true;
                    self.projected = true;
                }
            }
            self.plots = Arc::new(crate::plugin_plot::build(details, &self.project));
        } else {
            self.plots = Arc::new(vec![]);
        }
    }
    pub fn title(&self) -> String {
        self.details.as_ref().map_or_else(
            || format!("{} — Not attached", self.target.label()),
            |d| {
                d.target.slot().map_or_else(
                    || format!("{} — {}", d.name, d.owner_name),
                    |slot| format!("{} — {}, insert {}", d.name, d.owner_name, slot + 1),
                )
            },
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
    if let Some(error) = owner.active_diagnostic() {
        format!("Last good · {}", error.code)
    } else if owner.status == "Building code"
        || owner
            .document
            .view
            .as_ref()
            .is_some_and(|v| v.status == "building")
    {
        "Building · Last good".into()
    } else {
        "Synced".into()
    }
}
