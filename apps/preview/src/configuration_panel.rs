use crate::{
    configuration_edit::ConfigurationInput,
    configuration_wire::{ConfigurationEdit, HostEdit},
    plugin_manager::CatalogEntry,
    ui::Preview,
};
use gpui::{prelude::*, *};

impl Preview {
    pub(super) fn configuration_panel(&self, entry: &CatalogEntry, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let mut panel = div().flex().flex_col().gap_1();
        if self.document.configuration.selected.is_none() {
            return panel;
        }
        let scope = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .mb_2()
            .child(
                theme
                    .button(
                        "configuration-scope",
                        if self.document.configuration.shared {
                            "Shared"
                        } else {
                            "This instance"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.document.configuration.shared = !this.document.configuration.shared;
                        this.document.configuration.input = None;
                        cx.notify();
                    })),
            );
        panel = panel.child(
            scope
                .child(self.effect_order_controls(entry, cx))
                .child(self.rack_controls(entry, cx)),
        );
        let Some(site) = self.configuration_site() else {
            return panel.child(theme.label("No isolated source boundary for this selection. Select a shared definition or edit its code."));
        };
        if site.config.plugin_id != entry.plugin_id
            || site.config.plugin_version != entry.plugin_version
        {
            return panel;
        }
        if self.document.configuration.shared {
            panel = panel.child(
                div()
                    .mb_2()
                    .text_size(px(11.))
                    .text_color(rgb(theme.gold))
                    .child(format!("Editing {} uses", site.usages.len())),
            );
        }
        if site.kind == "effect" {
            panel = panel.child(self.configuration_number(
                "mix",
                "Insert mix",
                true,
                site.config.mix.unwrap_or(1.),
                0.,
                1.,
                1.,
                false,
                cx,
            ));
            let bypass = site.config.bypass.unwrap_or(false);
            panel = panel.child(
                div()
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child("Bypass")
                    .child(
                        theme
                            .tool(
                                "configuration-bypass",
                                if bypass { "On" } else { "Off" },
                                bypass,
                            )
                            .w(px(84.))
                            .opacity(if self.document_ready() { 1. } else { 0.4 })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(operation) =
                                    this.configuration_operation(ConfigurationEdit::Host {
                                        values: HostEdit {
                                            bypass: Some(!bypass),
                                            mix: None,
                                        },
                                    })
                                {
                                    this.document_request(operation);
                                }
                                cx.notify();
                            })),
                    ),
            );
        }
        for parameter in &entry.parameters {
            let value = site
                .config
                .parameters
                .get(&parameter.id)
                .copied()
                .unwrap_or(parameter.default);
            panel = panel.child(self.configuration_number(
                &parameter.id,
                &parameter.label,
                false,
                value,
                parameter.min,
                parameter.max,
                parameter.default,
                parameter.unit == oxitone_core::wire::ParameterUnit::Enum,
                cx,
            ));
        }
        panel
    }
    #[allow(clippy::too_many_arguments)]
    fn configuration_number(
        &self,
        parameter: &str,
        label: &str,
        host: bool,
        value: f64,
        min: f64,
        max: f64,
        default: f64,
        discrete: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let prefix = format!(
            "configuration-{}-{parameter}",
            if host { "host" } else { "plugin" }
        );
        let parameter = parameter.to_owned();
        let mut row = div()
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .opacity(if self.document_ready() { 1. } else { 0.4 })
            .child(div().flex_1().min_w_0().truncate().child(label.to_owned()));
        for (name, label, next) in [
            (
                "less",
                "−",
                (value - if discrete { 1. } else { (max - min) / 100. }).max(min),
            ),
            (
                "more",
                "+",
                (value + if discrete { 1. } else { (max - min) / 100. }).min(max),
            ),
        ] {
            let parameter = parameter.clone();
            row = row.child(
                theme
                    .button(format!("{prefix}-{name}"), label)
                    .w(px(24.))
                    .px_0()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_configuration_value(parameter.clone(), host, next);
                        cx.notify();
                    })),
            );
        }
        let reset_parameter = parameter.clone();
        row = row.child(
            theme
                .icon_button(
                    format!("{prefix}-reset"),
                    crate::ui_icons::Icon::Undo,
                    "Reset to default",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_configuration_value(reset_parameter.clone(), host, default);
                    cx.notify();
                })),
        );
        let editing = self
            .document
            .configuration
            .input
            .as_ref()
            .filter(|input| input.parameter == parameter && input.host == host);
        let text = editing
            .map(|input| format!("{}│", input.text))
            .unwrap_or_else(|| {
                if value != 0. && value.abs() < 0.0001 {
                    return format!("{value:.3e}");
                }
                format!("{value:.5}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_owned()
            });
        row.child(
            theme
                .button(format!("{prefix}-value"), text)
                .w(px(84.))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if !this.document_ready() {
                        return;
                    }
                    let Some(site) = this.configuration_site() else {
                        return;
                    };
                    this.document.configuration.input = Some(ConfigurationInput {
                        site: site.handle.clone(),
                        usage: (!this.document.configuration.shared && site.scope == "reference")
                            .then(|| site.usages[0].handle.clone()),
                        parameter: parameter.clone(),
                        host,
                        text: value.to_string(),
                        select_all: true,
                    });
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        )
    }
}
