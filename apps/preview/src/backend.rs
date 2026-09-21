use crate::{
    engine::Engine,
    model::UiEvent,
    wire::{self, Frame},
};
use serde_json::Value;
use std::{
    io,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

pub enum Command {
    Document(crate::document_wire::DocumentRequest),
    Frame(Frame, Option<Sender<Value>>),
    Vst3Control {
        snapshot_revision: String,
        request: Value,
        received: std::time::Instant,
        reply: Sender<Value>,
    },
    Vst3Completed(Box<crate::engine::ControlCompleted>, Sender<Value>),
    Invalid(String),
    Shutdown,
}
pub struct Backend {
    pub commands: Sender<Command>,
    pub events: Receiver<UiEvent>,
    thread: Option<thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    socket: PathBuf,
}

impl Backend {
    pub fn start(socket: PathBuf, simulated: bool) -> io::Result<Self> {
        // A caller-owned, new private socket path; never delete an existing endpoint.
        let listener = UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let (commands, receiver) = mpsc::channel();
        let (events_tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let engine_stop = stop.clone();
        let task_sender = commands.clone();
        let thread = thread::Builder::new()
            .name("oxitone-preview-control".into())
            .spawn(move || {
                let document_events = events_tx.clone();
                let mut engine = Engine::new(simulated, events_tx);
                let mut document_requests = std::collections::VecDeque::new();
                let mut vst3_tasks = 0usize;
                while !engine_stop.load(Ordering::Relaxed) {
                    match receiver.recv_timeout(Duration::from_millis(33)) {
                        Ok(Command::Document(request)) => {
                            if document_requests.len() < 64 {
                                document_requests.push_back(request);
                            } else {
                                let _ = document_events.send(UiEvent::Document(
                                    crate::document_wire::DocumentMessage::Response {
                                        document_protocol_version: "2.0".into(),
                                        session_id: request.session_id,
                                        request_id: request.request_id,
                                        revision: request.base_revision,
                                        accepted: false,
                                        error: Some(crate::document_wire::DocumentError {
                                            code: "BudgetExceeded".into(),
                                            message: "document request queue is full".into(),
                                        }),
                                    },
                                ));
                            }
                        }
                        Ok(Command::Frame(Frame::Vst3Instances { snapshot_revision }, reply)) => {
                            if let Some(reply) = reply {
                                let _ = reply.send(engine.vst3_instances(snapshot_revision));
                            }
                        }
                        Ok(Command::Vst3Control {
                            snapshot_revision,
                            request,
                            received,
                            reply,
                        }) => {
                            let job = if vst3_tasks >= 8 {
                                Err(oxitone_core::OxitoneError::new(
                                    "BudgetExceeded",
                                    "Preview VST3 control task queue is full",
                                ))
                            } else {
                                engine.prepare_vst3_control(snapshot_revision, request, received)
                            };
                            match job {
                                Ok(job) => {
                                    let sender = task_sender.clone();
                                    let response = reply.clone();
                                    match thread::Builder::new()
                                        .name("oxitone-preview-vst3".into())
                                        .spawn(move || {
                                            let _ = sender.send(Command::Vst3Completed(
                                                Box::new(job.run()),
                                                response,
                                            ));
                                        }) {
                                        Ok(_) => vst3_tasks += 1,
                                        Err(error) => {
                                            let _ = reply.send(
                                                engine.error(wire::invalid(&error.to_string())),
                                            );
                                        }
                                    }
                                }
                                Err(error) => {
                                    let _ = reply.send(engine.error(error));
                                }
                            }
                        }
                        Ok(Command::Vst3Completed(completed, reply)) => {
                            vst3_tasks -= 1;
                            let _ = reply.send(engine.complete_vst3_control(*completed));
                        }
                        Ok(Command::Frame(frame, reply)) => {
                            let shutdown = matches!(frame, Frame::Shutdown);
                            let mut response = engine.handle(frame);
                            if let Some(reply) = reply {
                                if !document_requests.is_empty() {
                                    response["documentRequests"] = serde_json::to_value(
                                        document_requests.drain(..).collect::<Vec<_>>(),
                                    )
                                    .unwrap();
                                }
                                let _ = reply.send(response);
                            }
                            if shutdown {
                                break;
                            }
                        }
                        Ok(Command::Invalid(message)) => {
                            engine.error(wire::invalid(&message));
                        }
                        Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    engine.publish_playback();
                }
                // Engine/session/library retirement happens on this control thread.
                engine_stop.store(true, Ordering::Relaxed);
            })?;
        let sender = commands.clone();
        let listener_stop = stop.clone();
        thread::Builder::new().name("oxitone-preview-ipc".into()).spawn(move || {
            while !listener_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // BSD accept inherits O_NONBLOCK from the listener. Each client
                        // has a dedicated reader, so idle gaps must block instead of EOF.
                        if stream.set_nonblocking(false).is_err() { continue; }
                        let sender = sender.clone();
                        thread::spawn(move || loop {
                            let value = match wire::read_frame(&mut stream) {
                                Ok(Some(value)) => value,
                                Ok(None) => break,
                                Err(error) => { let _ = sender.send(Command::Invalid(error.to_string())); break; }
                            };
                            let frame = match wire::decode(value) {
                                Ok(frame) => frame,
                                Err(error) => {
                                    let _ = wire::write_frame(&mut stream, &serde_json::json!({"protocolVersion":"1.0", "type":"rejected","code":error.code,"message":error.message}));
                                    let _ = sender.send(Command::Invalid(error.message));
                                    continue;
                                }
                            };
                            let shutdown = matches!(frame, Frame::Shutdown);
                            let (reply, response) = mpsc::channel();
                            let command = match frame {
                                Frame::Vst3Control { snapshot_revision, request } => Command::Vst3Control {
                                    snapshot_revision, request, received: std::time::Instant::now(), reply,
                                },
                                frame => Command::Frame(frame, Some(reply)),
                            };
                            if sender.send(command).is_err() { break; }
                            match response.recv() {
                                Ok(value) if wire::write_frame(&mut stream, &value).is_ok() => {},
                                _ => break,
                            }
                            if shutdown { break; }
                        });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(25)),
                    Err(_) => break,
                }
            }
        })?;
        Ok(Self {
            commands,
            events,
            thread: Some(thread),
            stop,
            socket,
        })
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}
