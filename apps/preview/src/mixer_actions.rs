//! Keyboard navigation selects a scope and reveals its strip; musical values stay read-only.
use crate::{mixer_model, ui::Preview};
use gpui::*;

impl Preview {
    pub fn mixer_key(&mut self, key: &str) -> bool {
        let Some(project) = &self.project else {
            return false;
        };
        let mut strips = mixer_model::strips(project);
        strips.sort_by_key(|s| s.id != "mix_master");
        let current = strips
            .iter()
            .position(|s| s.id == self.selected_scope)
            .unwrap_or(0);
        let next = match key {
            "left" => current.saturating_sub(1),
            "right" => (current + 1).min(strips.len() - 1),
            "home" => 0,
            "end" => strips.len() - 1,
            "up" | "down" | "pageup" | "pagedown" => {
                let scroll = &self.workspace.mixer;
                let delta = match key {
                    "up" => 28.,
                    "down" => -28.,
                    "pageup" => 160.,
                    _ => -160.,
                };
                scroll.set_offset(point(
                    scroll.offset().x,
                    (scroll.offset().y + px(delta)).clamp(-scroll.max_offset().height, px(0.)),
                ));
                return true;
            }
            _ => return false,
        };
        self.selected_scope = strips[next].id.clone();
        self.workspace.inspector.set_offset(point(px(0.), px(0.)));
        if next > 0 {
            let scroll = &self.workspace.mixer;
            let left = px((next - 1) as f32 * 84.);
            let visible_left = -scroll.offset().x;
            let width = scroll.bounds().size.width;
            let offset = if left < visible_left {
                -left
            } else if left + px(84.) > visible_left + width {
                width - left - px(84.)
            } else {
                scroll.offset().x
            };
            scroll.set_offset(point(
                offset.clamp(-scroll.max_offset().width, px(0.)),
                scroll.offset().y,
            ));
        }
        true
    }
}
