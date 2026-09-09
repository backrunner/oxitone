use crate::{
    document_wire::SourceNote,
    note_edit::{NoteChange, NoteGesture},
    note_transform::NoteAction,
    piano_note_paint::visible_notes,
    playlist_actions::Clip,
    playlist_edit::{Drag, DragMode, ResourceKind},
    playlist_projection::project,
};
use gpui::{point, px};

fn clip() -> Clip {
    Clip {
        kind: ResourceKind::Pattern,
        id: "clip".into(),
        resource: "pattern".into(),
        track: "a".into(),
        start: 0.,
        length: 4.,
        enabled: true,
    }
}
fn drag(mode: DragMode) -> Drag {
    Drag {
        mode,
        kind: ResourceKind::Pattern,
        resource: "pattern".into(),
        clip: Some(("clip".into(), 0)),
        length: 4.,
        anchor: point(px(0.), px(0.)),
        offset: 0.,
        target: Some(("b".into(), 8.)),
        moved: true,
    }
}
#[test]
fn placement_move_copy_resize_and_resource_insertion_keep_actual_content() {
    let mut d = drag(DragMode::Move);
    let moved = project(vec![clip()], Some(&d));
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].track, "b");
    assert_eq!(moved[0].start, 8.);
    assert_eq!(moved[0].resource, "pattern");
    assert_eq!(moved[0].id, "clip");
    d.mode = DragMode::Copy;
    let copied = project(vec![clip()], Some(&d));
    assert_eq!(copied.len(), 2);
    assert_eq!(copied[0].track, "a");
    assert_eq!(copied[1].track, "b");
    assert_ne!(copied[0].id, copied[1].id);
    assert_eq!(copied[0].resource, copied[1].resource);
    d.mode = DragMode::Resize;
    d.length = 2.;
    assert_eq!(project(vec![clip()], Some(&d))[0].length, 2.);
    d.clip = None;
    assert_eq!(project(vec![], Some(&d))[0].resource, "pattern");
    d.target = None;
    assert_eq!(
        project(vec![clip()], Some(&d))[0].start,
        0.,
        "invalid drop retains source"
    );
}
#[test]
fn notes_move_in_source_order_with_duplicate_resize_velocity_and_erase() {
    let note = |pitch, start| SourceNote {
        pitch,
        start,
        duration: 1.,
        velocity: 0.6,
    };
    let source = vec![note(72, 2.), note(60, 0.), note(60, 0.)];
    let mut g = NoteGesture {
        select_after: true,
        site: "source".into(),
        placement: None,
        action: NoteAction::Move,
        pointer: point(px(0.), px(0.)),
        scroll: (0., 0.),
        changes: vec![NoteChange {
            index: Some(1),
            before: source[1].clone(),
            after: SourceNote {
                pitch: 61,
                start: 3.,
                duration: 2.,
                velocity: 0.8,
            },
            selector: serde_json::Value::Null,
        }],
    };
    let moved = visible_notes(&source, &[1].into(), Some(&g));
    assert_eq!(moved.len(), 3);
    assert_eq!(moved.iter().filter(|p| p.note == source[1]).count(), 1);
    assert_eq!(moved.last().unwrap().note, g.changes[0].after);
    assert!(moved.last().unwrap().selected);
    g.action = NoteAction::Duplicate;
    assert_eq!(
        visible_notes(&source, &Default::default(), Some(&g)).len(),
        4
    );
    g.action = NoteAction::Erase;
    assert_eq!(
        visible_notes(&source, &Default::default(), Some(&g)).len(),
        2
    );
}
#[test]
fn track_operations_omit_unmodified_boolean_fields() {
    let edit = crate::project_edit::ProjectEdit::Track {
        index: 2,
        enabled: None,
        mute: Some(true),
        solo: None,
    };
    assert_eq!(
        serde_json::to_value(edit).unwrap(),
        serde_json::json!({"kind":"track", "index":2, "mute":true})
    );
}
