use super::*;
use serde_json::{json, Value};

struct Reply(std::io::Cursor<Vec<u8>>);
impl Read for Reply {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        self.0.read(target)
    }
}
impl Write for Reply {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn edit_exchange(command: Command, page: Value) -> io::Result<Result<State>> {
    let response = json!({"controlProtocolVersion":1,"ok":true,"state":{"editorOpen":false,"nextSequence":2,"edits":page}});
    let mut bytes = MAGIC.to_vec();
    codec::write_json(&mut bytes, &response).unwrap();
    exchange(
        &mut Reply(std::io::Cursor::new(bytes)),
        &Request {
            control_protocol_version: 1,
            command,
        },
        &crate::stream::tests::shared().info,
        2,
    )
}
fn edit_page() -> Value {
    json!({"captureId":"1","status":"recording","firstSequence":0,"nextSequence":1,"pendingEvents":0,
    "events":[{"sequence":0,"parameterId":9,"kind":"begin","position":{"audioSequence":1,"reset":false,"transport":{
        "projectFrame":128,"continuousFrame":128,"projectBeat":0.01,"barBeat":0,"tempo":120,"timeSignature":[4,4],"playing":true
    }}}]})
}
#[test]
fn gesture_response_rejects_gaps_future_clocks_nulls_and_command_mismatch() {
    let read = || Command::ReadEdits {
        capture_id: "1".into(),
        from_sequence: 0,
    };
    edit_exchange(read(), edit_page()).unwrap().unwrap();
    for (pointer, value) in [
        ("/captureId", json!("2")),
        ("/firstSequence", json!(1)),
        ("/nextSequence", json!(2)),
        ("/events/0/sequence", json!(1)),
        ("/events/0/parameterId", json!(10)),
        ("/events/0/position/audioSequence", json!(2)),
        ("/events/0/position/transport/playing", json!(false)),
        ("/events/0/kind", json!("value")),
        ("/status", json!("stopped")),
        ("/pendingEvents", json!(4097)),
    ] {
        let mut page = edit_page();
        *page.pointer_mut(pointer).unwrap() = value;
        assert!(edit_exchange(read(), page).is_err(), "{pointer}");
    }
    for field in ["endPosition", "error"] {
        let mut page = edit_page();
        page[field] = Value::Null;
        assert!(edit_exchange(read(), page).is_err(), "{field}");
    }
    let mut page = edit_page();
    page["events"][0]["value"] = Value::Null;
    assert!(edit_exchange(read(), page).is_err());
    assert!(edit_exchange(
        Command::StopEdits {
            capture_id: "1".into(),
            from_sequence: 0
        },
        edit_page()
    )
    .is_err());
    assert!(edit_exchange(Command::StartEdits {}, edit_page()).is_err());
    assert!(edit_exchange(Command::Poll {}, edit_page()).is_err());
}

#[test]
fn bounded_control_queue_rejects_overflow_without_retiring_audio() {
    let shared = crate::stream::tests::shared();
    let (socket, _peer) = UnixStream::pair().unwrap();
    let (control, pending) = Controller::new(shared.clone(), socket);
    for _ in 0..8 {
        let (reply, _receiver) = mpsc::sync_channel(1);
        control
            .sender
            .try_send(Pending {
                request: Request {
                    control_protocol_version: CONTROL_VERSION,
                    command: Command::Poll {},
                },
                deadline: Instant::now() + Duration::from_secs(1),
                reply,
            })
            .unwrap();
    }
    assert_eq!(
        control
            .request(Command::Poll {}, Duration::from_secs(1))
            .unwrap_err()
            .code,
        "BudgetExceeded"
    );
    assert_eq!(shared.status(), Status::Running);
    // No rejected command was queued for delayed execution.
    for _ in 0..8 {
        pending.try_recv().unwrap();
    }
    assert!(pending.try_recv().is_err());
}
