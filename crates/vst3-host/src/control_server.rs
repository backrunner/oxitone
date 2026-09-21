//! Main-thread live controls. Audio and UI share one processor without sharing mutable ownership.
use crate::{
    control_wire::{Command, Request, State, CONTROL_VERSION, MAGIC},
    native, stream_codec as codec,
    stream_wire::Ready,
    Result,
};
use std::{
    io::Write,
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::{Duration, Instant},
};
use vst3_host::{audio::BusAudioBuffers, Plugin};
#[path = "control_audio.rs"]
mod audio;
#[path = "control_restart.rs"]
mod restart;

#[cfg(target_os = "macos")]
use crate::editor::window::{self, EditorWindow, Mode};

pub(crate) struct Controls {
    silent: BusAudioBuffers,
    #[cfg(target_os = "macos")]
    editor: Option<EditorWindow>,
    next_pump: Instant,
    edits: crate::edit_journal::Journal,
    frozen: Option<restart::Frozen>,
}
impl Controls {
    pub fn new(plugin: &mut Plugin, ready: &Ready) -> Result<Self> {
        let mut controls = Self {
            silent: crate::silent_processing::buffers(plugin)?,
            #[cfg(target_os = "macos")]
            editor: None,
            next_pump: Instant::now(),
            edits: Default::default(),
            frozen: None,
        };
        // Initialization must not occupy the event queue when the first PCM automation arrives.
        controls.flush(plugin, ready)?;
        if controls.restart_required() {
            return Err(restart::required());
        }
        Ok(controls)
    }
    fn is_open(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.editor.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    pub fn close(&mut self, plugin: &mut Plugin) -> Result<()> {
        #[cfg(target_os = "macos")]
        if let Some(mut editor) = self.editor.take() {
            editor.close(plugin)?;
        }
        Ok(())
    }
    pub fn wait(&mut self, socket: &UnixStream, plugin: &mut Plugin, ready: &Ready) -> Result<()> {
        loop {
            if !self.is_open() {
                return Ok(());
            }
            if self.is_open() && Instant::now() >= self.next_pump {
                #[cfg(target_os = "macos")]
                if self
                    .editor
                    .as_mut()
                    .unwrap()
                    .poll(
                        plugin,
                        objc2::MainThreadMarker::new()
                            .ok_or_else(|| crate::invalid("editor requires main thread"))?,
                        0.,
                    )?
                    .is_some()
                {
                    self.close(plugin)?;
                }
                self.next_pump = Instant::now() + Duration::from_millis(16);
                self.flush(plugin, ready)?;
                self.edits.collect(plugin, ready);
            }
            let mut fd = libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let timeout = if self.is_open() { 16 } else { -1 };
            // SAFETY: live socket and one initialized pollfd, on the helper main thread only.
            let result = unsafe { libc::poll(&mut fd, 1, timeout) };
            if result > 0 {
                return Ok(());
            }
            if result < 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                return Err(crate::invalid(std::io::Error::last_os_error()));
            }
        }
    }
    pub fn handle(
        &mut self,
        socket: &mut UnixStream,
        plugin: &mut Plugin,
        ready: &Ready,
        sequence: u64,
    ) -> Result<()> {
        let result = codec::read_json(socket)
            .map_err(crate::invalid)
            .and_then(|value| {
                let request: Request = serde_json::from_value(value).map_err(crate::invalid)?;
                request.validate()?;
                self.execute(request.command, plugin, ready, sequence)
            });
        let response = match &result {
            Ok(state) => {
                serde_json::json!({"controlProtocolVersion":CONTROL_VERSION,"ok":true,"state":state})
            }
            Err(error) => {
                serde_json::json!({"controlProtocolVersion":CONTROL_VERSION,"ok":false,"error":error})
            }
        };
        socket.write_all(MAGIC).map_err(crate::invalid)?;
        codec::write_json(socket, &response).map_err(crate::invalid)?;
        // A plugin processing/capability failure invalidates the immutable Ready/PDC contract.
        match result {
            Err(error) if matches!(error.code, "RealtimeFault" | "SourceChanged") => Err(error),
            _ => Ok(()),
        }
    }
    fn execute(
        &mut self,
        command: Command,
        plugin: &mut Plugin,
        ready: &Ready,
        sequence: u64,
    ) -> Result<State> {
        let mut info = None;
        if self.check_restart(plugin, ready)? {
            self.sync_values(plugin, ready)?;
        }
        if self.restart_required()
            && matches!(
                command,
                Command::StartEdits {}
                    | Command::StartRecording { .. }
                    | Command::OpenEditor {}
                    | Command::SetParameter { .. }
            )
        {
            return Err(restart::required());
        }
        self.edits.collect(plugin, ready);
        let mut edit_page = None;
        match command {
            Command::StartEdits {} => edit_page = Some(self.edits.start(plugin)?),
            Command::StartRecording {
                mode,
                ref parameter_ids,
            } => {
                edit_page =
                    Some(
                        self.edits
                            .start_recording(plugin, ready, mode, parameter_ids.clone())?,
                    )
            }
            Command::ReadEdits {
                ref capture_id,
                from_sequence,
            } => edit_page = Some(self.edits.read(capture_id, from_sequence, false)?),
            Command::StopEdits {
                ref capture_id,
                from_sequence,
            } => edit_page = Some(self.edits.read(capture_id, from_sequence, true)?),
            Command::DiscardEdits { ref capture_id } => {
                if self.restart_required() {
                    self.edits.discard_frozen(capture_id)?;
                } else {
                    self.edits.discard(capture_id, plugin)?;
                }
            }
            Command::OpenEditor {} => {
                #[cfg(target_os = "macos")]
                if self.editor.is_none() {
                    self.editor = Some(EditorWindow::open(
                        plugin,
                        window::initialize()?,
                        Mode::Live,
                    )?);
                }
                #[cfg(not(target_os = "macos"))]
                return Err(crate::unsupported("macOS VST3 editors only"));
            }
            Command::CloseEditor {} => self.close(plugin)?,
            Command::Poll {} => {}
            Command::SetParameter {
                parameter_id,
                value,
            } => {
                if !ready
                    .parameters
                    .iter()
                    .any(|p| p.id == parameter_id && p.writable)
                {
                    return Err(crate::invalid("Unknown or read-only VST3 parameter"));
                }
                // A restored dense preset may already fill all 4096 slots. Drain its seed
                // before enqueueing the first live override, without advancing audio time.
                self.flush(plugin, ready)?;
                if self.restart_required() {
                    return Err(restart::required());
                }
                plugin.set_parameter(parameter_id, value).map_err(native)?;
            }
            Command::Capture {} => {}
        }
        self.flush(plugin, ready)?;
        self.edits.collect(plugin, ready);
        if self.restart_required() {
            if let Some(page) = &edit_page {
                edit_page = Some(
                    self.edits
                        .read(&page.capture_id, page.first_sequence, false)?,
                );
            }
        }
        if matches!(command, Command::Capture {}) {
            // getState is allowed during Processing on the UI thread. Do not reset voices/tails.
            info = Some(if let Some(frozen) = &self.frozen {
                frozen.info.clone()
            } else {
                let captured = crate::inspection::inspect(plugin, ready.sha256.clone())?;
                self.after_audio(plugin, ready)?;
                self.frozen
                    .as_ref()
                    .map_or(captured, |frozen| frozen.info.clone())
            });
        }
        if sequence > 9_007_199_254_740_991 {
            return Err(crate::invalid("VST3 control sequence exhausted"));
        }
        Ok(State {
            editor_open: self.is_open(),
            next_sequence: sequence,
            info,
            edits: edit_page,
            restart: self.frozen.as_ref().map(|f| f.summary.clone()),
        })
    }
}
