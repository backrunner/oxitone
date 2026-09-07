//! Piano rendering is clipped to the viewport; keys/ruler/velocity stay pinned.
use crate::{piano_layout::*, theme::Theme, ui::alpha};
use gpui::*;
use oxitone_core::wire::PatternSpec;

struct Painter<'a> {
    origin: Point<Pixels>,
    window: &'a mut Window,
    cx: &'a mut App,
}
impl Painter<'_> {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        if w > 0. && h > 0. {
            self.window.paint_quad(fill(
                Bounds::new(self.origin + point(px(x), px(y)), size(px(w), px(h))),
                color,
            ));
        }
    }
    fn text(&mut self, x: f32, y: f32, label: impl Into<SharedString>, color: u32, height: f32) {
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
    pattern: &PatternSpec,
    sounding: &[bool; 128],
    phase: Option<f64>,
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
        let step = l.grid_step();
        let first = (l.beat(KEY_WIDTH) / step).floor() as usize;
        let last = (l.beat(KEY_WIDTH + l.grid_width) / step).ceil() as usize;
        for tick in first..=last {
            let beat = tick as f64 * step;
            p.rect(
                l.x(beat),
                RULER,
                1.,
                l.grid_height,
                alpha(theme.border, if beat.fract() == 0. { 1. } else { 0.4 }),
            );
        }
        for note in &pattern.notes {
            let x = l.x(note.start.to_f64());
            let width = (note.duration.to_f64() as f32 * l.beat_width - 1.).max(7.);
            let y = l.y(note.pitch);
            if x > KEY_WIDTH + l.grid_width
                || x + width < KEY_WIDTH
                || y > RULER + l.grid_height
                || y + l.key_height < RULER
            {
                continue;
            }
            let active = sounding[usize::from(note.pitch)]
                && phase.is_some_and(|beat| {
                    beat >= note.start.to_f64()
                        && beat < note.start.to_f64() + note.duration.to_f64()
                });
            p.rect(
                x,
                y + 2.,
                width,
                (l.key_height - 4.).max(3.),
                alpha(theme.accent, 0.55 + note.velocity as f32 * 0.45),
            );
            if active {
                p.rect(x, y + 2., width, 2., rgb(theme.gold));
            }
            if width >= 34. && l.key_height >= 16. {
                p.text(
                    x + 4.,
                    y,
                    note_name(note.pitch),
                    theme.on_accent,
                    l.key_height,
                );
            }
        }
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
            if l.key_height >= 8. {
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
    p.rect(0., velocity_y, l.width, VELOCITY, rgb(theme.panel));
    p.rect(0., velocity_y, l.width, 1., rgb(theme.border));
    p.text(9., 0., "NOTE", theme.muted, RULER);
    p.text(8., velocity_y + 5., "VELOCITY", theme.muted, 18.);
    p.text(8., velocity_y + 26., "0 – 127", theme.muted, 18.);
    let mask = Bounds::new(
        bounds.origin + point(px(KEY_WIDTH), px(0.)),
        size(px(l.grid_width), px(velocity_y + VELOCITY)),
    );
    p.window
        .with_content_mask(Some(ContentMask { bounds: mask }), |window| {
            let mut p = Painter {
                origin: bounds.origin,
                window,
                cx: p.cx,
            };
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
            for note in &pattern.notes {
                let x = l.x(note.start.to_f64());
                if x < KEY_WIDTH - 7. || x > KEY_WIDTH + l.grid_width {
                    continue;
                }
                let h = (note.velocity as f32 * (VELOCITY - 12.)).max(2.);
                p.rect(x, velocity_y + VELOCITY - 5. - h, 4., h, rgb(theme.accent));
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
