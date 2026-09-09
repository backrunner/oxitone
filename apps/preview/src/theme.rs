//! Semantic colors selected from the window's current system appearance.
use gpui::WindowAppearance;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: u32,
    pub panel: u32,
    pub raised: u32,
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
                bg: 0x111215,
                panel: 0x1a1c20,
                raised: 0x22252b,
                border: 0x34373f,
                text: 0xe9ebef,
                muted: 0xa6adb9,
                accent: 0x69c7b8,
                on_accent: 0x102b28,
                gold: 0xe8b977,
                danger: 0xf19a88,
                diagnostic_bg: 0x3b272a,
                diagnostic_text: 0xf4c0b3,
                button: 0x26292f,
                button_hover: 0x343840,
                selected: 0x233632,
                lanes: [0x1b1d22, 0x17191d],
                piano_rows: [0x24272d, 0x1b1d22],
                keys: [0x343941, 0x111317],
                key_text: [0xc0cbd9, 0xa2afc1],
                scope: 0x131519,
                meter: 0x080f16,
                secondary: 0x7baad4,
                tracks: [0x69c7b8, 0xe8b977, 0xb5aff2, 0x80bce2, 0xe2a1b4],
                clip_opacity: 0.18,
            },
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self {
                bg: 0xf4f6f9,
                panel: 0xffffff,
                raised: 0xf5f8fc,
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

    pub fn track(self, index: usize) -> u32 {
        self.tracks[index % self.tracks.len()]
    }
}
