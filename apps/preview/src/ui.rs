use crate::{
    analysis::AnalysisMap,
    backend::{Backend, Command},
    model::{Diagnostic, PlaybackStatus, UiEvent, ViewProject},
    wire::Frame,
};
use gpui::{prelude::*, *};
use serde_json::json;
use std::{sync::Arc, time::Duration};

pub const BG: u32 = 0x0d1118;
pub const PANEL: u32 = 0x151b25;
pub const BORDER: u32 = 0x293240;
pub const TEXT: u32 = 0xdce4ee;
pub const MUTED: u32 = 0x8492a6;
pub const ACCENT: u32 = 0x69c7b8;
pub const GOLD: u32 = 0xe8b977;

pub struct Preview {
    backend: Backend,
    pub project: Option<Arc<ViewProject>>,
    pub playback: PlaybackStatus,
    pub analysis: AnalysisMap,
    pub selected_clip: Option<String>,
    pub selected_scope: String,
    pub zoom: f32,
    pub loop_enabled: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    pub status: String,
    pub diagnostic: Option<Diagnostic>,
    pub show_scopes: bool,
    pub position_focus: FocusHandle,
    pub position_text: String,
}

impl Preview {
    pub fn new(backend: Backend, cx: &mut Context<Self>) -> Self {
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
            project: None,
            playback: PlaybackStatus::default(),
            analysis: AnalysisMap::new(),
            selected_clip: None,
            selected_scope: "mix_master".into(),
            zoom: 17.0,
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 16.0,
            status: "Waiting for code".into(),
            diagnostic: None,
            show_scopes: true,
            position_focus: cx.focus_handle(),
            position_text: String::new(),
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
                        self.selected_clip = project
                            .snapshot
                            .pattern_clips
                            .first()
                            .map(|clip| clip.id.clone());
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
                UiEvent::Diagnostic(diagnostic) => self.diagnostic = Some(diagnostic),
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
                UiEvent::Playback(status) => self.playback = status,
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
    pub fn seek(&self, beat: f64) {
        self.transport(json!({"command":"seek","beat":beat_wire(beat)}));
    }
    pub fn play(&self) {
        let mut command = json!({"command":"play"});
        if self.loop_enabled {
            if let Some(project) = &self.project {
                let frame = |beat| {
                    project
                        .plan
                        .tempo
                        .beat_to_frame(oxitone_core::Beat::from_f64(beat).unwrap())
                };
                command["loopRegion"] = json!({"startFrame":frame(self.loop_start).to_string(),"endFrame":frame(self.loop_end).to_string()});
            }
        }
        self.transport(command);
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family("Helvetica Neue")
            .child(self.header(cx))
            .child(self.transport_bar(cx));
        if let Some(diagnostic) = &self.diagnostic {
            root = root.child(
                div()
                    .px_5()
                    .py_2()
                    .bg(rgb(0x3b272a))
                    .text_sm()
                    .text_color(rgb(0xf0baad))
                    .child(format!(
                        "{} · {}  {}",
                        diagnostic.code,
                        diagnostic.message,
                        diagnostic.path.as_deref().unwrap_or("")
                    )),
            );
        }
        if self.project.is_some() {
            root = root.child(crate::arrangement::view(self, cx)).child(
                div()
                    .flex()
                    .h(px(290.))
                    .min_h(px(200.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(crate::piano::view(self, cx))
                    .child(crate::mixer::view(self, cx)),
            );
            if self.show_scopes {
                root = root.child(crate::scopes::view(self));
            }
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
                    .child(div().text_sm().text_color(rgb(MUTED)).child(
                        "Your tracks, notes and mixer appear after the first successful build.",
                    )),
            );
        }
        root.child(self.footer())
    }
}

pub fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .px_3()
        .py_1()
        .rounded_sm()
        .bg(rgb(0x252f3e))
        .text_xs()
        .cursor_pointer()
        .child(label.into())
}
pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(text.into())
}
pub fn color(index: usize) -> u32 {
    [0x69c7b8, 0xe8b977, 0x9d9ce4, 0x6ca8ce, 0xcd879b][index % 5]
}
pub fn alpha(value: u32, alpha: f32) -> Rgba {
    Rgba {
        a: alpha,
        ..rgb(value)
    }
}
fn beat_wire(beat: f64) -> serde_json::Value {
    let beat = oxitone_core::Beat::from_f64(beat.max(0.0)).unwrap();
    serde_json::to_value(beat).unwrap()
}
