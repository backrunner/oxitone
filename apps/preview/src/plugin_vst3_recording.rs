//! Native recording controls submit semantic document requests only.
use crate::{
    plugin_window::PluginWindow,
    vst3_model::Command,
    vst3_recording::{Mode, Status},
};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> Div {
    let theme = this.theme;
    let mut content = div().flex().flex_col().gap_2();
    if let Some(recording) = &this.recording {
        if recording.instance_id != this.identity {
            let id = recording.id.clone();
            return content
                .child(theme.label("Recording belongs to another instance"))
                .child(
                    theme
                        .ghost("vst3-record-other-cancel", "Cancel recording")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            submit(
                                this,
                                Command::CancelRecording {
                                    recording_id: id.clone(),
                                },
                                cx,
                            )
                        })),
                );
        }
        let status = match recording.status {
            Status::Starting => "Starting recording…",
            Status::Recording => "Recording · keep transport playing until Stop completes",
            Status::Stopping => "Stopping · waiting for audio boundary…",
            Status::Captured => "Take captured · acceptance failed",
            Status::Failed => "Recording failed",
        };
        content = content.child(theme.label(status));
        if let Some(error) = &recording.error {
            content = content.child(theme.label(error.message.clone()));
        }
        let id = recording.id.clone();
        let mut actions = div().flex().gap_2();
        if matches!(recording.status, Status::Recording | Status::Captured) {
            let stop_id = id.clone();
            actions = actions.child(
                theme
                    .ghost(
                        "vst3-record-stop",
                        if recording.status == Status::Captured {
                            "Retry acceptance"
                        } else {
                            "Stop recording"
                        },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        submit(
                            this,
                            Command::StopRecording {
                                recording_id: stop_id.clone(),
                            },
                            cx,
                        )
                    })),
            );
        }
        return content.child(
            actions.child(
                theme
                    .ghost("vst3-record-cancel", "Cancel recording")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        submit(
                            this,
                            Command::CancelRecording {
                                recording_id: id.clone(),
                            },
                            cx,
                        )
                    })),
            ),
        );
    }
    content = content.child(
        theme
            .ghost("vst3-record-expand", "Record automation")
            .on_click(cx.listener(|this, _, _, cx| {
                this.recording_selection.expanded = !this.recording_selection.expanded;
                cx.notify();
            })),
    );
    if !this.recording_selection.expanded {
        return content;
    }
    let Some(details) = &this.details else {
        return content;
    };
    let parameters: Vec<_> = details
        .parameters
        .iter()
        .filter(|p| !p.host && p.spec.automation == Some(true))
        .filter_map(|p| {
            p.spec
                .id
                .parse::<u32>()
                .ok()
                .map(|id| (id, p.spec.label.clone()))
        })
        .collect();
    if parameters.is_empty() {
        return content.child(theme.label("No recordable parameters"));
    }
    let mut modes = div().flex().gap_2();
    for (id, label, mode) in [
        ("vst3-record-touch", "Touch", Mode::Touch),
        ("vst3-record-write", "Write", Mode::Write),
    ] {
        let selected = this.recording_selection.mode == mode;
        modes = modes.child(
            theme
                .ghost(id, format!("{}{}", if selected { "● " } else { "" }, label))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.recording_selection.mode = mode;
                    cx.notify();
                })),
        );
    }
    content = content.child(modes).child(theme.label(format!(
        "{} / 32 parameters armed",
        this.recording_selection.parameters.len()
    )));
    let pages = parameters.len().div_ceil(16);
    let page = this.recording_selection.page.min(pages - 1);
    let mut choices = div().flex().flex_wrap().gap_2();
    for (id, name) in parameters.iter().skip(page * 16).take(16) {
        let id = *id;
        let selected = this.recording_selection.parameters.contains(&id);
        choices = choices.child(
            theme
                .ghost(
                    SharedString::from(format!("vst3-record-param-{id}")),
                    format!("{}{}", if selected { "● " } else { "" }, name),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.recording_selection.parameters.remove(&id)
                        && this.recording_selection.parameters.len() < 32
                    {
                        this.recording_selection.parameters.insert(id);
                    }
                    cx.notify();
                })),
        );
    }
    content = content.child(choices);
    if pages > 1 {
        let mut navigation = div().flex().gap_2();
        for (id, label, next) in [
            ("vst3-record-prev", "Previous", page.saturating_sub(1)),
            ("vst3-record-next", "Next", (page + 1).min(pages - 1)),
        ] {
            navigation = navigation.child(theme.ghost(id, label).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.recording_selection.page = next;
                    cx.notify();
                },
            )));
        }
        content = content.child(navigation.child(theme.label(format!("{} / {pages}", page + 1))));
    }
    let enabled = !this.recording_selection.parameters.is_empty()
        && this.owner.upgrade().is_some_and(|owner| {
            let owner = owner.read(cx);
            owner.document_ready() && owner.plugin_configuration_target(&this.target).is_some()
        });
    content.child(
        theme
            .ghost("vst3-record-start", "Start recording")
            .opacity(if enabled { 1. } else { 0.4 })
            .on_click(cx.listener(move |this, _, _, cx| {
                if !enabled {
                    return;
                }
                let target = this.target.clone();
                let identity = this.identity.clone();
                let mode = this.recording_selection.mode;
                let parameter_ids = this
                    .recording_selection
                    .parameters
                    .iter()
                    .copied()
                    .collect();
                let _ = this.owner.update(cx, |owner, cx| {
                    if owner.document_ready()
                        && owner
                            .project
                            .as_ref()
                            .is_some_and(|p| crate::plugin_identity::key(p, &target) == identity)
                    {
                        if let Some((site, usage)) = owner.plugin_configuration_target(&target) {
                            owner.document_request(crate::document_wire::DocumentOperation::Vst3 {
                                command: Command::StartRecording {
                                    site,
                                    usage,
                                    mode,
                                    parameter_ids,
                                },
                            });
                        }
                    }
                    cx.notify();
                });
            })),
    )
}
fn submit(this: &PluginWindow, command: Command, cx: &mut Context<PluginWindow>) {
    let _ = this.owner.update(cx, |owner, cx| {
        owner.document_request(crate::document_wire::DocumentOperation::Vst3 { command });
        cx.notify();
    });
}
