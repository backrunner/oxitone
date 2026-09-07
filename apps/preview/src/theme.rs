//! Semantic colors selected from the window's current system appearance.
use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: u32,
    pub panel: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub gold: u32,
    pub danger: u32,
    pub diagnostic_bg: u32,
    pub diagnostic_text: u32,
    pub button: u32,
    pub button_hover: u32,
    pub selected: u32,
    pub lanes: [u32; 2],
    pub piano_rows: [u32; 2],
    pub keys: [u32; 2],
    pub key_text: [u32; 2],
    pub scope: u32,
    pub meter: u32,
    pub secondary: u32,
    pub tracks: [u32; 5],
    pub clip_opacity: f32,
}

impl Theme {
    pub fn from_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self {
                bg: 0x0d1118,
                panel: 0x151b25,
                border: 0x303c4d,
                text: 0xe4ebf4,
                muted: 0xa2afc1,
                accent: 0x69c7b8,
                on_accent: 0x102b28,
                gold: 0xe8b977,
                danger: 0xf19a88,
                diagnostic_bg: 0x3b272a,
                diagnostic_text: 0xf4c0b3,
                button: 0x252f3e,
                button_hover: 0x334154,
                selected: 0x20383c,
                lanes: [0x141c27, 0x111822],
                piano_rows: [0x1c2430, 0x111720],
                keys: [0x303b49, 0x0a1018],
                key_text: [0xc0cbd9, 0xa2afc1],
                scope: 0x0a1017,
                meter: 0x080f16,
                secondary: 0x7baad4,
                tracks: [0x69c7b8, 0xe8b977, 0xb5aff2, 0x80bce2, 0xe2a1b4],
                clip_opacity: 0.18,
            },
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self {
                bg: 0xf4f6f9,
                panel: 0xffffff,
                border: 0xcbd4df,
                text: 0x202d40,
                muted: 0x4c5c70,
                accent: 0x096452,
                on_accent: 0xffffff,
                gold: 0x805007,
                danger: 0xa53427,
                diagnostic_bg: 0xffe8e2,
                diagnostic_text: 0x8e3023,
                button: 0xe5ebf2,
                button_hover: 0xd5dfe9,
                selected: 0xdceee9,
                lanes: [0xffffff, 0xf0f3f7],
                piano_rows: [0xfafcfe, 0xe8edf3],
                keys: [0xffffff, 0x303b49],
                key_text: [0x4c5c70, 0xe4ebf4],
                scope: 0xfafcfe,
                meter: 0xd8e1eb,
                secondary: 0x276394,
                tracks: [0x096452, 0x805007, 0x6045a0, 0x215d8b, 0x973956],
                clip_opacity: 0.08,
            },
        }
    }

    pub fn button(
        self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Stateful<Div> {
        div()
            .id(ElementId::Name(id.into()))
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(rgb(self.border))
            .bg(rgb(self.button))
            .text_color(rgb(self.text))
            .hover(move |style| style.bg(rgb(self.button_hover)))
            .active(move |style| style.bg(rgb(self.selected)))
            .text_xs()
            .cursor_pointer()
            .child(label.into())
    }

    pub fn label(self, text: impl Into<SharedString>) -> Div {
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(self.muted))
            .child(text.into())
    }

    pub fn track(self, index: usize) -> u32 {
        self.tracks[index % self.tracks.len()]
    }
}
