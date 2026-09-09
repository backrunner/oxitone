//! Pure piano coordinates; all 128 MIDI keys remain reachable through scrolling.
pub use crate::piano_state::{NoteTool, PianoState, Snap};

pub const KEY_WIDTH: f32 = 76.;
pub const RULER: f32 = 26.;
pub const VELOCITY: f32 = 64.;
pub const SCROLLBAR: f32 = 10.;

pub fn velocity_height(height: f32, requested: f32) -> f32 {
    requested.clamp(36., (height * 0.4).max(36.))
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
    pub velocity_height: f32,
}

impl PianoLayout {
    #[cfg(test)]
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
        Self::with_velocity(
            width, height, length, pitches, zoom, key_zoom, offset, VELOCITY,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn with_velocity(
        width: f32,
        height: f32,
        length: f64,
        pitches: impl Iterator<Item = u8>,
        zoom: f32,
        key_zoom: f32,
        offset: Option<(f32, f32)>,
        velocity_height: f32,
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
        let velocity_height = self::velocity_height(height, velocity_height);
        let grid_height = (height - RULER - velocity_height - SCROLLBAR).max(1.);
        let key_height =
            (grid_height / f32::from((high - low).max(8) + 5)).clamp(8., 24.) * key_zoom;
        let key_height = key_height.clamp(8., 36.);
        // Musical scale is independent of phrase length: accepted edits never zoom the view.
        let beat_width = (96. * zoom).clamp(1., 1200.);
        // Virtual horizon grows with navigation; only visible rows/ticks are painted.
        let requested_x = offset.map_or(0., |o| o.0).max(0.);
        let extent = ((length as f32 + 16.) * beat_width)
            .max(requested_x + grid_width * 3.)
            .max(grid_width * 4.);
        let max_x = extent - grid_width;
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
            velocity_height,
        }
    }
    pub fn x(self, beat: f64) -> f32 {
        KEY_WIDTH + beat as f32 * self.beat_width - self.scroll_x
    }
    pub fn y(self, pitch: u8) -> f32 {
        RULER + f32::from(127 - pitch) * self.key_height - self.scroll_y
    }
    pub fn beat(self, x: f32) -> f64 {
        f64::from((x - KEY_WIDTH + self.scroll_x) / self.beat_width).max(0.)
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
