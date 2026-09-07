//! Readability checks cover both palettes without coupling tests to exact colors.
use crate::theme::Theme;
use gpui::WindowAppearance;

fn luminance(color: u32) -> f64 {
    [16, 8, 0]
        .into_iter()
        .zip([0.2126, 0.7152, 0.0722])
        .map(|(shift, weight)| {
            let c = f64::from((color >> shift) & 255) / 255.;
            weight
                * if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
        })
        .sum()
}

fn readable(foreground: u32, background: u32, minimum: f64) {
    let a = luminance(foreground);
    let b = luminance(background);
    let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
    assert!(
        contrast >= minimum,
        "{foreground:06x} on {background:06x}: {contrast:.2} < {minimum}"
    );
}

fn composite(foreground: u32, background: u32, opacity: f32) -> u32 {
    [16, 8, 0].into_iter().fold(0, |result, shift| {
        let fg = ((foreground >> shift) & 255) as f32;
        let bg = ((background >> shift) & 255) as f32;
        result | (((fg * opacity + bg * (1. - opacity)).round() as u32) << shift)
    })
}

#[test]
fn text_remains_readable_on_both_system_themes() {
    for appearance in [WindowAppearance::Light, WindowAppearance::Dark] {
        let t = Theme::from_appearance(appearance);
        for surface in [t.bg, t.panel, t.selected, t.scope, t.button, t.button_hover] {
            readable(t.text, surface, 4.5);
        }
        for surface in [t.bg, t.panel, t.selected] {
            readable(t.muted, surface, 4.5);
            readable(t.accent, surface, 4.5);
            readable(t.gold, surface, 4.5);
        }
        readable(t.diagnostic_text, t.diagnostic_bg, 4.5);
        readable(t.on_accent, t.accent, 4.5);
        for key in 0..2 {
            readable(t.key_text[key], t.keys[key], 4.5);
        }
        for lane in t.lanes {
            for tint in t.tracks {
                let fill = composite(tint, lane, t.clip_opacity);
                readable(tint, fill, 4.5);
                readable(t.muted, fill, 4.5);
                readable(tint, composite(tint, fill, 0.08), 4.5);
            }
        }
        for signal in [t.accent, t.secondary, t.gold, t.danger] {
            readable(signal, t.scope, 3.);
            readable(signal, t.meter, 3.);
        }
    }
}
