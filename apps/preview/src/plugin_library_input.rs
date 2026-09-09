//! Library navigation and package input share the document queue, never an installer process.
use crate::{
    document_wire::DocumentOperation, plugin_manager::LibrarySection, ui::Preview,
    window_manager::WindowId,
};
use gpui::*;

impl Preview {
    pub(super) fn select_library_plugin(&mut self, handle: String) {
        let index = self
            .library_entries()
            .iter()
            .position(|entry| entry.handle == handle);
        let state = &mut self.document.manager;
        if state.selected.as_ref() != Some(&handle) {
            state.section = None;
            state.details_scroll.set_offset(point(px(0.), px(0.)));
        }
        state.selected = Some(handle);
        state.searching = false;
        if let Some(index) = index {
            state.list_scroll.scroll_to_item(index);
        }
    }

    pub(super) fn submit_plugin_package(&mut self) {
        let input = self.document.manager.package_input.trim();
        let Some((package_name, version)) = crate::plugin_manager_model::parse_package_spec(input)
        else {
            self.document.manager.input_error =
                Some("Enter a package name, optionally followed by @version".into());
            return;
        };
        let operation = DocumentOperation::InstallPlugin {
            package_name,
            version,
        };
        if !self.document_operation_ready(&operation) {
            self.document.manager.input_error =
                Some("Wait for the current operation or reconnect the project".into());
            return;
        }
        self.document_request(operation);
        if self.document.pending.is_some() {
            let state = &mut self.document.manager;
            state.adding = false;
            state.searching = false;
            state.input_error = None;
        }
    }

    pub fn plugin_search_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.document.manager.open || self.document.windows.front() != Some(WindowId::Plugins) {
            return false;
        }
        let key = &event.keystroke;
        let command = key.modifiers.platform || key.modifiers.control;
        if command && key.key == "f" {
            self.document.manager.searching = true;
            self.document.manager.adding = false;
            self.document.manager.select_all = true;
        } else if !self.document.manager.searching {
            if command || key.modifiers.alt || key.modifiers.shift {
                return false;
            }
            match key.key.as_str() {
                "up" | "down" | "home" | "end" => {
                    let entries = self.library_entries();
                    let current = entries.iter().position(|entry| {
                        self.document.manager.selected.as_ref() == Some(&entry.handle)
                    });
                    let index = match key.key.as_str() {
                        "up" => current.unwrap_or(1).saturating_sub(1),
                        "down" => current
                            .map_or(0, |i| i + 1)
                            .min(entries.len().saturating_sub(1)),
                        "home" => 0,
                        _ => entries.len().saturating_sub(1),
                    };
                    if let Some(entry) = entries.get(index) {
                        let handle = entry.handle.clone();
                        self.select_library_plugin(handle);
                    }
                }
                "enter" if !event.is_held => {
                    if self.document.manager.assignment.is_some() {
                        self.assign_selected_plugin();
                    } else {
                        self.toggle_library_section(LibrarySection::Uses);
                    }
                }
                "escape" => self.document.manager.section = None,
                _ => return false,
            }
        } else if key.key == "escape" {
            self.document.manager.searching = false;
            self.document.manager.adding = false;
            self.document.manager.input_error = None;
        } else if key.key == "enter" {
            if self.document.manager.adding {
                self.submit_plugin_package();
            } else {
                self.document.manager.searching = false;
            }
        } else {
            let state = &mut self.document.manager;
            let input = if state.adding {
                &mut state.package_input
            } else {
                &mut state.query
            };
            state.input_error = None;
            if command && key.key == "a" {
                state.select_all = true;
            } else if key.key == "backspace" {
                if state.select_all {
                    input.clear();
                } else {
                    input.pop();
                }
                state.select_all = false;
            } else if command && key.key == "v" {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    if state.select_all {
                        input.clear();
                    }
                    input.extend(text.chars().take(385));
                    state.select_all = false;
                }
            } else if command || key.modifiers.alt {
                // Editing a search/package string must not edit the music behind it.
            } else if let Some(text) = &key.key_char {
                if state.select_all {
                    input.clear();
                }
                input.extend(text.chars().take(385));
                state.select_all = false;
            }
            *input = input
                .chars()
                .take(if state.adding { 385 } else { 256 })
                .collect();
            state.list_scroll.set_offset(point(px(0.), px(0.)));
            state.section = None;
        }
        cx.stop_propagation();
        cx.notify();
        true
    }
}
