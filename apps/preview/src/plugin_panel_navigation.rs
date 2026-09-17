//! A single navigation strip for sound pages; inspection stays behind one entry.
use crate::{
    plugin_window::{DetailTab, PluginWindow},
    ui_icons::Icon,
};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> Div {
    let t = this.theme;
    let mut tabs = div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .min_h(px(40.))
        .flex_wrap()
        .flex_shrink_0()
        .border_b_1()
        .border_color(rgb(t.border));
    if this.tab == DetailTab::Panel {
        if let Some(layout) = &this.panel {
            for page in &layout.pages {
                let id = page.id.clone();
                tabs = tabs.child(
                    t.tool(
                        format!("panel-page-{id}"),
                        page.title.clone(),
                        this.page == id,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.focus(window);
                        this.page = id.clone();
                        this.choice_open = None;
                        this.scroll.set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    })),
                );
            }
        }
    } else {
        tabs = tabs.child(
            t.icon_button("panel-back", Icon::Left, "Back to controls")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tab = DetailTab::Panel;
                    this.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                })),
        );
        for (tab, title) in [
            (DetailTab::Parameters, "Parameters"),
            (DetailTab::Resources, "Assets"),
            (DetailTab::Plugin, "Info"),
        ] {
            tabs = tabs.child(t.tool(title, title, this.tab == tab).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.tab = tab;
                    this.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                },
            )));
        }
    }
    tabs = tabs.child(div().flex_1());
    let oscillator = this.tab == DetailTab::Panel
        && this.panel.as_ref().is_some_and(|p| {
            p.pages
                .iter()
                .filter(|p| p.id == this.page)
                .flat_map(|p| &p.groups)
                .flat_map(|g| &g.controls)
                .any(|c| matches!(c, crate::plugin_layout::Control::Oscillator { .. }))
        });
    if oscillator {
        tabs = tabs.child(
            t.ghost(
                "wave-view",
                if this.stacked_waveforms { "3D" } else { "2D" },
            )
            .text_color(rgb(t.muted))
            .on_click(cx.listener(|this, _, _, cx| {
                this.stacked_waveforms = !this.stacked_waveforms;
                cx.notify();
            })),
        );
    }
    if this.tab == DetailTab::Panel {
        tabs = tabs.child(
            t.icon_button(
                "panel-inspect",
                Icon::Info,
                "Inspect parameters, assets and plugin information",
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.tab = DetailTab::Parameters;
                this.choice_open = None;
                this.scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            })),
        );
    }
    if this.tab == DetailTab::Plugin {
        tabs = tabs.child(
            t.button(
                "copy-plugin-reference",
                if this.copied { "Copied" } else { "Copy JSON" },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(details) = &this.details {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        serde_json::to_string_pretty(&details.source).unwrap(),
                    ));
                    this.copied = true;
                    cx.notify();
                }
            })),
        );
    } else if this
        .owner
        .upgrade()
        .is_some_and(|owner| owner.read(cx).document.view.is_some())
    {
        tabs = tabs.child(
            t.icon_button(
                "plugin-edit-source",
                Icon::Code,
                "Configure source values and editing scope",
            )
            .relative()
            .child({
                let bounds = this.source_button.clone();
                canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                    .absolute()
                    .inset_0()
                    .size_full()
            })
            .on_click(cx.listener(|this, _, window, cx| {
                let target = this.target.clone();
                let _ = this.owner.update(cx, |owner, cx| {
                    owner.edit_plugin_source(&target, window, cx)
                });
            })),
        );
    }
    tabs
}
