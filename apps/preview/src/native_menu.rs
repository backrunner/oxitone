//! Infrequent commands live in native menus and use the same document/transport paths.
use crate::{
    about::{Quit, ShowAbout, ShowShortcuts},
    document_wire::DocumentOperation,
    ui::Preview,
};
use gpui::*;

actions!(oxitone, [SaveProject, UndoEdit, RedoEdit, ToggleLoop]);

pub fn install(cx: &mut App) {
    cx.set_menus(vec![
        Menu {
            name: "Oxitone".into(),
            items: vec![
                MenuItem::action("About Oxitone", ShowAbout),
                MenuItem::separator(),
                MenuItem::action("Quit Oxitone", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![MenuItem::action("Save", SaveProject)],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", UndoEdit),
                MenuItem::action("Redo", RedoEdit),
            ],
        },
        Menu {
            name: "Playback".into(),
            items: vec![MenuItem::action("Toggle loop", ToggleLoop)],
        },
        Menu {
            name: "Help".into(),
            items: vec![MenuItem::action("Keyboard shortcuts", ShowShortcuts)],
        },
    ]);
}

impl Preview {
    fn menu_command_available(&self) -> bool {
        !self.close.open
            && !self.show_about
            && !self.show_shortcuts
            && self.view_menu.selected.is_none()
            && self.pattern_picker.selected.is_none()
            && self.document.tempo.input.is_none()
            && self.document.configuration.input.is_none()
            && !self.document.manager.searching
    }

    fn menu_document_operation(&mut self, operation: DocumentOperation, cx: &mut Context<Self>) {
        if self.menu_command_available() {
            self.document_request(operation);
            cx.notify();
        }
    }

    pub fn save_project_action(&mut self, _: &SaveProject, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_document_operation(DocumentOperation::Save, cx);
    }

    pub fn undo_edit_action(&mut self, _: &UndoEdit, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_document_operation(DocumentOperation::Undo, cx);
    }

    pub fn redo_edit_action(&mut self, _: &RedoEdit, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_document_operation(DocumentOperation::Redo, cx);
    }

    pub fn loop_action(&mut self, _: &ToggleLoop, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_command_available() && self.project.is_some() {
            self.toggle_loop();
            cx.notify();
        }
    }
}
