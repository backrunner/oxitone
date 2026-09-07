//! Pure piano coordinates; all 128 MIDI keys remain reachable through scrolling.
use gpui::{px, Pixels, Size};
use std::{cell::Cell, rc::Rc};

pub const KEY_WIDTH: f32 = 76.;
pub const RULER: f32 = 26.;
pub const VELOCITY: f32 = 64.;
pub const SCROLLBAR: f32 = 10.;

#[derive(Clone)]
pub struct PianoState {
    pub viewport: Rc<Cell<Size<Pixels>>>,
    pub origin: Rc<Cell<gpui::Point<Pixels>>>,
    pub zoom: f32,
    pub key_zoom: f32,
    pub offset: Option<(f32, f32)>,
    pub selection: Option<String>,
}

impl Default for PianoState {
    fn default() -> Self {
        Self {
            viewport: Rc::new(Cell::new(gpui::size(px(700.), px(300.)))),
            origin: Rc::new(Cell::new(gpui::point(px(0.), px(0.)))),
            zoom: 1.,
            key_zoom: 1.,
            offset: None,
            selection: None,
        }
    }
}

impl PianoState {
    pub fn fit(&mut self) {
        self.zoom = 1.;
        self.key_zoom = 1.;
        self.offset = None;
    }

    pub fn layout(&self, length: f64, pitches: impl Iterator<Item = u8>) -> PianoLayout {
        let size = self.viewport.get();
        PianoLayout::new(
            f32::from(size.width),
            f32::from(size.height),
            length,
            pitches,
            self.zoom,
            self.key_zoom,
            self.offset,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PianoLayout {
    pub width: f32,
    pub height: f32,
    pub grid_width: f32,
    pub grid_height: f32,
    pub beat_width: f32,
    pub key_height: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub length: f64,
}

impl PianoLayout {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: f32,
        height: f32,
        length: f64,
        pitches: impl Iterator<Item = u8>,
        zoom: f32,
        key_zoom: f32,
        offset: Option<(f32, f32)>,
    ) -> Self {
        let (mut low, mut high) = (127, 0);
        for pitch in pitches {
            low = low.min(pitch);
            high = high.max(pitch);
        }
        if low > high {
            (low, high) = (60, 72);
        }
        let grid_width = (width - KEY_WIDTH - SCROLLBAR).max(1.);
        let grid_height = (height - RULER - VELOCITY - SCROLLBAR).max(1.);
        let key_height =
            (grid_height / f32::from((high - low).max(8) + 5)).clamp(8., 24.) * key_zoom;
        let key_height = key_height.clamp(8., 36.);
        let beat_width = (grid_width / length.max(0.001) as f32 * zoom).clamp(1., 1200.);
        let max_x = (length as f32 * beat_width - grid_width).max(0.);
        let max_y = (128. * key_height - grid_height).max(0.);
        let center = (127. - (f32::from(low) + f32::from(high)) * 0.5 + 0.5) * key_height;
        let (scroll_x, scroll_y) = offset.unwrap_or((0., center - grid_height * 0.5));
        Self {
            width,
            height,
            grid_width,
            grid_height,
            beat_width,
            key_height,
            scroll_x: scroll_x.clamp(0., max_x),
            scroll_y: scroll_y.clamp(0., max_y),
            max_x,
            max_y,
            length,
        }
    }
    pub fn x(self, beat: f64) -> f32 {
        KEY_WIDTH + beat as f32 * self.beat_width - self.scroll_x
    }
    pub fn y(self, pitch: u8) -> f32 {
        RULER + f32::from(127 - pitch) * self.key_height - self.scroll_y
    }
    pub fn beat(self, x: f32) -> f64 {
        f64::from((x - KEY_WIDTH + self.scroll_x) / self.beat_width).clamp(0., self.length)
    }
    pub fn visible_keys(self) -> impl Iterator<Item = u8> {
        (0..=127).rev().filter(move |&p| {
            self.y(p) < RULER + self.grid_height && self.y(p) + self.key_height > RULER
        })
    }
    pub fn grid_step(self) -> f64 {
        if self.beat_width >= 56. {
            0.25
        } else if self.beat_width >= 28. {
            0.5
        } else {
            (28. / self.beat_width).ceil() as f64
        }
    }
}

pub fn note_name(pitch: u8) -> String {
    format!(
        "{}{}",
        ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"][usize::from(pitch % 12)],
        i32::from(pitch) / 12 - 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fits_notes_and_short_patterns_without_a_dead_canvas() {
        let l = PianoLayout::new(800., 320., 4., [36, 42, 46].into_iter(), 1., 1., None);
        assert_eq!(l.max_x, 0.);
        assert!((l.x(4.) - (800. - SCROLLBAR)).abs() < 0.01);
        for pitch in [36, 42, 46] {
            assert!(l.visible_keys().any(|p| p == pitch));
        }
        for beat in [0., 0.125, 3.9, 4.] {
            assert!((l.beat(l.x(beat)) - beat).abs() < 0.0001);
        }
    }
    #[test]
    fn scroll_clamps_and_all_midi_keys_are_reachable() {
        let l = PianoLayout::new(
            400.,
            250.,
            128.,
            [0, 127].into_iter(),
            8.,
            1.,
            Some((1e6, 1e6)),
        );
        assert_eq!(l.scroll_x, l.max_x);
        assert!(l.visible_keys().any(|p| p == 0));
        let top = PianoLayout::new(
            400.,
            250.,
            128.,
            [0, 127].into_iter(),
            8.,
            1.,
            Some((-1., -1.)),
        );
        assert_eq!(top.scroll_x, 0.);
        assert!(top.visible_keys().any(|p| p == 127));
        assert_eq!(note_name(0), "C-1");
        assert_eq!(note_name(127), "G9");
    }
}
