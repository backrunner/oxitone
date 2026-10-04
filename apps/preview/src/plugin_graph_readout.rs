//! Selected graph values share the exact source parameter model used by the auxiliary knobs.
use crate::{plugin_graph_handle::Handle, plugin_window::PluginWindow};
use gpui::{prelude::*, *};

pub fn view(handles: &[Handle], this: &PluginWindow, editable: bool) -> Div {
    let mut row = div().flex().flex_wrap().items_center().gap_3();
    let Some(handle) = handles
        .iter()
        .find(|h| Some(h.key().as_str()) == this.graph_selected.as_deref())
    else {
        return row
            .text_size(px(10.))
            .text_color(rgb(this.theme.muted))
            .child(if editable {
                "Drag the graph to select and adjust a node"
            } else {
                "Source parameters"
            });
    };
    row = row.child(
        div()
            .text_color(rgb(this.theme.track(handle.color)))
            .font_weight(FontWeight::SEMIBOLD)
            .child(handle.label.clone()),
    );
    if let Some(details) = &this.details {
        for axis in [&handle.x, &handle.y, &handle.auxiliary]
            .into_iter()
            .flatten()
        {
            if let Some(p) = details
                .parameters
                .iter()
                .find(|p| !p.host && p.spec.id == axis.parameter)
            {
                row = row.child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            div()
                                .text_color(rgb(this.theme.muted))
                                .child(p.spec.label.clone()),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(crate::plugin_controls::value(p)),
                        ),
                );
            }
        }
    }
    row.text_size(px(11.))
}
