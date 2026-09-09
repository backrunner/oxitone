//! Compact library; choosing a plugin never opens its parameter reference.
pub use crate::plugin_manager_model::{CatalogEntry, LibrarySection, ManagerUi};
use crate::{document_wire::DocumentOperation, ui::Preview, ui_icons::Icon};
use gpui::{prelude::*, *};

impl Preview {
    pub fn plugin_manager(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let state = &self.document.manager;
        let entries = self.library_entries();
        let selected = entries
            .iter()
            .copied()
            .find(|entry| state.selected.as_deref() == Some(&entry.handle));
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(rgb(t.panel))
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(crate::plugin_library::search(self, cx))
                    .child(t.ghost("plugin-add", "Add package").on_click(cx.listener(
                        |this, _, window, cx| {
                            this.document.manager.adding = !this.document.manager.adding;
                            this.document.manager.searching = this.document.manager.adding;
                            this.document.manager.select_all = true;
                            this.document.manager.input_error = None;
                            this.document.manager.section = None;
                            this.workspace_focus.focus(window);
                            cx.notify();
                        },
                    )))
                    .child(
                        t.icon_button("plugin-refresh", Icon::Loop, "Rescan plugins")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.document_request(DocumentOperation::RefreshPlugins);
                                cx.notify();
                            })),
                    ),
            )
            .when(state.adding, |d| {
                d.child(crate::plugin_library::install(self, cx))
            })
            .when_some(state.input_error.as_ref(), |d, error| {
                d.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_size(px(11.))
                        .text_color(rgb(t.danger))
                        .child(error.clone()),
                )
            })
            .child(crate::plugin_library::filters(self, cx))
            .child(crate::plugin_picker::bar(self, cx))
            .child(crate::plugin_library::list(self, &entries, cx))
            .child(
                div()
                    .h(px(36.))
                    .flex_shrink_0()
                    .px_2()
                    .border_t_1()
                    .border_color(rgb(t.border))
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(
                        selected.is_some_and(|entry| !entry.usages.is_empty()),
                        |d| {
                            d.child(
                                t.tool(
                                    "plugin-uses",
                                    "Used in project",
                                    state.section == Some(LibrarySection::Uses),
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.toggle_library_section(LibrarySection::Uses);
                                        cx.notify();
                                    },
                                )),
                            )
                        },
                    )
                    .child(div().flex_1())
                    .when(selected.is_some(), |d| {
                        d.child(
                            t.tool(
                                "plugin-details",
                                "Details",
                                state.section == Some(LibrarySection::Details),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_library_section(LibrarySection::Details);
                                cx.notify();
                            })),
                        )
                    }),
            )
            .when_some(selected.filter(|_| state.section.is_some()), |d, entry| {
                d.child(self.plugin_catalog_detail(entry, cx))
            })
    }

    pub(super) fn library_entries(&self) -> Vec<&CatalogEntry> {
        self.document
            .view
            .as_ref()
            .into_iter()
            .flat_map(|v| &v.plugins)
            .filter(|entry| {
                entry.matches(self.document.manager.category, &self.document.manager.query)
                    && self
                        .document
                        .manager
                        .assignment
                        .as_ref()
                        .is_none_or(|s| (entry.kind == "instrument") == s.instrument())
            })
            .collect()
    }

    pub(super) fn toggle_library_section(&mut self, section: LibrarySection) {
        let index = self
            .library_entries()
            .iter()
            .position(|entry| self.document.manager.selected.as_ref() == Some(&entry.handle));
        let state = &mut self.document.manager;
        state.section = (state.section != Some(section)).then_some(section);
        state.details_scroll.set_offset(point(px(0.), px(0.)));
        state.searching = false;
        if let Some(index) = index {
            // The drawer changes list height in this frame; align by top, not cached height.
            state.list_scroll.scroll_to_top_of_item(index);
        }
    }
}
