use crate::{
    configuration_wire::{ConfigurationEdit, ConfigurationSite, HostEdit},
    document_wire::DocumentOperation,
    ui::Preview,
};
use gpui::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct ConfigurationUi {
    pub open: bool,
    pub plugin: Option<String>,
    pub scroll: ScrollHandle,
    pub selected: Option<String>,
    pub shared: bool,
    pub input: Option<ConfigurationInput>,
}
pub struct ConfigurationInput {
    pub site: String,
    pub usage: Option<String>,
    pub parameter: String,
    pub host: bool,
    pub text: String,
    pub select_all: bool,
}
impl Preview {
    pub fn configuration_site(&self) -> Option<&ConfigurationSite> {
        let state = &self.document.configuration;
        let selected = state.selected.as_ref()?;
        let sites = &self.document.view.as_ref()?.configuration_sites;
        if state.shared {
            sites.iter().find(|site| {
                site.scope == "definition"
                    && site.usages.iter().any(|usage| &usage.handle == selected)
            })
        } else {
            sites
                .iter()
                .find(|site| {
                    site.scope == "reference"
                        && site.usages.len() == 1
                        && &site.usages[0].handle == selected
                })
                .or_else(|| {
                    sites
                        .iter()
                        .find(|site| site.usages.len() == 1 && &site.usages[0].handle == selected)
                })
        }
    }
    pub fn configuration_operation(&self, edit: ConfigurationEdit) -> Option<DocumentOperation> {
        let site = self.configuration_site()?;
        Some(DocumentOperation::Configuration {
            site: site.handle.clone(),
            usage: (!self.document.configuration.shared && site.scope == "reference")
                .then(|| site.usages[0].handle.clone()),
            edit,
        })
    }
    pub fn set_configuration_value(&mut self, parameter: String, host: bool, value: f64) {
        let edit = value_edit(parameter, host, value);
        if let Some(operation) = self.configuration_operation(edit) {
            self.document_request(operation);
        }
    }
    pub fn configuration_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.document.configuration.open
            || self.document.windows.front() != Some(crate::window_manager::WindowId::Configuration)
        {
            return false;
        }
        let Some(input) = &mut self.document.configuration.input else {
            return false;
        };
        let key = &event.keystroke;
        if key.key == "escape" {
            self.document.configuration.input = None;
        } else if key.key == "enter" {
            let input = self.document.configuration.input.take().unwrap();
            if let Ok(value) = input.text.parse::<f64>() {
                if value.is_finite() {
                    self.document_request(DocumentOperation::Configuration {
                        site: input.site,
                        usage: input.usage,
                        edit: value_edit(input.parameter, input.host, value),
                    });
                } else {
                    self.document.configuration.input = Some(input);
                }
            } else {
                self.document.configuration.input = Some(input);
            }
        } else if (key.modifiers.platform || key.modifiers.control) && key.key == "a" {
            input.select_all = true;
        } else if key.key == "backspace" {
            if input.select_all {
                input.text.clear();
            } else {
                input.text.pop();
            }
            input.select_all = false;
        } else if (key.modifiers.platform || key.modifiers.control) && key.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                if input.select_all {
                    input.text.clear();
                }
                input.text.extend(text.chars().take(64));
                input.select_all = false;
            }
        } else if key.modifiers.platform || key.modifiers.control || key.modifiers.alt {
            // A numeric input owns its shortcuts; do not undo music behind it.
        } else if let Some(text) = &key.key_char {
            if input.select_all {
                input.text.clear();
            }
            input.text.extend(
                text.chars()
                    .filter(|c| c.is_ascii_digit() || ".eE+-".contains(*c))
                    .take(64),
            );
            input.select_all = false;
        }
        if let Some(input) = &mut self.document.configuration.input {
            input.text.truncate(input.text.floor_char_boundary(64));
        }
        cx.stop_propagation();
        cx.notify();
        true
    }
}
fn value_edit(parameter: String, host: bool, value: f64) -> ConfigurationEdit {
    if host {
        ConfigurationEdit::Host {
            values: HostEdit {
                mix: Some(value),
                bypass: None,
            },
        }
    } else {
        ConfigurationEdit::Parameters {
            values: BTreeMap::from([(parameter, value)]),
        }
    }
}
