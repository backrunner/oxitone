use crate::{
    analysis::AnalysisMap,
    backend::{Backend, Command},
    model::{Diagnostic, PlaybackStatus, UiEvent, ViewProject},
    theme::Theme,
    wire::Frame,
};
use gpui::{prelude::*, *};
use std::{sync::Arc, time::Duration};

pub struct Preview {
    backend: Backend,
    pub theme: Theme,
    pub project: Option<Arc<ViewProject>>,
    pub playback: PlaybackStatus,
    pub cue_frame: u64,
    pub requested_playing: Option<bool>,
    pub requested_position: Option<(u64, u64)>,
    pub show_shortcuts: bool,
    pub analysis: AnalysisMap,
    pub selected_clip: Option<String>,
    pub selected_scope: String,
    pub zoom: f32,
    pub piano: crate::piano_layout::PianoState,
    pub workspace: crate::workspace::Workspace,
    pub loop_enabled: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    pub status: String,
    pub diagnostic: Option<Diagnostic>,
    pub show_scopes: bool,
    pub position_focus: FocusHandle,
    pub piano_focus: FocusHandle,
    pub mixer_focus: FocusHandle,
    pub inspector_focus: FocusHandle,
    pub workspace_focus: FocusHandle,
    pub position_text: String,
    pub plugin_windows: std::collections::HashMap<
        crate::plugin_details::DetailTarget,
        WindowHandle<crate::plugin_window::PluginWindow>,
    >,
}

impl Preview {
    pub fn new(backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.on_window_should_close(cx, |_, cx| {
            cx.defer(|cx| cx.quit());
            true
        });
        let workspace_focus = cx.focus_handle();
        workspace_focus.focus(window);
        crate::capture::schedule(window, cx);
        cx.observe_window_appearance(window, |this, window, cx| {
            this.theme = Theme::from_appearance(window.appearance());
            cx.notify();
        })
        .detach();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(33))
                .await;
            if this
                .update(cx, |view, cx| {
                    view.poll(cx);
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
        Self {
            backend,
            theme: Theme::from_appearance(window.appearance()),
            project: None,
            playback: PlaybackStatus::default(),
            cue_frame: 0,
            requested_playing: None,
            requested_position: None,
            show_shortcuts: false,
            analysis: AnalysisMap::new(),
            selected_clip: None,
            selected_scope: "mix_master".into(),
            zoom: 17.0,
            piano: crate::piano_layout::PianoState::default(),
            workspace: crate::workspace::Workspace::default(),
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 16.0,
            status: "Waiting for code".into(),
            diagnostic: None,
            show_scopes: true,
            position_focus: cx.focus_handle(),
            piano_focus: cx.focus_handle(),
            mixer_focus: cx.focus_handle(),
            inspector_focus: cx.focus_handle(),
            workspace_focus,
            position_text: String::new(),
            plugin_windows: Default::default(),
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        while let Ok(event) = self.backend.events.try_recv() {
            match event {
                UiEvent::Accepted(project) => {
                    if !project
                        .snapshot
                        .pattern_clips
                        .iter()
                        .any(|clip| Some(&clip.id) == self.selected_clip.as_ref())
                    {
                        self.selected_clip = project.initial_clip().map(|clip| clip.id.clone());
                    }
                    if self.project.is_none() {
                        self.loop_end = project.plan.content_end_beat.to_f64().max(4.0);
                    }
                    if !project
                        .telemetry
                        .channels
                        .iter()
                        .chain(&project.telemetry.buses)
                        .any(|node| node.id == self.selected_scope)
                    {
                        self.selected_scope = "mix_master".into();
                    }
                    self.project = Some(project);
                    self.analysis.clear();
                    self.diagnostic = None;
                    self.status = "Code up to date".into();
                }
                UiEvent::Diagnostic(diagnostic) => {
                    self.requested_playing = None;
                    self.requested_position = None;
                    self.diagnostic = Some(diagnostic);
                }
                UiEvent::Status(status) => {
                    if status == "watching"
                        && self
                            .diagnostic
                            .as_ref()
                            .is_some_and(|d| d.code == "PreviewBuildFailed")
                    {
                        self.diagnostic = None;
                    }
                    self.status = if status == "building" {
                        "Building code"
                    } else {
                        "Code up to date"
                    }
                    .into();
                }
                UiEvent::Playback(status) => self.observe_transport(status),
                UiEvent::Shutdown => cx.quit(),
            }
        }
        if let Some(project) = &self.project {
            let epoch = project
                .telemetry
                .epoch
                .load(std::sync::atomic::Ordering::Relaxed);
            for node in project
                .telemetry
                .channels
                .iter()
                .chain(project.telemetry.buses.iter())
            {
                self.analysis.entry(node.id.clone()).or_default().update(
                    node,
                    self.playback.audible,
                    epoch,
                    self.playback.playing,
                );
            }
        }
    }

    pub fn transport(&self, command: serde_json::Value) {
        let _ = self
            .backend
            .commands
            .send(Command::Frame(Frame::Transport { command }, None));
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut root = div()
            .id("preview-workspace")
            .track_focus(&self.workspace_focus)
            .on_key_down(cx.listener(Self::workspace_key))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme.bg))
            .text_color(rgb(theme.text))
            .font_family("Helvetica Neue")
            .on_mouse_move(cx.listener(|this, event, window, cx| {
                if this.workspace.gesture.is_some() {
                    this.move_gesture(event, window);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.workspace.gesture = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.workspace.gesture = None),
            )
            .child(self.header(window, cx))
            .child(self.transport_bar(cx));
        if let Some(diagnostic) = &self.diagnostic {
            root = root.child(
                div()
                    .px_5()
                    .py_2()
                    .bg(rgb(theme.diagnostic_bg))
                    .text_sm()
                    .text_color(rgb(theme.diagnostic_text))
                    .child(format!(
                        "{} · {}  {}",
                        diagnostic.code,
                        diagnostic.message,
                        diagnostic.path.as_deref().unwrap_or("")
                    )),
            );
        }
        if self.project.is_some() {
            root = root.child(crate::workspace::panels(self, window, cx));
        } else {
            root = root.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap_4()
                    .child(div().text_2xl().child("Music, written in code."))
                    .child(div().text_sm().text_color(rgb(theme.muted)).child(
                        "Your tracks, notes and mixer appear after the first successful build.",
                    )),
            );
        }
        root.child(self.footer()).when(self.show_shortcuts, |d| {
            d.child(crate::shortcut_help::view(self, cx))
        })
    }
}

pub fn alpha(value: u32, alpha: f32) -> Rgba {
    Rgba {
        a: alpha,
        ..rgb(value)
    }
}
