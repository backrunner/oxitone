use crate::{
    backend::{Backend, Command},
    document_wire::{DocumentOperation, DocumentRequest},
    wire,
};
use serde_json::json;
use std::{os::unix::net::UnixStream, time::Duration};

#[test]
fn gui_document_requests_survive_local_transport_and_are_correlated_separately() {
    let name = format!(
        "oxi-doc-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::path::PathBuf::from("/tmp").join(name);
    let backend = Backend::start(path.clone(), true).unwrap();
    backend
        .commands
        .send(Command::Document(DocumentRequest {
            document_protocol_version: "2.0".into(),
            session_id: "session".into(),
            request_id: "stream/gpui/1".into(),
            base_revision: 7,
            operation: DocumentOperation::Save,
        }))
        .unwrap();
    backend
        .commands
        .send(Command::Frame(wire::Frame::Query, None))
        .unwrap();
    let mut stream = UnixStream::connect(path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    // Dedicated plugin RPCs must never consume reverse requests intended for DawRunner.
    for frame in [
        json!({"protocolVersion":"1.0","type":"vst3Instances","snapshotRevision":"1"}),
        json!({"protocolVersion":"1.0","type":"vst3Control","snapshotRevision":"1","request":{
            "instanceControlVersion":1,"graphGeneration":"1","instanceId":"missing","command":{"kind":"poll"}
        }}),
    ] {
        wire::write_frame(&mut stream, &frame).unwrap();
        let response = wire::read_frame(&mut stream).unwrap().unwrap();
        assert_eq!(response["type"], "rejected");
        assert!(response.get("documentRequests").is_none());
    }
    wire::write_frame(
        &mut stream,
        &json!({ "protocolVersion": "1.0", "type": "query" }),
    )
    .unwrap();
    let response = wire::read_frame(&mut stream).unwrap().unwrap();
    assert_eq!(response["documentProtocolVersion"], "2.0");
    assert_eq!(
        response["documentRequests"][0]["requestId"],
        "stream/gpui/1"
    );
    assert_eq!(response["documentRequests"][0]["baseRevision"], 7);
    wire::write_frame(
        &mut stream,
        &json!({ "protocolVersion": "1.0", "type": "query" }),
    )
    .unwrap();
    assert!(wire::read_frame(&mut stream)
        .unwrap()
        .unwrap()
        .get("documentRequests")
        .is_none());
    drop(stream);
    drop(backend);
}
