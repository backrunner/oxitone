//! Panel sizing uses the measured workspace, never fixed title/status deductions.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum EditorMode {
    #[default]
    Split,
    Piano,
    Mixer,
}
pub const DIVIDER: f32 = 8.;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum DockMode {
    #[default]
    Piano,
    Mixer,
    Split,
}
pub const ANALYSIS_BAR: f32 = 24.;

#[derive(Clone, Copy, Debug)]
pub struct PanelLayout {
    pub height: f32,
    pub editor: f32,
    pub piano: f32,
    pub mixer: f32,
    pub scopes: f32,
    pub max_editor: f32,
}
impl PanelLayout {
    pub fn new(width: f32, height: f32, editor: f32, fraction: f32, scopes: Option<f32>) -> Self {
        let width = width.max(1.);
        let analysis_bar = if scopes.is_some() { ANALYSIS_BAR } else { 0. };
        let scopes = scopes.map_or(0., |s| s.clamp(60., (height * 0.35).max(60.)) + DIVIDER);
        let height = (height - analysis_bar - scopes).max(1.);
        let max_editor = (height - 120. - DIVIDER).max(120.);
        let editor = editor.clamp(220_f32.min(max_editor), max_editor);
        let available = (width - DIVIDER).max(1.);
        let piano = (available * fraction).clamp(
            360_f32.min(available * 0.5),
            (available - 340.).max(available * 0.5),
        );
        Self {
            height,
            editor,
            piano,
            mixer: available - piano,
            scopes: (scopes - DIVIDER).max(0.),
            max_editor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_has_useful_travel_and_preserves_minimum_visible_regions() {
        let left = PanelLayout::new(1060., 560., 800., 0., None);
        let right = PanelLayout::new(1060., 560., 10., 1., None);
        assert!(right.piano - left.piano > 300.);
        assert!(left.height - left.editor - DIVIDER >= 120.);
        assert_eq!(right.editor, 220.);
        assert!((left.piano + left.mixer + DIVIDER - 1060.).abs() < 0.01);
        let tiny = PanelLayout::new(300., 190., 800., 1., Some(300.));
        assert!(tiny.piano > 0. && tiny.mixer > 0. && tiny.editor > 0.);
    }
}
