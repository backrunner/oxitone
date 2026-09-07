//! Development smoke for routing navigation using the real routed example.
use crate::{mixer_inspector::InspectorTab, mixer_model, ui::Preview};
use gpui::*;
pub fn step(this: &mut Preview, frame: usize, mode: &str, cx: &mut Context<Preview>) {
    let Some(project) = &this.project else {
        return;
    };
    if frame == 2 {
        let strips = mixer_model::strips(project);
        let selected = strips
            .iter()
            .find(|s| s.name == "Music bus")
            .expect("use mixer-preview.ts");
        let id = selected.id.clone();
        assert_eq!(selected.sends().count(), 2);
        this.select_mixer(&id);
        this.workspace.inspector_tab = InspectorTab::Routing;
        this.workspace.mixer_expanded = mode == "expanded";
        cx.notify();
    } else if frame == 5 {
        let strips = mixer_model::strips(project);
        let selected = strips.iter().find(|s| s.id == this.selected_scope).unwrap();
        let send = selected
            .sends()
            .find(|r| r.destination_name == "Room reverb")
            .unwrap();
        assert_eq!(send.ratio, 0.24);
        assert!(send.automated);
        let id = send.destination.clone();
        this.select_mixer(&id);
        cx.notify();
    } else if frame == 7 {
        let strips = mixer_model::strips(project);
        let room = strips.iter().find(|s| s.id == this.selected_scope).unwrap();
        assert_eq!(room.name, "Room reverb");
        assert_eq!(room.inputs.len(), 2);
        let music = room
            .inputs
            .iter()
            .find(|r| r.source_name == "Music bus")
            .unwrap()
            .source
            .clone();
        this.select_mixer(&music);
        if mode == "chain" {
            this.workspace.inspector_tab = InspectorTab::Chain;
        }
        cx.notify();
    } else if frame == 12 {
        assert!(!this.playback.playing);
        assert_eq!(project.snapshot.revision, 1);
        eprintln!("Preview mixer routing smoke passed");
    }
}

pub fn keyboard(frame: usize, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
    match frame {
        9 => view.read(cx).inspector_focus.focus(window),
        10 => {
            window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        }
        11 => {
            let scroll = &view.read(cx).workspace.inspector;
            assert!(
                (f32::from(scroll.offset().y + scroll.max_offset().height)).abs() < 1.,
                "inspector End scrolls its own content"
            );
            window.dispatch_keystroke(Keystroke::parse("home").unwrap(), cx);
        }
        13 => {
            assert_eq!(view.read(cx).workspace.inspector.offset().y, px(0.));
            eprintln!("Preview inspector keyboard smoke passed");
        }
        _ => {}
    }
}
