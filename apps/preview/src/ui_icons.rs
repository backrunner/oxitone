//! Small vector glyphs share geometry and weight across the native interface.
use gpui::{prelude::*, *};
#[derive(Clone, Copy)]
pub enum Icon {
    Wave,
    Effect,
    Route,
    Output,
    Input,
}
pub fn icon(kind: Icon, color: u32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |at, _, window, _| {
            let lines: &[&[(f32, f32)]] = match kind {
                Icon::Wave => &[&[
                    (1., 8.),
                    (3., 8.),
                    (5., 3.),
                    (7., 13.),
                    (9., 5.),
                    (11., 11.),
                    (13., 8.),
                    (15., 8.),
                ]],
                Icon::Effect => &[
                    &[(2., 4.), (14., 4.)],
                    &[(2., 8.), (14., 8.)],
                    &[(2., 12.), (14., 12.)],
                    &[(5., 2.), (5., 6.)],
                    &[(11., 6.), (11., 10.)],
                    &[(7., 10.), (7., 14.)],
                ],
                Icon::Route => &[
                    &[(2., 8.), (7., 8.), (7., 3.), (14., 3.)],
                    &[(7., 8.), (7., 13.), (14., 13.)],
                    &[(11., 1.), (14., 3.), (11., 5.)],
                    &[(11., 11.), (14., 13.), (11., 15.)],
                ],
                Icon::Output => &[
                    &[(2., 8.), (12., 8.)],
                    &[(8., 4.), (12., 8.), (8., 12.)],
                    &[(14., 2.), (14., 14.)],
                ],
                Icon::Input => &[
                    &[(3., 2.), (3., 14.)],
                    &[(14., 8.), (5., 8.)],
                    &[(9., 4.), (5., 8.), (9., 12.)],
                ],
            };
            let mut path = PathBuilder::stroke(px(1.35));
            for line in lines {
                for (i, (x, y)) in line.iter().enumerate() {
                    let p = at.origin + point(px(*x), px(*y));
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color));
            }
        },
    )
    .size(px(16.))
    .flex_shrink_0()
}
