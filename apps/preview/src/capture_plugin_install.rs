//! Exercise the real package-entry keyboard path with a captured control sink, never npm/network.
use crate::{
    backend::Command,
    document_wire::{DocumentError, DocumentMessage, DocumentOperation},
    ui::Preview,
    window_manager::WindowId,
};
use gpui::*;

pub fn prepare(view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
    view.update(cx, |s, cx| {
        assert_eq!(s.document.view.as_ref().unwrap().status, "invalid");
        assert!(s.document.pending.is_none());
        s.document.manager.open = true;
        s.document.manager.adding = true;
        s.document.manager.searching = true;
        s.document.manager.package_input = "@fixture/missing@1.0.0".into();
        s.document.windows.focus(WindowId::Plugins);
        s.workspace_focus.focus(window);
        cx.notify();
    });
}
pub fn invalid_project(view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
    assert_eq!(
        view.read(cx).document.windows.front(),
        Some(WindowId::Plugins)
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    let original = view.update(cx, |s, _| {
        std::mem::replace(&mut s.backend.commands, sender)
    });
    window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
    // Restore the real control sender before checking; no installer command leaves this helper.
    view.update(cx, |s, _| s.backend.commands = original);
    let Command::Document(request) = receiver
        .try_recv()
        .expect("invalid projects must be able to submit a package installation")
    else {
        panic!("package entry must submit a document request");
    };
    assert!(
        matches!(&request.operation, DocumentOperation::InstallPlugin { package_name, version }
        if package_name == "@fixture/missing" && version.as_deref() == Some("1.0.0"))
    );
    view.update(cx, |s, _| {
        assert!(!s.document.manager.adding && s.document.pending.is_some());
        s.observe_document(DocumentMessage::Response {
            document_protocol_version: "2.0".into(),
            request_id: request.request_id,
            session_id: request.session_id,
            revision: request.base_revision,
            accepted: false,
            error: Some(DocumentError {
                code: "PluginInstallFailed".into(),
                message: "Simulated install rejection".into(),
            }),
        });
        assert!(s.document.pending.is_none());
        assert_eq!(
            s.active_diagnostic().unwrap().message,
            "Simulated install rejection"
        );
    });
    eprintln!("Plugin install input smoke passed: invalid draft, actual Enter dispatch, captured request and rejected response; no npm/network");
}
