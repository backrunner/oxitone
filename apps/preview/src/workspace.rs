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
    Height {
        pointer: f32,
        height: f32,
    },
    Split {
        pointer: f32,
        fraction: f32,
        width: f32,
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
    pub mixer_expanded: bool,
    pub playlist_fitted: bool,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            arrangement: ScrollHandle::new(),
            mixer: ScrollHandle::new(),
            inspector: ScrollHandle::new(),
            gesture: None,
            editor_height: 404.,
            piano_fraction: 0.45,
            inspector_open: true,
            inspector_tab: crate::mixer_inspector::InspectorTab::Routing,
            mixer_expanded: false,
            playlist_fitted: false,
        }
    }
}

pub fn panels(
    this: &mut crate::ui::Preview,
    window: &Window,
    cx: &mut Context<crate::ui::Preview>,
) -> impl IntoElement {
    use gpui::prelude::*;
    let theme = this.theme;
    let size = window.viewport_size();
    let height = this
        .workspace
        .editor_height
        .min((f32::from(size.height) - 390.).max(240.));
    let width = f32::from(size.width);
    let expanded = this.workspace.mixer_expanded;
    let fraction = this
        .workspace
        .piano_fraction
        .clamp(420. / width, 1. - 526. / width);
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(crate::arrangement::view(this, width, cx))
        .child(
            div()
                .id("editor-height-divider")
                .h(px(6.))
                .flex_shrink_0()
                .bg(rgb(theme.bg))
                .border_t_1()
                .border_color(rgb(theme.border))
                .cursor(CursorStyle::ResizeUpDown)
                .hover(move |s| s.bg(rgb(theme.selected)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.workspace.gesture = Some(Gesture::Height {
                            pointer: f32::from(event.position.y),
                            height,
                        });
                        cx.stop_propagation();
                    }),
                ),
        )
        .child(
            div()
                .h(px(height))
                .flex_shrink_0()
                .flex()
                .overflow_hidden()
                .when(!expanded, |d| {
                    d.child(
                        div()
                            .w(relative(fraction))
                            .flex_shrink_0()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .child(crate::piano::view(this, width * fraction, cx)),
                    )
                    .child(
                        div()
                            .id("editor-width-divider")
                            .w(px(6.))
                            .flex_shrink_0()
                            .bg(rgb(theme.bg))
                            .cursor(CursorStyle::ResizeLeftRight)
                            .hover(move |s| s.bg(rgb(theme.selected)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.workspace.gesture = Some(Gesture::Split {
                                        pointer: f32::from(event.position.x),
                                        fraction,
                                        width,
                                    });
                                    cx.stop_propagation();
                                }),
                            ),
                    )
                })
                .child(crate::mixer::view(
                    this,
                    height,
                    if expanded {
                        width
                    } else {
                        width * (1. - fraction) - 6.
                    },
                    cx,
                )),
        )
        .when(this.show_scopes, |d| {
            d.child(crate::scopes::view(
                this,
                if f32::from(size.height) < 820. {
                    88.
                } else {
                    96.
                },
            ))
        })
}

impl crate::ui::Preview {
    pub fn move_gesture(&mut self, event: &MouseMoveEvent, window: &Window) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.workspace.gesture = None;
            return;
        }
        let Some(gesture) = &self.workspace.gesture else {
            return;
        };
        match gesture {
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
                    self.piano.offset = Some(match axis {
                        Axis::Horizontal => ((offset.0 + delta).clamp(0., layout.max_x), offset.1),
                        Axis::Vertical => (offset.0, (offset.1 + delta).clamp(0., layout.max_y)),
                    });
                }
            }
            Gesture::Height { pointer, height } => {
                self.workspace.editor_height = (height - (f32::from(event.position.y) - pointer))
                    .clamp(
                        240.,
                        (f32::from(window.viewport_size().height) - 390.).max(240.),
                    );
            }
            Gesture::Split {
                pointer,
                fraction,
                width,
            } => {
                self.workspace.piano_fraction = (fraction
                    + (f32::from(event.position.x) - pointer) / width)
                    .clamp(420. / width, 1. - 526. / width);
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
