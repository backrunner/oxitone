//! Direct graph manipulation; a handle can edit two parameters in one source transaction.
use crate::{
    plugin_parameter_drag::{Binding, Range},
    plugin_window::PluginWindow,
};
use gpui::{prelude::*, *};

#[derive(Clone, Debug)]
pub struct Axis {
    pub parameter: String,
    pub range: Range,
}
impl Axis {
    pub fn new(parameter: impl Into<String>, range: Range) -> Self {
        Self {
            parameter: parameter.into(),
            range,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Handle {
    pub label: String,
    pub position: (f32, f32),
    pub color: usize,
    pub x: Option<Axis>,
    pub y: Option<Axis>,
    /// Option/Alt-drag edits this axis without changing either primary axis.
    pub auxiliary: Option<Axis>,
}
impl Handle {
    pub fn key(&self) -> String {
        format!(
            "graph:{}:{}",
            self.x.as_ref().map_or("", |a| &a.parameter),
            self.y.as_ref().map_or("", |a| &a.parameter)
        )
    }
}

pub fn overlay(handles: &[Handle], this: &PluginWindow, cx: &mut Context<PluginWindow>) -> Div {
    let area = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
    let layout_area = area.clone();
    let recorded = this.parameter_bounds.clone();
    let area_key = format!(
        "graph-area:{}",
        handles.first().map_or_else(String::new, Handle::key)
    );
    let mut root = div().absolute().inset_0().child(
        canvas(
            move |bounds, _, _| {
                layout_area.set(bounds);
                recorded.borrow_mut().insert(area_key.clone(), bounds);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full(),
    );
    let Some(details) = &this.details else {
        return root;
    };
    let enabled = this.owner.upgrade().is_some_and(|o| {
        let owner = o.read(cx);
        owner.document_ready() && owner.plugin_configuration_target(&this.target).is_some()
    });
    for handle in handles {
        let parameters: Vec<_> = [&handle.x, &handle.y]
            .into_iter()
            .enumerate()
            .filter_map(|(i, axis)| {
                let axis = axis.as_ref()?;
                let p = details
                    .parameters
                    .iter()
                    .find(|p| !p.host && p.spec.id == axis.parameter)?;
                Some((p.clone(), axis.range, i == 0))
            })
            .collect();
        if parameters.is_empty() {
            continue;
        }
        let handle_input = handle.clone();
        let area = area.clone();
        let recorded = this.parameter_bounds.clone();
        let key = handle.key();
        let color = this.theme.track(handle.color);
        let selected = this.graph_selected.as_deref() == Some(key.as_str());
        let active = selected
            || this.owner.upgrade().is_some_and(|o| {
                o.read(cx).document.plugin.changes().iter().any(|c| {
                    c.identity == this.identity
                        && parameters.iter().any(|(p, _, _)| c.parameter == p.spec.id)
                })
            });
        root = root.child(
            div()
                .id(SharedString::from(key.clone()))
                .absolute()
                .left(relative(handle.position.0.clamp(0., 1.)))
                .top(relative(handle.position.1.clamp(0., 1.)))
                .ml(px(-11.))
                .mt(px(-11.))
                .size(px(22.))
                .rounded_full()
                .border_2()
                .border_color(rgb(color))
                .bg(rgb(this.theme.scope))
                .when(active, |d| d.bg(crate::ui::alpha(color, 0.22)))
                .text_color(rgb(color))
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .flex()
                .items_center()
                .justify_center()
                .when(enabled, |d| {
                    d.cursor(CursorStyle::Crosshair)
                        .hover(|d| d.bg(crate::ui::alpha(color, 0.22)))
                })
                .child(handle.label.clone())
                .child(
                    canvas(
                        move |bounds, _, _| {
                            recorded.borrow_mut().insert(key.clone(), bounds);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if !enabled {
                            return;
                        }
                        begin(this, &handle_input, area.get(), event, window, cx);
                        cx.stop_propagation();
                    }),
                ),
        );
    }
    let handles = handles.to_vec();
    root.when(enabled && !handles.is_empty(), |d| {
        d.cursor(CursorStyle::Crosshair)
    })
    .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            if !enabled {
                return;
            }
            let bounds = area.get();
            let offset = event.position - bounds.origin;
            let at = (f32::from(offset.x), f32::from(offset.y));
            let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            if let Some(handle) = nearest(&handles, at, size) {
                begin(this, handle, bounds, event, window, cx);
                cx.stop_propagation();
            }
        }),
    )
}

fn nearest(handles: &[Handle], at: (f32, f32), size: (f32, f32)) -> Option<&Handle> {
    let distance = |h: &Handle| {
        (h.position.0.clamp(0., 1.) * size.0 - at.0).powi(2)
            + (h.position.1.clamp(0., 1.) * size.1 - at.1).powi(2)
    };
    handles
        .iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

pub fn begin(
    this: &mut PluginWindow,
    handle: &Handle,
    bounds: Bounds<Pixels>,
    event: &MouseDownEvent,
    window: &mut Window,
    cx: &mut Context<PluginWindow>,
) {
    this.focus(window);
    this.choice_open = None;
    this.graph_selected = Some(handle.key());
    let Some(details) = &this.details else { return };
    let axes = if event.modifiers.alt && handle.auxiliary.is_some() {
        vec![(handle.auxiliary.as_ref(), false, 180.)]
    } else {
        vec![
            (handle.x.as_ref(), true, f32::from(bounds.size.width) as f64),
            (
                handle.y.as_ref(),
                false,
                f32::from(bounds.size.height) as f64,
            ),
        ]
    };
    let bindings = axes
        .into_iter()
        .filter_map(|(axis, horizontal, pixels)| {
            let axis = axis?;
            let parameter = details
                .parameters
                .iter()
                .find(|p| !p.host && p.spec.id == axis.parameter)?;
            Some(Binding::new(
                parameter.clone(),
                axis.range,
                horizontal,
                pixels,
            ))
        })
        .collect();
    let _ = this.owner.update(cx, |owner, cx| {
        if event.click_count == 2 {
            owner.reset_plugin_graph(&this.target, &this.identity, bindings, event);
        } else {
            owner.begin_plugin_gesture(&this.target, &this.identity, bindings, event);
        }
        cx.notify();
    });
    cx.notify();
}
