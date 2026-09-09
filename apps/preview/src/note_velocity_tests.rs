use crate::{
    document_wire::{SourceNote, SourceOutput},
    note_velocity::{stroke, velocity_at},
    piano_layout::PianoLayout,
};
use std::collections::BTreeSet;

fn layout() -> PianoLayout {
    PianoLayout::new(800., 400., 4., [60].into_iter(), 1., 1., None)
}
fn outputs() -> Vec<SourceOutput> {
    [0., 1., 1., 2.]
        .into_iter()
        .enumerate()
        .map(|(i, start)| SourceOutput {
            note: SourceNote {
                start,
                pitch: 60 + i as u8,
                duration: 0.5,
                velocity: 0.7,
            },
            select: serde_json::json!({ "step": i }),
        })
        .collect()
}
#[test]
fn absolute_height_clamps_and_tracks_lane_resize() {
    let l = layout();
    let bottom = l.height - crate::piano_layout::SCROLLBAR - 5.;
    assert_eq!(velocity_at(l, bottom), 0.);
    assert_eq!(
        velocity_at(l, bottom - (l.velocity_height - 12.) * 0.5),
        0.5
    );
    assert_eq!(velocity_at(l, -500.), 1.);
    assert_eq!(velocity_at(l, 1000.), 0.);
}
#[test]
fn fast_strokes_cross_chords_in_both_directions_without_duplicate_edits() {
    let l = layout();
    let bottom = l.height - crate::piano_layout::SCROLLBAR - 5.;
    let a = (l.x(0.), bottom);
    let b = (l.x(2.), bottom - (l.velocity_height - 12.));
    let mut changes = vec![];
    stroke(&outputs(), &BTreeSet::new(), &mut changes, a, b, l);
    assert_eq!(changes.len(), 4);
    for (c, v) in changes.iter().zip([0., 0.5, 0.5, 1.]) {
        assert!((c.after.velocity - v).abs() < 0.001);
    }
    stroke(&outputs(), &BTreeSet::new(), &mut changes, b, a, l);
    assert_eq!(changes.len(), 4);
    assert!((changes[1].after.velocity - 0.5).abs() < 0.001);
}

#[test]
fn chord_members_share_the_hit_area_even_with_different_durations() {
    let l = layout();
    let mut notes = outputs();
    notes[1].note.duration = 0.01;
    notes[2].note.duration = 2.;
    let mut changes = vec![];
    let at = (l.x(1.) + 20., 0.);
    stroke(&notes, &BTreeSet::new(), &mut changes, at, at, l);
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|c| c.after.velocity == 1.));
}
#[test]
fn selection_limits_chord_members_and_wide_targets_allow_vertical_drawing() {
    let l = layout();
    let mut changes = vec![];
    let selected = BTreeSet::from([2]);
    stroke(
        &outputs(),
        &selected,
        &mut changes,
        (l.x(1.) + 10., 500.),
        (l.x(1.) + 10., 0.),
        l,
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].index, Some(2));
    assert_eq!(changes[0].after.velocity, 1.);
}
