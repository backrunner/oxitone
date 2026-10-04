use crate::{browser_tree::Entry, ui::Preview};
use gpui::*;
impl Preview {
    pub fn browser_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.document.browser.focused {
            return false;
        }
        let key = &event.keystroke;
        let Some(selected) = self.document.browser.selected.clone() else {
            return false;
        };
        if (key.modifiers.platform || key.modifiers.control) && key.key == "r" {
            let path = if self.document.browser.children.contains_key(&selected) {
                selected
            } else {
                selected.parent().unwrap_or(&selected).to_path_buf()
            };
            self.document.browser.children.remove(&path);
            self.document.browser.expanded.remove(&path);
            self.expand_sample_folder(path, cx);
            return true;
        }
        if key.modifiers.platform || key.modifiers.control || key.modifiers.alt {
            return false;
        }
        let mut rows = Vec::new();
        for path in &self.document.browser.roots {
            crate::browser_tree_view::flatten(
                &self.document.browser,
                Entry {
                    path: path.clone(),
                    directory: true,
                },
                0,
                &mut rows,
            );
        }
        let Some(index) = rows.iter().position(|(entry, _)| entry.path == selected) else {
            return false;
        };
        match key.key.as_str() {
            "up" | "down" | "home" | "end" => {
                let next = match key.key.as_str() {
                    "up" => index.saturating_sub(1),
                    "down" => (index + 1).min(rows.len() - 1),
                    "home" => 0,
                    _ => rows.len() - 1,
                };
                self.document.browser.selected = Some(rows[next].0.path.clone());
            }
            "left" => {
                if !self.document.browser.expanded.remove(&selected) {
                    if let Some(parent) = selected
                        .parent()
                        .filter(|p| rows.iter().any(|(e, _)| e.path == *p))
                    {
                        self.document.browser.selected = Some(parent.to_path_buf());
                    }
                }
            }
            "right" | "enter" if rows[index].0.directory => {
                if !self.document.browser.expanded.contains(&selected) {
                    self.expand_sample_folder(selected, cx);
                } else if index + 1 < rows.len() && rows[index + 1].1 > rows[index].1 {
                    self.document.browser.selected = Some(rows[index + 1].0.path.clone());
                }
            }
            _ => return false,
        }
        let tree = &self.document.browser;
        if let Some(bounds) = tree
            .selected
            .as_ref()
            .and_then(|path| tree.row_bounds.borrow().get(path).copied())
        {
            let viewport = tree.scroll.bounds();
            let dy = if bounds.top() < viewport.top() {
                viewport.top() - bounds.top()
            } else if bounds.bottom() > viewport.bottom() {
                viewport.bottom() - bounds.bottom()
            } else {
                px(0.)
            };
            tree.scroll.set_offset(point(
                px(0.),
                (tree.scroll.offset().y + dy).clamp(-tree.scroll.max_offset().height, px(0.)),
            ));
        }
        true
    }
}
