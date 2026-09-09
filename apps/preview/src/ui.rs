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
    pub(crate) backend: Backend,
    pub document: crate::document_ui::DocumentUi,
    pub theme: Theme,
    pub project: Option<Arc<ViewProject>>,
    pub playback: PlaybackStatus,
    pub cue_frame: u64,
    pub requested_playing: Option<bool>,
    pub requested_position: Option<(u64, u64)>,
    pub show_shortcuts: bool,
    pub status_details: bool,
    pub close: crate::close_state::CloseState,
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
    pub piano_focus: FocusHandle,
    pub mixer_focus: FocusHandle,
    pub inspector_focus: FocusHandle,
    pub workspace_focus: FocusHandle,
    pub plugin_windows:
        std::collections::HashMap<String, Entity<crate::plugin_window::PluginWindow>>,
}

impl Preview {
    pub fn new(backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let owner = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            owner
                .update(cx, |this, cx| this.request_close(window, cx))
                .unwrap_or(true)
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
            document: Default::default(),
            theme: Theme::from_appearance(window.appearance()),
            project: None,
            playback: PlaybackStatus::default(),
            cue_frame: 0,
            requested_playing: None,
            requested_position: None,
            show_shortcuts: false,
            status_details: false,
            close: Default::default(),
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
            show_scopes: false,
            piano_focus: cx.focus_handle(),
            mixer_focus: cx.focus_handle(),
            inspector_focus: cx.focus_handle(),
            workspace_focus,
            plugin_windows: Default::default(),
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        while let Ok(event) = self.backend.events.try_recv() {
            match event {
                UiEvent::Document(message) => self.observe_document(message),
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
                    if !self.project.as_ref().is_some_and(|old| {
                        std::sync::Arc::ptr_eq(&old.telemetry, &project.telemetry)
                    }) {
                        self.analysis.clear();
                    }
                    self.project = Some(project);
                    self.settle_presentation();
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
        if self
            .close
            .complete(self.document.view.as_ref(), self.document.pending.is_some())
        {
            self.close.cancel();
            cx.quit();
            return;
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

pub fn alpha(value: u32, alpha: f32) -> Rgba {
    Rgba {
        a: alpha,
        ..rgb(value)
    }
}
