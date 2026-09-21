//! Cloneable control handle for exactly one helper lifetime. No realtime callers.
#[path = "control_info.rs"]
mod info;
#[cfg(test)]
#[path = "stream_control_tests.rs"]
mod tests;
use super::{Shared, Status};
use crate::{
    control_wire::{Command, Request, State, CONTROL_VERSION, MAGIC},
    stream_codec as codec, Error, Result,
};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    time::{Duration, Instant},
};

pub(super) struct Pending {
    pub request: Request,
    pub deadline: Instant,
    pub reply: SyncSender<Result<State>>,
}

#[derive(Clone)]
pub struct Controller {
    shared: Arc<Shared>,
    interrupt: Arc<UnixStream>,
    sender: SyncSender<Pending>,
}
impl Controller {
    pub(super) fn new(shared: Arc<Shared>, socket: UnixStream) -> (Self, Receiver<Pending>) {
        let (sender, receiver) = mpsc::sync_channel(8);
        (
            Self {
                shared,
                interrupt: Arc::new(socket),
                sender,
            },
            receiver,
        )
    }
    /// Blocks only this control caller. Timeout invalidates and interrupts the entire session.
    pub fn request(&self, command: Command, timeout: Duration) -> Result<State> {
        command.validate()?;
        if !(Duration::from_millis(1)..=Duration::from_secs(600)).contains(&timeout) {
            return Err(crate::invalid("invalid VST3 control timeout"));
        }
        if self.shared.status() != Status::Running {
            return Err(Error::new("PluginHostCrashed", "VST3 session is closed"));
        }
        if self
            .shared
            .restart_required
            .load(std::sync::atomic::Ordering::Acquire)
            && matches!(
                command,
                Command::SetParameter { .. }
                    | Command::OpenEditor {}
                    | Command::StartEdits {}
                    | Command::StartRecording { .. }
            )
        {
            return Err(Error::new(
                "PluginRestartRequired",
                "VST3 state is frozen; capture it before rebuilding the graph",
            ));
        }
        if let Command::SetParameter { parameter_id, .. } = &command {
            if !self
                .shared
                .info
                .parameters
                .iter()
                .any(|p| p.id == *parameter_id && p.writable)
            {
                return Err(crate::invalid("Unknown or read-only VST3 parameter"));
            }
        }
        let deadline = Instant::now() + timeout;
        let (reply, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Pending {
                request: Request {
                    control_protocol_version: CONTROL_VERSION,
                    command,
                },
                deadline,
                reply,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => Error::new("BudgetExceeded", "VST3 control queue is full"),
                TrySendError::Disconnected(_) => {
                    Error::new("PluginHostCrashed", "VST3 control worker is closed")
                }
            })?;
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(result) if Instant::now() < deadline => result,
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {
                self.shared.stop(Status::TimedOut);
                let _ = self.interrupt.shutdown(Shutdown::Both);
                Err(Error::new(
                    "PluginHostTimeout",
                    "VST3 control deadline exceeded; session retired",
                ))
            }
            Err(_) => Err(Error::new(
                "PluginHostCrashed",
                "VST3 control worker exited",
            )),
        }
    }
}

pub(super) fn exchange(
    socket: &mut (impl Read + Write),
    request: &Request,
    ready: &crate::stream_wire::Ready,
    next_sequence: u64,
) -> io::Result<Result<State>> {
    socket.write_all(MAGIC)?;
    codec::write_json(socket, request)?;
    let mut magic = [0; 4];
    socket.read_exact(&mut magic)?;
    let response = codec::read_json(socket)?;
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid VST3 control response");
    if &magic != MAGIC
        || response.as_object().is_none_or(|v| v.len() != 3)
        || response
            .get("controlProtocolVersion")
            .and_then(|v| v.as_u64())
            != Some(CONTROL_VERSION.into())
    {
        return Err(invalid());
    }
    match response.get("ok").and_then(|v| v.as_bool()) {
        Some(true) => {
            let state: State =
                serde_json::from_value(response.get("state").cloned().ok_or_else(invalid)?)
                    .map_err(|_| invalid())?;
            if state.next_sequence > 9_007_199_254_740_991 {
                return Err(invalid());
            }
            if response["state"].as_object().is_none_or(|s| {
                s.len()
                    != 2 + usize::from(state.info.is_some())
                        + usize::from(state.edits.is_some())
                        + usize::from(state.restart.is_some())
            }) {
                return Err(invalid());
            }
            if state.restart.as_ref().is_some_and(|r| !r.valid()) {
                return Err(invalid());
            }
            if state.next_sequence != next_sequence
                || matches!(request.command, Command::Capture {}) != state.info.is_some()
                || matches!(
                    request.command,
                    Command::StartEdits {}
                        | Command::StartRecording { .. }
                        | Command::ReadEdits { .. }
                        | Command::StopEdits { .. }
                ) != state.edits.is_some()
            {
                return Err(invalid());
            }
            if let Some(page) = &state.edits {
                page.validate(ready).map_err(|_| invalid())?;
                if matches!(request.command, Command::StopEdits { .. })
                    && page.status == crate::edit_wire::Status::Recording
                {
                    return Err(invalid());
                }
                match &request.command {
                    Command::StartEdits {}
                        if page.first_sequence == 0
                            && page.next_sequence == 0
                            && page.recording.is_none()
                            && page.status == crate::edit_wire::Status::Recording => {}
                    Command::StartRecording {
                        mode,
                        parameter_ids,
                    } if page.first_sequence == 0
                        && page.next_sequence == 0
                        && page.status == crate::edit_wire::Status::Recording
                        && page.recording.as_ref().is_some_and(|recording| {
                            recording.mode == *mode && recording.parameter_ids == *parameter_ids
                        }) => {}
                    Command::ReadEdits {
                        capture_id,
                        from_sequence,
                    }
                    | Command::StopEdits {
                        capture_id,
                        from_sequence,
                    } if page.capture_id == *capture_id
                        && page.first_sequence == *from_sequence => {}
                    _ => return Err(invalid()),
                }
                if page
                    .events
                    .iter()
                    .any(|event| event.position.audio_sequence >= next_sequence)
                    || page
                        .end_position
                        .is_some_and(|end| end.audio_sequence >= next_sequence)
                {
                    return Err(invalid());
                }
            }
            if let Some(value) = &state.info {
                info::validate(value, ready, state.restart.is_some())?;
            }
            Ok(Ok(state))
        }
        Some(false) => {
            let error = response.get("error").ok_or_else(invalid)?;
            if error.as_object().is_none_or(|v| v.len() != 2) {
                return Err(invalid());
            }
            let code = match error.get("code").and_then(|v| v.as_str()) {
                Some("PluginCapabilityUnsupported") => "PluginCapabilityUnsupported",
                Some("ProtocolVersionUnsupported") => "ProtocolVersionUnsupported",
                Some("BudgetExceeded") => "BudgetExceeded",
                Some("RealtimeFault") => "RealtimeFault",
                Some("PluginRestartRequired") => "PluginRestartRequired",
                Some("SourceChanged") => "SourceChanged",
                Some("PluginConfigInvalid") => "PluginConfigInvalid",
                Some("PluginTaskConflict") => "PluginTaskConflict",
                _ => return Err(invalid()),
            };
            Ok(Err(Error::new(
                code,
                error
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(invalid)?,
            )))
        }
        _ => Err(invalid()),
    }
}
