use crate::{
    close_state::CloseState,
    document_wire::{DocumentMessage, DocumentView},
};
use serde_json::json;

fn view() -> DocumentView {
    serde_json::from_value(json!({
        "sessionId":"session", "revision":3, "acceptedRevision":3, "savedRevision":2,
        "status":"ready", "modified":true, "saving":false, "sites":[], "automationSites":[],
        "configurationSites":[], "rackSites":[], "files":[], "conflicts":[], "plugins":[]
    }))
    .unwrap()
}
fn response(request: &str, accepted: bool) -> DocumentMessage {
    serde_json::from_value(json!({
        "type":"response", "documentProtocolVersion":"2.0", "sessionId":"session",
        "requestId":request, "accepted":accepted, "revision":3,
        "error": if accepted { json!(null) } else { json!({"code":"SourceChanged","message":"Disk changed"}) }
    })).unwrap()
}
fn event(view: DocumentView) -> DocumentMessage {
    DocumentMessage::Event {
        document_protocol_version: "2.0".into(),
        view,
    }
}

#[test]
fn closing_waits_for_its_save_response_and_matching_clean_view_in_either_order() {
    for response_first in [false, true] {
        let mut state = CloseState::default();
        let mut view = view();
        state.begin_save("save".into(), &view);
        state.observe(&response("another", true));
        assert!(!state.complete(Some(&view), false));
        if response_first {
            state.observe(&response("save", true));
            assert!(!state.complete(Some(&view), false));
        }
        view.modified = false;
        view.saved_revision = 3;
        state.observe(&event(view.clone()));
        if !response_first {
            assert!(!state.complete(Some(&view), false));
            state.observe(&response("save", true));
        }
        assert!(!state.complete(Some(&view), true));
        assert!(state.complete(Some(&view), false));
    }
}

#[test]
fn rejected_or_cancelled_save_never_closes_and_late_ack_cannot_reactivate_it() {
    let mut state = CloseState::default();
    let mut view = view();
    state.begin_save("save".into(), &view);
    state.observe(&response("save", false));
    assert_eq!(state.error.as_deref(), Some("Disk changed"));
    assert!(!state.saving());
    state.begin_save("retry".into(), &view);
    state.cancel();
    state.observe(&response("retry", true));
    view.modified = false;
    view.saved_revision = 3;
    assert!(!state.complete(Some(&view), false));
}

#[test]
fn newer_edits_or_a_restarted_service_cancel_automatic_close() {
    for restart in [false, true] {
        let mut state = CloseState::default();
        let mut view = view();
        state.begin_save("save".into(), &view);
        if restart {
            view.session_id = "new-session".into();
        } else {
            view.revision = 4;
        }
        state.observe(&event(view.clone()));
        state.observe(&response("save", true));
        assert!(state.error.is_some());
        assert!(!state.saving());
        assert!(!state.complete(Some(&view), false));
    }
}
