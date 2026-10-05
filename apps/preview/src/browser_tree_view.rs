use crate::{
    browser_tree::{BrowserTree, Entry},
    sample_drop::{SampleDrag, SampleSource},
    ui::Preview,
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};
use std::path::PathBuf;

const TREE_INSET: f32 = 8.;
const TREE_INDENT: f32 = 16.;
const ICON_SIZE: f32 = 16.;
const ICON_GAP: f32 = 4.;

fn row_inset(depth: usize) -> Pixels {
    px(TREE_INSET + depth as f32 * TREE_INDENT)
}

fn child_label_inset(depth: usize) -> Pixels {
    row_inset(depth + 1) + px(2. * (ICON_SIZE + ICON_GAP))
}

pub fn flatten(tree: &BrowserTree, entry: Entry, depth: usize, rows: &mut Vec<(Entry, usize)>) {
    if depth > 24 || rows.len() >= 4096 {
        return;
    }
    let path = entry.path.clone();
    rows.push((entry, depth));
    if tree.expanded.contains(&path) {
        if let Some(Ok(children)) = tree.children.get(&path) {
            for child in children {
                flatten(tree, child.clone(), depth + 1, rows);
            }
        }
    }
}
pub fn view(this: &mut Preview, cx: &mut Context<Preview>) -> Div {
    let t = this.theme;
    if this.document.browser.roots.is_empty() {
        if let Some(view) = &this.document.view {
            this.document
                .browser
                .roots
                .push(PathBuf::from(&view.project_root));
        }
    }
    let tree = &this.document.browser;
    tree.row_bounds.borrow_mut().clear();
    let mut rows = Vec::new();
    for path in &tree.roots {
        flatten(
            tree,
            Entry {
                path: path.clone(),
                directory: true,
            },
            0,
            &mut rows,
        );
    }
    let mut list = div().flex().flex_col().pb_2().child(
        div()
            .h(px(32.))
            .pl_3()
            .pr_2()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .child("SAMPLE FOLDERS"),
            )
            .child(
                t.icon_button("sample-folder-add", Icon::Plus, "Add sample folder")
                    .size(px(24.))
                    .rounded(px(5.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.add_sample_folder(cx);
                    })),
            ),
    );
    for (index, (entry, depth)) in rows.into_iter().enumerate() {
        let path = entry.path;
        let expanded = tree.expanded.contains(&path);
        let selected = tree.selected.as_ref() == Some(&path);
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string();
        let error = tree
            .children
            .get(&path)
            .and_then(|r| r.as_ref().err())
            .cloned();
        let loading = tree.loading.contains(&path);
        let action_path = path.clone();
        let measured_path = path.clone();
        let row_bounds = tree.row_bounds.clone();
        list = list.child(
            div()
                .id(("sample-tree", index))
                .relative()
                .child(
                    canvas(
                        move |bounds, _, _| {
                            row_bounds
                                .borrow_mut()
                                .insert(measured_path.clone(), bounds);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .h(px(28.))
                .flex_shrink_0()
                .pl(row_inset(depth))
                .pr_2()
                .flex()
                .items_center()
                .gap(px(ICON_GAP))
                .cursor_pointer()
                .bg(rgb(if selected { t.selected } else { t.panel }))
                .hover(move |s| s.bg(rgb(t.button)))
                .child(
                    div()
                        .size(px(ICON_SIZE))
                        .flex_shrink_0()
                        .when(entry.directory, |d| {
                            d.child(icon(
                                if expanded {
                                    Icon::ChevronDown
                                } else {
                                    Icon::Right
                                },
                                t.muted,
                            ))
                        }),
                )
                .child(icon(
                    if entry.directory {
                        Icon::Folder
                    } else {
                        Icon::Wave
                    },
                    if entry.directory { t.muted } else { t.gold },
                ))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_size(px(11.))
                        .child(name.clone()),
                )
                .when(loading, |d| d.child(div().text_xs().child("…")))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.workspace_focus.focus(window);
                        this.document.browser.focused = true;
                        this.document.browser.selected = Some(action_path.clone());
                        if entry.directory {
                            this.expand_sample_folder(action_path.clone(), cx);
                        } else if this.document_ready() {
                            this.document.browser.drag = Some(SampleDrag::new(
                                SampleSource::File {
                                    path: action_path.to_string_lossy().into(),
                                },
                                name.clone(),
                                event.position,
                            ));
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
        );
        if expanded {
            if let Some(error) = error {
                list = list.child(
                    div()
                        .pl(child_label_inset(depth))
                        .pr_2()
                        .text_size(px(10.))
                        .text_color(rgb(t.muted))
                        .child(error),
                );
            } else if tree
                .children
                .get(&path)
                .is_some_and(|r| r.as_ref().is_ok_and(|items| items.is_empty()))
            {
                list = list.child(
                    div()
                        .pl(child_label_inset(depth))
                        .pr_2()
                        .text_size(px(10.))
                        .text_color(rgb(t.muted))
                        .child("No audio files"),
                );
            }
        }
    }
    list
}
pub fn body(this: &mut Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let tree = view(this, cx);
    div()
        .id("browser-content")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&this.document.browser.scroll)
        .child(tree)
        .child(crate::pattern_manager::view(this, cx))
}
