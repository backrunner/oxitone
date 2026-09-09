//! Keyboard navigation selects a scope and reveals its strip; musical values stay read-only.
use crate::{mixer_model, ui::Preview};
use gpui::*;

pub fn scroll_details(scroll: &ScrollHandle, key: &str) -> bool {
    let delta = match key {
        "up" => 32.,
        "down" => -32.,
        "pageup" => f32::from(scroll.bounds().size.height),
        "pagedown" => -f32::from(scroll.bounds().size.height),
        "home" => f32::from(scroll.max_offset().height),
        "end" => -f32::from(scroll.max_offset().height),
        _ => return false,
    };
    scroll.set_offset(point(
        px(0.),
        (scroll.offset().y + px(delta)).clamp(-scroll.max_offset().height, px(0.)),
    ));
    true
}

impl Preview {
    pub fn mixer_key(&mut self, key: &str) -> bool {
        let Some(project) = &self.project else {
            return false;
        };
        let mut strips: Vec<_> = mixer_model::strips(project).iter().collect();
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
        let id = strips[next].id.clone();
        self.select_mixer(&id);
        true
    }

    pub fn select_mixer(&mut self, id: &str) {
        self.document.playlist.focused = false;
        let Some(project) = &self.project else {
            return;
        };
        let strips = mixer_model::strips(project);
        let Some(selected) = strips.iter().find(|s| s.id == id) else {
            return;
        };
        self.selected_scope = selected.id.clone();
        self.workspace.inspector.set_offset(point(px(0.), px(0.)));
        if id != "mix_master" {
            let index = strips
                .iter()
                .filter(|s| s.id != "mix_master")
                .position(|s| s.id == id)
                .unwrap();
            let scroll = &self.workspace.mixer;
            let left = px(index as f32 * mixer_model::STRIP_WIDTH);
            let visible_left = -scroll.offset().x;
            let width = scroll.bounds().size.width;
            let offset = if left < visible_left {
                -left
            } else if left + px(mixer_model::STRIP_WIDTH) > visible_left + width {
                width - left - px(mixer_model::STRIP_WIDTH)
            } else {
                scroll.offset().x
            };
            scroll.set_offset(point(
                offset.clamp(-scroll.max_offset().width, px(0.)),
                scroll.offset().y,
            ));
        }
    }
}
