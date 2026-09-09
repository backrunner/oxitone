//! Workspace-local window state; never owns platform windows or audio instances.
use gpui::*;
use std::{cell::Cell, collections::BTreeMap, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowId {
    Patterns,
    Automation,
    Plugins,
    Configuration,
    Piano,
    Mixer,
    Plugin(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl WindowBounds {
    pub fn fit(self, viewport: Size<Pixels>) -> Self {
        let w = f32::from(viewport.width).max(1.);
        let h = f32::from(viewport.height).max(1.);
        let width = self.width.clamp(1., w);
        let height = self.height.clamp(1., h);
        Self {
            x: self.x.clamp(0., w - width),
            y: self.y.clamp(0., h - height),
            width,
            height,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct WindowState {
    pub bounds: WindowBounds,
    pub maximized: bool,
    pub z: u64,
}
#[derive(Clone, Copy, Debug)]
pub enum GestureKind {
    Move,
    Resize {
        left: bool,
        right: bool,
        top: bool,
        bottom: bool,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct WindowDrag {
    pub id: WindowId,
    pub anchor: Point<Pixels>,
    pub origin: WindowBounds,
    pub kind: GestureKind,
}
#[derive(Default)]
pub struct WindowManager {
    states: BTreeMap<WindowId, WindowState>,
    pub next_z: u64,
    pub visible: Vec<WindowId>,
    pub hidden: bool,
    pub drag: Option<WindowDrag>,
    pub desktop: Rc<Cell<Bounds<Pixels>>>,
    pub piano_open: bool,
    pub mixer_open: bool,
}
impl WindowManager {
    pub fn front(&self) -> Option<WindowId> {
        if self.hidden {
            return None;
        }
        self.visible
            .iter()
            .copied()
            .max_by_key(|id| self.state(*id).z)
    }
    pub fn forget(&mut self, id: WindowId) {
        self.states.remove(&id);
        self.visible.retain(|key| *key != id);
    }
    pub fn hit(&self, position: Point<Pixels>) -> Option<WindowId> {
        if self.hidden {
            return None;
        }
        let point = position - self.desktop.get().origin;
        self.visible.iter().rev().copied().find(|id| {
            let b = self.bounds(*id);
            Bounds {
                origin: gpui::point(px(b.x), px(b.y)),
                size: size(px(b.width), px(b.height)),
            }
            .contains(&point)
        })
    }
    pub fn state(&self, id: WindowId) -> WindowState {
        self.states.get(&id).copied().unwrap_or_else(|| {
            let (x, y, width, height) = match id {
                WindowId::Patterns => (12., 12., 244., 420.),
                WindowId::Automation => (270., 28., 760., 420.),
                WindowId::Plugins => (160., 44., 540., 480.),
                WindowId::Configuration => (560., 44., 460., 480.),
                WindowId::Piano => (260., 64., 900., 460.),
                WindowId::Mixer => (220., 108., 960., 480.),
                WindowId::Plugin(n) => (
                    280. + (n % 5) as f32 * 24.,
                    36. + (n % 5) as f32 * 24.,
                    820.,
                    540.,
                ),
            };
            WindowState {
                bounds: WindowBounds {
                    x,
                    y,
                    width,
                    height,
                },
                maximized: false,
                z: 0,
            }
        })
    }
    pub fn focus(&mut self, id: WindowId) {
        self.hidden = false;
        let mut state = self.state(id);
        self.next_z = self.next_z.saturating_add(1);
        state.z = self.next_z;
        self.states.insert(id, state);
    }
    pub fn toggle_maximize(&mut self, id: WindowId) {
        self.focus(id);
        let state = self.states.get_mut(&id).unwrap();
        state.maximized = !state.maximized;
    }
    pub fn set_bounds(&mut self, id: WindowId, bounds: WindowBounds) {
        let mut state = self.state(id);
        state.bounds = bounds;
        self.states.insert(id, state);
    }
    pub fn begin(&mut self, id: WindowId, anchor: Point<Pixels>, kind: GestureKind) {
        self.focus(id);
        if self.state(id).maximized {
            return;
        }
        self.drag = Some(WindowDrag {
            id,
            anchor,
            origin: self.bounds(id),
            kind,
        });
    }
    pub fn move_drag(&mut self, position: Point<Pixels>) {
        let Some(drag) = self.drag else { return };
        let dx = f32::from(position.x - drag.anchor.x);
        let dy = f32::from(position.y - drag.anchor.y);
        let o = drag.origin;
        let viewport = self.desktop.get().size;
        let w = f32::from(viewport.width).max(1.);
        let h = f32::from(viewport.height).max(1.);
        let mut b = o;
        match drag.kind {
            GestureKind::Move => {
                b.x += dx;
                b.y += dy;
            }
            GestureKind::Resize {
                left,
                right,
                top,
                bottom,
            } => {
                let min_w = (if drag.id == WindowId::Patterns {
                    190_f32
                } else {
                    340.
                })
                .min(w);
                let min_h = (if drag.id == WindowId::Plugins {
                    300_f32
                } else {
                    180.
                })
                .min(h);
                if left {
                    b.x = (o.x + dx).clamp(0., (o.x + o.width - min_w).max(0.));
                    b.width = o.x + o.width - b.x;
                }
                if top {
                    b.y = (o.y + dy).clamp(0., (o.y + o.height - min_h).max(0.));
                    b.height = o.y + o.height - b.y;
                }
                if right {
                    b.width = (o.width + dx).clamp(min_w, (w - o.x).max(min_w));
                }
                if bottom {
                    b.height = (o.height + dy).clamp(min_h, (h - o.y).max(min_h));
                }
            }
        }
        self.set_bounds(drag.id, b.fit(viewport));
    }
    pub fn cancel(&mut self) {
        if let Some(drag) = self.drag.take() {
            self.set_bounds(drag.id, drag.origin);
        }
    }
    pub fn end_drag(&mut self) {
        self.drag = None;
    }
    pub fn bounds(&self, id: WindowId) -> WindowBounds {
        let state = self.state(id);
        let viewport = self.desktop.get().size;
        if state.maximized {
            WindowBounds {
                x: 0.,
                y: 0.,
                width: f32::from(viewport.width).max(1.),
                height: f32::from(viewport.height).max(1.),
            }
        } else {
            state.bounds.fit(viewport)
        }
    }
}
