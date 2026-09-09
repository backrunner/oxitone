//! Contextual musical values and cursors; shortcuts live in tooltips and help.
use crate::{note_transform::NoteAction, piano_layout::*, ui::Preview, workspace::Gesture};
use gpui::{prelude::*, *};

pub fn cursor(this: &Preview) -> CursorStyle {
    if matches!(this.workspace.gesture, Some(Gesture::PianoPan { .. })) {
        return CursorStyle::ClosedHand;
    }
    if let Some(g) = &this.document.gesture {
        return match g.action {
            NoteAction::Resize | NoteAction::InsertResize => CursorStyle::ResizeLeftRight,
            NoteAction::Velocity => CursorStyle::Crosshair,
            NoteAction::Duplicate => CursorStyle::DragCopy,
            NoteAction::Move | NoteAction::Insert => CursorStyle::ClosedHand,
            _ => CursorStyle::Crosshair,
        };
    }
    let Some(l) = this.piano_layout() else {
        return CursorStyle::Arrow;
    };
    let Some(at) = this.piano.pointer.map(|p| p - this.piano.origin.get()) else {
        return CursorStyle::Arrow;
    };
    let (x, y) = (f32::from(at.x), f32::from(at.y));
    if x < KEY_WIDTH || y < RULER {
        return CursorStyle::Arrow;
    }
    if y >= RULER + l.grid_height {
        return CursorStyle::Crosshair;
    }
    if let Some(site) = this.pattern_site() {
        for output in site.outputs.iter().rev() {
            let n = &output.note;
            if x >= l.x(n.start)
                && x <= l.x(n.start + n.duration).max(l.x(n.start) + 7.)
                && y >= l.y(n.pitch)
                && y < l.y(n.pitch) + l.key_height
            {
                let edge = (l.beat_width * n.duration as f32 * 0.3).clamp(2., 8.);
                return if x > l.x(n.start + n.duration) - edge {
                    CursorStyle::ResizeLeftRight
                } else {
                    CursorStyle::OpenHand
                };
            }
        }
    }
    CursorStyle::Crosshair
}

pub fn status(this: &Preview, total: usize) -> Div {
    let t = this.theme;
    let count = this.document.notes.indices.len();
    let label = if let Some(g) = this
        .document
        .gesture
        .as_ref()
        .filter(|g| g.action == NoteAction::Erase)
    {
        format!("{} erased", g.changes.len())
    } else if count > 0 {
        format!("{count} selected")
    } else {
        format!("{total} notes")
    };
    let values = this
        .document
        .gesture
        .as_ref()
        .and_then(|g| {
            if g.action == NoteAction::Velocity {
                let l = this.piano_layout()?;
                let y = f32::from(g.pointer.y - this.piano.origin.get().y);
                return Some(format!(
                    "Velocity {}",
                    (crate::note_velocity::velocity_at(l, y) * 127.).round() as u8
                ));
            }
            let c = g.changes.first()?;
            Some(format!(
                "{}    Start {:.3}    Length {:.3}    Vel {}",
                note_name(c.after.pitch),
                c.after.start,
                c.after.duration,
                (c.after.velocity * 127.).round() as u8
            ))
        })
        .or_else(|| {
            let l = this.piano_layout()?;
            let at = this.piano.pointer? - this.piano.origin.get();
            let (x, y) = (f32::from(at.x), f32::from(at.y));
            if x < KEY_WIDTH || y < RULER || y >= RULER + l.grid_height {
                return None;
            }
            let pitch =
                (127. - ((y - RULER + l.scroll_y) / l.key_height).floor()).clamp(0., 127.) as u8;
            Some(format!("{}    Beat {:.3}", note_name(pitch), l.beat(x)))
        })
        .unwrap_or_default();
    div()
        .h(px(22.))
        .flex_shrink_0()
        .px_3()
        .flex()
        .items_center()
        .gap_3()
        .bg(rgb(t.panel))
        .text_size(px(10.))
        .text_color(rgb(t.muted))
        .child(label)
        .child(div().flex_1().min_w_0().truncate().child(values))
}
