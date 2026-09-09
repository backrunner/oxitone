//! Piano tools, view scale and scroll anchors in musical coordinates.
use crate::piano_layout::{PianoLayout, VELOCITY};
use gpui::{px, Pixels, Size};
use std::{cell::Cell, rc::Rc};
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum NoteTool {
    #[default]
    Draw,
    Paint,
    Select,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Snap {
    Bar,
    Beat,
    Half,
    #[default]
    Quarter,
    Eighth,
    Free,
}
impl Snap {
    pub fn step(self) -> f64 {
        match self {
            Self::Bar => 4.,
            Self::Beat => 1.,
            Self::Half => 0.5,
            Self::Quarter => 0.25,
            Self::Eighth => 0.125,
            Self::Free => 1. / 960.,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Bar => "4 beats",
            Self::Beat => "1 beat",
            Self::Half => "½ beat",
            Self::Quarter => "¼ beat",
            Self::Eighth => "⅛ beat",
            Self::Free => "Free",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Bar => Self::Beat,
            Self::Beat => Self::Half,
            Self::Half => Self::Quarter,
            Self::Quarter => Self::Eighth,
            Self::Eighth => Self::Free,
            Self::Free => Self::Bar,
        }
    }
    pub fn round(self, beat: f64) -> f64 {
        (beat / self.step()).round() * self.step()
    }
}

#[derive(Clone)]
pub struct PianoState {
    pub viewport: Rc<Cell<Size<Pixels>>>,
    pub origin: Rc<Cell<gpui::Point<Pixels>>>,
    pub zoom: f32,
    pub key_zoom: f32,
    offset: Option<(f32, f32)>,
    scroll_basis: Option<PianoLayout>,
    last_layout: Rc<Cell<Option<PianoLayout>>>,
    pub selection: Option<String>,
    pub channel: Option<String>,
    pub tool: NoteTool,
    pub snap: Snap,
    /// Magnet controls whether note gestures resolve to the selected grid.
    /// `Snap::Free` remains available as a fine 1/960-beat resolution.
    pub magnet: bool,
    pub note_length: f64,
    pub velocity_height: f32,
    pub pointer: Option<gpui::Point<Pixels>>,
    pub snap_menu: Option<gpui::Point<Pixels>>,
}

impl Default for PianoState {
    fn default() -> Self {
        Self {
            viewport: Rc::new(Cell::new(gpui::size(px(700.), px(300.)))),
            origin: Rc::new(Cell::new(gpui::point(px(0.), px(0.)))),
            zoom: 1.,
            key_zoom: 1.,
            offset: None,
            scroll_basis: None,
            last_layout: Rc::new(Cell::new(None)),
            selection: None,
            channel: None,
            tool: NoteTool::default(),
            snap: Snap::default(),
            magnet: true,
            note_length: 0.25,
            velocity_height: VELOCITY,
            pointer: None,
            snap_menu: None,
        }
    }
}

impl PianoState {
    pub fn set_offset(&mut self, offset: (f32, f32)) {
        self.offset = Some(offset);
        self.scroll_basis = self.last_layout.get();
    }
    pub fn effective_snap(&self, fine_override: bool) -> Snap {
        if fine_override || !self.magnet {
            Snap::Free
        } else {
            self.snap
        }
    }
    pub fn fit(&mut self) {
        self.fit_length(self.last_layout.get().map_or(8., |l| l.length));
    }
    pub fn fit_length(&mut self, length: f64) {
        let width = f32::from(self.viewport.get().width)
            - crate::piano_layout::KEY_WIDTH
            - crate::piano_layout::SCROLLBAR;
        self.zoom = (width / (length.max(8.) as f32 * 96.)).clamp(1. / 96., 128.);
        self.key_zoom = 1.;
        self.offset = None;
        self.scroll_basis = None;
    }

    pub fn layout(&self, length: f64, pitches: impl Iterator<Item = u8>) -> PianoLayout {
        let size = self.viewport.get();
        let mut layout = PianoLayout::with_velocity(
            f32::from(size.width),
            f32::from(size.height),
            length,
            pitches,
            self.zoom,
            self.key_zoom,
            self.offset,
            self.velocity_height,
        );
        if let (Some((x, y)), Some(basis)) = (self.offset, self.scroll_basis) {
            if basis.width != layout.width || basis.height != layout.height {
                layout.scroll_x = if x <= 0. {
                    0.
                } else {
                    (((x + basis.grid_width * 0.5) / basis.beat_width) * layout.beat_width
                        - layout.grid_width * 0.5)
                        .clamp(0., layout.max_x)
                };
                layout.scroll_y = (((y + basis.grid_height * 0.5) / basis.key_height)
                    * layout.key_height
                    - layout.grid_height * 0.5)
                    .clamp(0., layout.max_y);
            }
        }
        self.last_layout.set(Some(layout));
        layout
    }
}
