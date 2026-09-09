//! View preferences and pointer gestures never change the authoring snapshot.
use gpui::*;

#[derive(Clone, Copy)]
pub enum Axis {
    Horizontal,
    Vertical,
}
impl Axis {
    pub fn coordinate(self, p: Point<Pixels>) -> f32 {
        f32::from(match self {
            Self::Horizontal => p.x,
            Self::Vertical => p.y,
        })
    }
    pub fn extent(self, s: Size<Pixels>) -> f32 {
        f32::from(match self {
            Self::Horizontal => s.width,
            Self::Vertical => s.height,
        })
    }
}

pub enum Gesture {
    PianoPan {
        pointer: Point<Pixels>,
        offset: (f32, f32),
        button: MouseButton,
    },
    Scroll {
        handle: ScrollHandle,
        axis: Axis,
        pointer: f32,
        offset: Point<Pixels>,
        scale: f32,
    },
    PianoScroll {
        axis: Axis,
        pointer: f32,
        offset: (f32, f32),
        scale: f32,
    },
    Resize {
        pointer: Point<Pixels>,
        region: crate::workspace_resize::Resize,
    },
}

pub struct Workspace {
    pub arrangement: ScrollHandle,
    pub mixer: ScrollHandle,
    pub inspector: ScrollHandle,
    pub gesture: Option<Gesture>,
    pub editor_height: f32,
    pub piano_fraction: f32,
    pub inspector_open: bool,
    pub inspector_tab: crate::mixer_inspector::InspectorTab,
    pub mode: crate::workspace_layout::EditorMode,
    pub dock: crate::workspace_layout::DockMode,
    pub dock_open: bool,
    pub scopes_height: f32,
    pub inspector_width: f32,
    pub mixer_flow_open: bool,
    pub bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub dividers:
        std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, Bounds<Pixels>>>>,
    pub playlist_fitted: bool,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            arrangement: ScrollHandle::new(),
            mixer: ScrollHandle::new(),
            inspector: ScrollHandle::new(),
            gesture: None,
            editor_height: 330.,
            piano_fraction: 0.6,
            inspector_open: false,
            inspector_tab: crate::mixer_inspector::InspectorTab::Routing,
            mode: Default::default(),
            dock: Default::default(),
            dock_open: true,
            scopes_height: 120.,
            inspector_width: 240.,
            mixer_flow_open: false,
            bounds: Default::default(),
            dividers: Default::default(),
            playlist_fitted: false,
        }
    }
}

pub use crate::workspace_panels::panels;

impl crate::ui::Preview {
    pub fn move_gesture(&mut self, event: &MouseMoveEvent, _window: &Window) {
        let button = match &self.workspace.gesture {
            Some(Gesture::PianoPan { button, .. }) => *button,
            _ => MouseButton::Left,
        };
        if event.pressed_button != Some(button) {
            self.workspace.gesture = None;
            return;
        }
        let Some(gesture) = &self.workspace.gesture else {
            return;
        };
        match gesture {
            Gesture::PianoPan {
                pointer, offset, ..
            } => {
                if let Some(l) = self.piano_layout() {
                    let delta = event.position - *pointer;
                    self.piano.set_offset((
                        (offset.0 - f32::from(delta.x)).clamp(0., l.max_x),
                        (offset.1 - f32::from(delta.y)).clamp(0., l.max_y),
                    ));
                }
            }
            Gesture::Scroll {
                handle,
                axis,
                pointer,
                offset,
                scale,
            } => {
                let value = (axis.coordinate(*offset)
                    - (axis.coordinate(event.position) - pointer) * scale)
                    .clamp(-axis.extent(handle.max_offset()), 0.);
                let at = match axis {
                    Axis::Horizontal => point(px(value), offset.y),
                    Axis::Vertical => point(offset.x, px(value)),
                };
                handle.set_offset(at);
            }
            Gesture::PianoScroll {
                axis,
                pointer,
                offset,
                scale,
            } => {
                if let Some(layout) = self.piano_layout() {
                    let delta = (axis.coordinate(event.position) - pointer) * scale;
                    self.piano.set_offset(match axis {
                        Axis::Horizontal => ((offset.0 + delta).clamp(0., layout.max_x), offset.1),
                        Axis::Vertical => (offset.0, (offset.1 + delta).clamp(0., layout.max_y)),
                    });
                }
            }
            Gesture::Resize { pointer, region } => {
                let pointer = *pointer;
                let region = *region;
                crate::workspace_resize::apply(self, region, event.position - pointer);
            }
        }
    }
}

/// Thumb geometry is shared by all scrollbars; a full track represents no overflow.
pub fn thumb(track: f32, viewport: f32, max: f32, offset: f32) -> (f32, f32) {
    let length = (track * viewport / (viewport + max).max(1.)).clamp(18_f32.min(track), track);
    let start = if max > 0. {
        (-offset / max).clamp(0., 1.) * (track - length)
    } else {
        0.
    };
    (start, length)
}

#[cfg(test)]
mod tests {
    use super::thumb;
    #[test]
    fn scrollbar_maps_content_edges_and_clamps_stale_offsets() {
        assert_eq!(thumb(100., 100., 300., 0.), (0., 25.));
        assert_eq!(thumb(100., 100., 300., -300.), (75., 25.));
        assert_eq!(thumb(100., 100., 300., -900.), (75., 25.));
        assert_eq!(thumb(100., 100., 0., 0.), (0., 100.));
    }
}
