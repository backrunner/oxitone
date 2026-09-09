use crate::window_manager::*;
use gpui::{point, px, size, Bounds};

fn manager() -> WindowManager {
    let state = WindowManager::default();
    state.desktop.set(Bounds {
        origin: point(px(0.), px(100.)),
        size: size(px(1000.), px(600.)),
    });
    state
}
#[test]
fn maximize_restore_and_host_resize_preserve_user_geometry() {
    let mut m = manager();
    let id = WindowId::Piano;
    let original = WindowBounds {
        x: 20.,
        y: 30.,
        width: 700.,
        height: 400.,
    };
    m.set_bounds(id, original);
    m.toggle_maximize(id);
    assert_eq!(
        m.bounds(id),
        WindowBounds {
            x: 0.,
            y: 0.,
            width: 1000.,
            height: 600.
        }
    );
    m.toggle_maximize(id);
    assert_eq!(m.bounds(id), original);
    m.desktop.set(Bounds {
        origin: point(px(0.), px(100.)),
        size: size(px(480.), px(300.)),
    });
    assert_eq!(
        m.bounds(id),
        WindowBounds {
            x: 0.,
            y: 0.,
            width: 480.,
            height: 300.
        }
    );
    m.desktop.set(manager().desktop.get());
    assert_eq!(m.bounds(id), original);
}
#[test]
fn resize_from_every_edge_clamps_and_escape_restores() {
    let mut m = manager();
    for (left, right, top, bottom) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
        (true, false, true, false),
        (true, false, false, true),
        (false, true, true, false),
        (false, true, false, true),
    ] {
        let id = WindowId::Mixer;
        let b = WindowBounds {
            x: 100.,
            y: 80.,
            width: 700.,
            height: 400.,
        };
        m.set_bounds(id, b);
        m.begin(
            id,
            point(px(100.), px(100.)),
            GestureKind::Resize {
                left,
                right,
                top,
                bottom,
            },
        );
        m.move_drag(point(px(-4000.), px(4000.)));
        let next = m.bounds(id);
        assert!(
            next.x >= 0.
                && next.y >= 0.
                && next.x + next.width <= 1000.
                && next.y + next.height <= 600.
        );
        m.cancel();
        assert_eq!(m.bounds(id), b);
    }
}
#[test]
fn hit_testing_respects_desktop_origin_and_window_order() {
    let mut m = manager();
    m.set_bounds(
        WindowId::Piano,
        WindowBounds {
            x: 0.,
            y: 0.,
            width: 700.,
            height: 400.,
        },
    );
    m.set_bounds(
        WindowId::Patterns,
        WindowBounds {
            x: 20.,
            y: 20.,
            width: 220.,
            height: 400.,
        },
    );
    m.visible = vec![WindowId::Piano, WindowId::Patterns];
    assert_eq!(m.hit(point(px(30.), px(130.))), Some(WindowId::Patterns));
    assert_eq!(m.hit(point(px(300.), px(130.))), Some(WindowId::Piano));
    assert_eq!(m.hit(point(px(30.), px(30.))), None);
    m.hidden = true;
    assert_eq!(m.hit(point(px(30.), px(130.))), None);
}
