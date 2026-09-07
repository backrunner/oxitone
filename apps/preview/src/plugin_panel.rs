use crate::{plugin_layout::Control, plugin_window::PluginWindow};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, width: f32, cx: &mut Context<PluginWindow>) -> Div {
    let theme = this.theme;
    let Some(layout) = &this.panel else {
        return div();
    };
    let Some(details) = &this.details else {
        return div();
    };
    let parameter = |id: &str| {
        details
            .parameters
            .iter()
            .find(|p| !p.host && p.spec.id == id)
    };
    let mut content = div().p_3().flex().flex_col().gap_2();
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
    if layout.pages.len() > 1 {
        let mut tabs = div().flex().gap_2();
        for page in &layout.pages {
            let id = page.id.clone();
            tabs = tabs.child(
                theme
                    .button(format!("panel-page-{id}"), page.title.clone())
                    .when(this.page == id, |d| d.bg(rgb(theme.selected)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = id.clone();
                        this.scroll.set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    })),
            );
        }
        content = content.child(tabs);
    }
    let Some(page) = layout
        .pages
        .iter()
        .find(|p| p.id == this.page)
        .or(layout.pages.first())
    else {
        return content;
    };
    let mut groups = div().flex().flex_wrap().gap_2().items_start();
    for group in &page.groups {
        let cols = group
            .columns
            .min(((width - 52.) / 82.).floor().max(1.) as u32);
        let mut controls = div().flex().flex_wrap();
        for control in &group.controls {
            let element = match control {
                Control::Envelope {
                    attack,
                    decay,
                    sustain,
                    release,
                    ..
                } => {
                    let values = [attack, decay, sustain, release]
                        .map(|id| parameter(id).map_or(0., |p| p.value));
                    div()
                        .w_full()
                        .bg(rgb(theme.scope))
                        .rounded_md()
                        .mb_2()
                        .child(crate::plugin_dial::envelope(values, theme))
                }
                _ => {
                    let Some(p) = parameter(control.bindings()[0]) else {
                        continue;
                    };
                    div()
                        .w(relative(1. / cols as f32))
                        .child(crate::plugin_controls::control_view(control, p, theme))
                }
            };
            controls = controls.child(element);
        }
        groups = groups.child(
            div()
                .flex_1()
                .flex_basis(px(cols as f32 * 82. + 18.))
                .min_w(px(cols as f32 * 82. + 18.))
                .px_2()
                .py_2()
                .rounded_lg()
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(crate::ui::alpha(theme.border, 0.65))
                .child(
                    div()
                        .mb_2()
                        .px_1()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme.muted))
                        .child(group.title.clone()),
                )
                .child(controls),
        );
    }
    content
        .child(groups)
        .when(details.parameters.is_empty(), |d| {
            d.child(theme.label("No parameter controls"))
        })
}
