//! Viewer-only keyboard entry; it sends transport commands, never authoring edits.
use crate::{
    model::ViewProject,
    ui::{Preview, BORDER, MUTED},
};
use gpui::{prelude::*, *};
use oxitone_core::Beat;

pub fn resolve(project: &ViewProject, text: &str) -> Result<u64, String> {
    let invalid = || "Use bar.beat (1.1), seconds (12s) or mm:ss (0:12.5)".to_owned();
    let text = text.trim();
    if let Some((minutes, seconds)) = text.split_once(':') {
        let minutes = minutes.parse::<u32>().map_err(|_| invalid())?;
        let seconds = seconds.parse::<f64>().map_err(|_| invalid())?;
        if !seconds.is_finite() || !(0.0..60.).contains(&seconds) {
            return Err(invalid());
        }
        return project
            .plan
            .tempo
            .seconds_to_frame(f64::from(minutes) * 60. + seconds)
            .map_err(|e| e.message);
    }
    if let Some(seconds) = text.strip_suffix('s') {
        return project
            .plan
            .tempo
            .seconds_to_frame(seconds.parse().map_err(|_| invalid())?)
            .map_err(|e| e.message);
    }
    let (bar, beat) = text.split_once('.').unwrap_or((text, "1"));
    let bar = bar.parse::<u32>().map_err(|_| invalid())?;
    let beat = beat.parse::<u32>().map_err(|_| invalid())?;
    if bar == 0 || beat == 0 {
        return Err(invalid());
    }
    let position = project
        .plan
        .time_signatures
        .bar_beat_to_beat(
            bar,
            Beat::new(i64::from(beat) - 1, 1).map_err(|e| e.message)?,
        )
        .map_err(|e| e.message)?;
    Ok(project.plan.tempo.beat_to_frame(position))
}

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    div()
        .id("seek-entry")
        .track_focus(&this.position_focus)
        .w(px(132.))
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(BORDER))
        .text_xs()
        .text_color(rgb(MUTED))
        .cursor_text()
        .child(if this.position_text.is_empty() {
            "Go: 1.1 / 0:12 / 12s".into()
        } else {
            this.position_text.clone()
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, _| this.position_focus.focus(window)),
        )
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            match event.keystroke.key.as_str() {
                "enter" => {
                    if let Some(project) = &this.project {
                        match resolve(project, &this.position_text) {
                            Ok(frame) => {
                                this.transport(
                                    serde_json::json!({"command":"seek","frame":frame.to_string()}),
                                );
                                this.position_text.clear();
                                window.blur();
                            }
                            Err(message) => {
                                this.diagnostic = Some(crate::model::Diagnostic {
                                    code: "PreviewPositionInvalid".into(),
                                    message,
                                    path: None,
                                })
                            }
                        }
                    }
                }
                "escape" => {
                    this.position_text.clear();
                    window.blur();
                }
                "backspace" => {
                    this.position_text.pop();
                }
                _ => {
                    if let Some(text) = &event.keystroke.key_char {
                        if this.position_text.len() < 32
                            && text
                                .chars()
                                .all(|c| c.is_ascii_digit() || ".:s".contains(c))
                        {
                            this.position_text.push_str(text);
                        }
                    }
                }
            }
            cx.stop_propagation();
            cx.notify();
        }))
}
