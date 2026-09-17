//! Panel content; navigation is pinned above the scroll region.
use crate::plugin_window::PluginWindow;
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, width: f32, cx: &mut Context<PluginWindow>) -> Div {
    let theme = this.theme;
    let Some(layout) = &this.panel else {
        return div();
    };
    let Some(details) = &this.details else {
        return div();
    };
    let mut content = div().w_full().p_3().flex().flex_col().gap_2();
    let key = (
        details.info.descriptor.plugin_id.clone(),
        details.info.descriptor.plugin_version.clone(),
    );
    if let Some(error) = this.project.panels.error(&key) {
        content = content.child(
            div()
                .px_2()
                .py_1()
                .text_size(px(10.))
                .bg(rgb(theme.diagnostic_bg))
                .text_color(rgb(theme.diagnostic_text))
                .child(format!(
                    "{error} · {}",
                    if this.project.panels.layouts.contains_key(&key) {
                        "Last valid layout"
                    } else {
                        "Default layout"
                    }
                )),
        );
    }
    let Some(page) = layout
        .pages
        .iter()
        .find(|p| p.id == this.page)
        .or(layout.pages.first())
    else {
        return content;
    };
    if !this.project.panels.layouts.contains_key(&key) && !this.plots.is_empty() {
        content = content.child(crate::plugin_plot_view::view(
            this.plots.clone(),
            theme,
            width,
        ));
    }
    content
        .child(crate::plugin_panel_groups::view(
            this,
            &page.groups,
            width,
            cx,
        ))
        .when(details.parameters.is_empty(), |d| {
            d.child(theme.label("No parameter controls"))
        })
}
