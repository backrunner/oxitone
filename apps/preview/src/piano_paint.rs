//! Piano rendering is clipped to the viewport; keys/ruler/velocity stay pinned.
use crate::{piano_layout::*, theme::Theme, ui::alpha};
use gpui::*;

pub(crate) struct Painter<'a> {
    pub origin: Point<Pixels>,
    pub window: &'a mut Window,
    cx: &'a mut App,
}
impl Painter<'_> {
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        if w > 0. && h > 0. {
            self.window.paint_quad(fill(
                Bounds::new(self.origin + point(px(x), px(y)), size(px(w), px(h))),
                color,
            ));
        }
    }
    pub fn text(
        &mut self,
        x: f32,
        y: f32,
        label: impl Into<SharedString>,
        color: u32,
        height: f32,
    ) {
        let label = label.into();
        let run = TextRun {
            len: label.len(),
            font: font("Helvetica Neue"),
            color: rgb(color).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = self.window.text_system().shape_line(
            label,
            px((height - 2.).clamp(7., 11.)),
            &[run],
            None,
        );
        let _ = line.paint(
            self.origin + point(px(x), px(y)),
            px(height),
            self.window,
            self.cx,
        );
    }
}

pub fn paint(
    bounds: Bounds<Pixels>,
    l: PianoLayout,
    theme: Theme,
    notes: &[crate::piano_note_paint::PaintedNote],
    sounding: &[bool; 128],
    phase: Option<f64>,
    snap: Snap,
    window: &mut Window,
    cx: &mut App,
) {
    let grid = Bounds::new(
        bounds.origin + point(px(KEY_WIDTH), px(RULER)),
        size(px(l.grid_width), px(l.grid_height)),
    );
    window.with_content_mask(Some(ContentMask { bounds: grid }), |window| {
        let mut p = Painter {
            origin: bounds.origin,
            window,
            cx,
        };
        for pitch in l.visible_keys() {
            let black = [1, 3, 6, 8, 10].contains(&(pitch % 12));
            let y = l.y(pitch);
            p.rect(
                KEY_WIDTH,
                y,
                l.grid_width,
                l.key_height,
                rgb(theme.piano_rows[usize::from(black)]),
            );
            p.rect(
                KEY_WIDTH,
                y + l.key_height - 1.,
                l.grid_width,
                1.,
                alpha(theme.border, if pitch % 12 == 0 { 1. } else { 0.4 }),
            );
        }
        let step = l.grid_step().max(snap.step());
        let first = (l.beat(KEY_WIDTH) / step).floor() as usize;
        let last = (l.beat(KEY_WIDTH + l.grid_width) / step).ceil() as usize;
        for tick in first..=last {
            let beat = tick as f64 * step;
            p.rect(
                l.x(beat),
                RULER,
                1.,
                l.grid_height,
                alpha(
                    theme.border,
                    if beat.rem_euclid(4.) == 0. {
                        1.
                    } else if beat.fract() == 0. {
                        0.65
                    } else {
                        0.28
                    },
                ),
            );
        }
        crate::piano_note_paint::paint_notes(&mut p, l, theme, notes, sounding, phase);
        if let Some(phase) = phase {
            p.rect(l.x(phase), RULER, 2., l.grid_height, rgb(theme.gold));
        }
    });
    let keys = Bounds::new(
        bounds.origin + point(px(0.), px(RULER)),
        size(px(KEY_WIDTH), px(l.grid_height)),
    );
    window.with_content_mask(Some(ContentMask { bounds: keys }), |window| {
        let mut p = Painter {
            origin: bounds.origin,
            window,
            cx,
        };
        p.rect(0., RULER, KEY_WIDTH, l.grid_height, rgb(theme.keys[0]));
        for pitch in l.visible_keys() {
            let y = l.y(pitch);
            let black = [1, 3, 6, 8, 10].contains(&(pitch % 12));
            let on = sounding[usize::from(pitch)];
            let width = if black { KEY_WIDTH * 0.72 } else { KEY_WIDTH };
            p.rect(
                0.,
                y,
                width,
                l.key_height - 1.,
                rgb(if on {
                    theme.accent
                } else {
                    theme.keys[usize::from(black)]
                }),
            );
            if l.key_height >= 8. && (pitch % 12 == 0 || on) {
                p.text(
                    8.,
                    y,
                    note_name(pitch),
                    if on {
                        theme.on_accent
                    } else {
                        theme.key_text[usize::from(black)]
                    },
                    l.key_height,
                );
            }
        }
    });
    let mut p = Painter {
        origin: bounds.origin,
        window,
        cx,
    };
    let velocity_y = RULER + l.grid_height;
    p.rect(0., 0., l.width, RULER, rgb(theme.panel));
    p.rect(0., velocity_y, l.width, l.velocity_height, rgb(theme.panel));
    p.rect(0., velocity_y, l.width, 1., rgb(theme.border));
    p.text(9., 0., "Notes", theme.muted, RULER);
    p.text(8., velocity_y + 5., "Velocity", theme.muted, 18.);
    let mask = Bounds::new(
        bounds.origin + point(px(KEY_WIDTH), px(0.)),
        size(px(l.grid_width), px(velocity_y + l.velocity_height)),
    );
    p.window
        .with_content_mask(Some(ContentMask { bounds: mask }), |window| {
            let mut p = Painter {
                origin: bounds.origin,
                window,
                cx: p.cx,
            };
            for value in [0., 0.25, 0.5, 0.75, 1.] {
                let y = velocity_y + l.velocity_height - 5. - value * (l.velocity_height - 12.);
                p.rect(KEY_WIDTH, y, l.grid_width, 1., alpha(theme.border, 0.35));
            }
            let step = l.grid_step().max(1.);
            let first = (l.beat(KEY_WIDTH) / step).floor() as usize;
            let last = (l.beat(KEY_WIDTH + l.grid_width) / step).ceil() as usize;
            for tick in first..=last {
                let beat = tick as f64 * step;
                p.text(
                    l.x(beat) + 5.,
                    0.,
                    format!("{}", beat as usize + 1),
                    theme.muted,
                    RULER,
                );
                p.rect(l.x(beat), RULER - 5., 1., 5., rgb(theme.border));
            }
            for item in notes {
                let note = &item.note;
                let x = l.x(note.start);
                if x < KEY_WIDTH - 7. || x > KEY_WIDTH + l.grid_width {
                    continue;
                }
                let h = (note.velocity as f32 * (l.velocity_height - 12.)).max(2.);
                let color = if item.selected {
                    theme.gold
                } else {
                    theme.accent
                };
                p.rect(
                    x + 2.,
                    velocity_y + l.velocity_height - 5. - h,
                    2.,
                    h,
                    alpha(color, 0.7),
                );
                p.window.paint_quad(quad(
                    Bounds::new(
                        p.origin + point(px(x), px(velocity_y + l.velocity_height - 7. - h)),
                        size(px(6.), px(5.)),
                    ),
                    px(2.),
                    rgb(color),
                    px(0.),
                    rgb(color),
                    BorderStyle::default(),
                ));
            }
            if let Some(phase) = phase {
                p.rect(l.x(phase), 0., 2., RULER, rgb(theme.gold));
            }
        });
    let mut p = Painter {
        origin: bounds.origin,
        window,
        cx,
    };
    p.rect(
        l.width - SCROLLBAR,
        RULER,
        SCROLLBAR,
        l.grid_height,
        rgb(theme.bg),
    );
    let (top, vertical) =
        crate::workspace::thumb(l.grid_height, l.grid_height, l.max_y, -l.scroll_y);
    let y = RULER + top;
    p.rect(l.width - 7., y, 4., vertical, rgb(theme.muted));
    p.rect(
        KEY_WIDTH,
        l.height - SCROLLBAR,
        l.grid_width,
        SCROLLBAR,
        rgb(theme.bg),
    );
    let (left, horizontal) =
        crate::workspace::thumb(l.grid_width, l.grid_width, l.max_x, -l.scroll_x);
    let x = KEY_WIDTH + left;
    p.rect(x, l.height - 7., horizontal, 4., rgb(theme.muted));
}
