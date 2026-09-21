//! Control-thread process lifecycle; none of this runs on the realtime port.
use super::{RealtimePort, Shared, Status};
use crate::{
    stream_codec as codec,
    stream_wire::{Ready, Start, STREAM_VERSION},
    Error, Result,
};
use std::{
    io,
    net::Shutdown,
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[path = "stream_io.rs"]
mod worker;
use worker::TimedSocket;

pub struct SessionOptions {
    pub queue_depth: usize,
    pub startup_timeout: Duration,
    pub block_timeout: Duration,
}
impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            queue_depth: 4,
            startup_timeout: Duration::from_secs(30),
            block_timeout: Duration::from_millis(100),
        }
    }
}
pub struct Session {
    controller: super::Controller,
    shared: Arc<Shared>,
    interrupt: UnixStream,
    worker: Option<JoinHandle<()>>,
    pid: u32,
}
pub(super) struct ChildGuard(pub(super) Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        // This command is a fresh process group; stop descendants as well as a stuck plugin destructor.
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Session {
    /// Control thread only. The helper must have both `host` and `stream` features enabled.
    pub fn spawn(
        path: &Path,
        start: Start,
        options: SessionOptions,
    ) -> Result<(Self, RealtimePort)> {
        if !cfg!(target_os = "macos") {
            return Err(crate::unsupported("macOS VST3 streams only"));
        }
        start.validate()?;
        if !path.is_absolute()
            || !(2..=16).contains(&options.queue_depth)
            || !(Duration::from_millis(1)..=Duration::from_secs(600))
                .contains(&options.startup_timeout)
            || !(Duration::from_millis(1)..=Duration::from_secs(10))
                .contains(&options.block_timeout)
        {
            return Err(crate::invalid(
                "invalid VST3 stream executable, depth or timeout",
            ));
        }
        let (parent, child_socket) = UnixStream::pair().map_err(io_error)?;
        let fd = child_socket.as_raw_fd();
        let mut command = Command::new(path);
        command
            .arg("--stream")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        // Only async-signal-safe fd operations between fork and exec. fd 3 is a dedicated duplex socket.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = ChildGuard(
            command
                .spawn()
                .map_err(|e| Error::new("PluginHostUnavailable", e))?,
        );
        drop(child_socket);
        let pid = child.0.id();
        let interrupt = parent.try_clone().map_err(io_error)?;
        let mut socket = TimedSocket {
            stream: parent,
            deadline: Instant::now() + options.startup_timeout,
        };
        codec::write_json(&mut socket, &start).map_err(io_error)?;
        let value = codec::read_json(&mut socket).map_err(io_error)?;
        if value.get("streamProtocolVersion").and_then(|v| v.as_u64())
            != Some(u64::from(STREAM_VERSION))
        {
            return Err(Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 stream handshake",
            ));
        }
        if let Some(error) = value.get("error") {
            let code = match error.get("code").and_then(|v| v.as_str()) {
                Some("PluginManifestMismatch") => "PluginManifestMismatch",
                Some("ProtocolVersionUnsupported") => "ProtocolVersionUnsupported",
                Some("PluginCapabilityUnsupported") => "PluginCapabilityUnsupported",
                Some("BudgetExceeded") => "BudgetExceeded",
                Some("AssetUnavailable") => "AssetUnavailable",
                Some("SourceChanged") => "SourceChanged",
                Some("RealtimeFault") => "RealtimeFault",
                _ => "PluginConfigInvalid",
            };
            return Err(Error::new(
                code,
                error
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("VST3 stream initialization failed"),
            ));
        }
        let mut info: Ready = serde_json::from_value(value).map_err(crate::invalid)?;
        info.validate(&start)?;
        let shared = Shared::new(
            info,
            start.options.block_size,
            options.queue_depth,
            start.options.midi_output,
        );
        let (controller, controls) =
            super::Controller::new(shared.clone(), interrupt.try_clone().map_err(io_error)?);
        let background = shared.clone();
        let deadline = socket.deadline;
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("vst3-stream-io".into())
            .spawn(move || {
                worker::run(
                    background,
                    socket,
                    child,
                    options.block_timeout,
                    ready_tx,
                    controls,
                );
            })
            .map_err(io_error)?;
        let mut session = Self {
            controller,
            shared: shared.clone(),
            interrupt,
            worker: Some(worker),
            pid,
        };
        // Never expose a port before worker scheduling and wait setup are complete. This wait
        // shares the handshake deadline, and every failure still interrupts, kills and reaps.
        let readiness = ready_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .and_then(|()| {
                if Instant::now() < deadline {
                    Ok(())
                } else {
                    Err(mpsc::RecvTimeoutError::Timeout)
                }
            });
        if let Err(error) = readiness {
            session.close();
            return Err(Error::new(
                if error == mpsc::RecvTimeoutError::Timeout {
                    "PluginHostTimeout"
                } else {
                    "PluginHostCrashed"
                },
                "VST3 IO worker did not become ready",
            ));
        }
        Ok((
            session,
            RealtimePort {
                shared,
                next_sequence: 0,
                output_events: [crate::wire::Event::Midi {
                    frame: 0,
                    message: [0x80, 0, 0],
                }; crate::stream_wire::MAX_EVENTS],
                output_event_count: 0,
                output_payload: Vec::with_capacity(
                    oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES,
                ),
            },
        ))
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    /// This handle belongs to this helper only; replacing a graph never retargets old handles.
    pub fn controller(&self) -> super::Controller {
        self.controller.clone()
    }
    pub fn status(&self) -> Status {
        self.shared.status()
    }
    pub fn info(&self) -> &Ready {
        &self.shared.info
    }
    /// Control-side, nontransactional timing snapshot. Helper processing includes descheduling.
    pub fn diagnostics(&self) -> super::Diagnostics {
        self.shared
            .diagnostics
            .snapshot(self.shared.info.helper_time_constraint)
    }
    /// Interrupt IO, kill/reap the isolated process group, and join. Never call from the audio thread.
    pub fn close(&mut self) {
        self.shared.stop(Status::Closed);
        let _ = self.interrupt.shutdown(Shutdown::Both);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

fn io_error(error: io::Error) -> Error {
    Error::new(
        match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => "PluginHostTimeout",
            _ => "PluginHostCrashed",
        },
        error,
    )
}
