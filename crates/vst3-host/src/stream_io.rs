//! Worker-owned socket and total request deadline. No audio-thread system calls.
use super::ChildGuard;
use crate::{
    stream::{Shared, Status},
    stream_codec as codec, stream_thread,
};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::{atomic::Ordering, mpsc::SyncSender, Arc},
    time::{Duration, Instant},
};

pub(super) fn run(
    shared: Arc<Shared>,
    mut socket: TimedSocket,
    mut child: ChildGuard,
    timeout: Duration,
    ready: SyncSender<()>,
    controls: std::sync::mpsc::Receiver<crate::stream::control::Pending>,
) {
    let configured = stream_thread::configure(shared.info.sample_rate, shared.max_frames);
    shared
        .diagnostics
        .time_constraint
        .store(configured, Ordering::Relaxed);
    let idle = stream_thread::IdleWait::new();
    let mut check_exit = Instant::now();
    let mut next_sequence = 0;
    if ready.send(()).is_err() {
        return;
    }
    while shared.status() == Status::Running {
        // At most one control command per audio block; a busy UI cannot starve pending PCM.
        if let Ok(command) = controls.try_recv() {
            socket.deadline = command.deadline;
            match crate::stream::control::exchange(
                &mut socket,
                &command.request,
                &shared.info,
                next_sequence,
            ) {
                Ok(result) => {
                    if result.as_ref().is_ok_and(|s| s.restart.is_some())
                        || result
                            .as_ref()
                            .is_err_and(|e| e.code == "PluginRestartRequired")
                    {
                        shared.restart_required.store(true, Ordering::Release);
                    }
                    let fatal = result
                        .as_ref()
                        .is_err_and(|e| matches!(e.code, "RealtimeFault" | "SourceChanged"));
                    if fatal {
                        shared.stop(Status::PluginFault);
                    }
                    let _ = command.reply.send(result);
                    if fatal {
                        break;
                    }
                }
                Err(error) => {
                    shared.stop(
                        if matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        ) {
                            Status::TimedOut
                        } else {
                            Status::InvalidResponse
                        },
                    );
                    let _ = command.reply.send(Err(crate::Error::new(
                        if shared.status() == Status::TimedOut {
                            "PluginHostTimeout"
                        } else {
                            "PluginHostCrashed"
                        },
                        error,
                    )));
                    break;
                }
            }
        }
        let Some(mut block) = shared.pending.pop() else {
            // In-flight EOF is immediate. An idle crash needs no 10 kHz waitpid polling.
            let before = Instant::now();
            if before >= check_exit {
                if !matches!(child.0.try_wait(), Ok(None)) {
                    shared.stop(Status::Crashed);
                    break;
                }
                check_exit = before + Duration::from_millis(10);
            }
            idle.wait();
            shared.diagnostics.idle(before.elapsed());
            continue;
        };
        let identity = (block.sequence, block.frames, block.transport, block.reset);
        socket.deadline = Instant::now() + timeout;
        let before = Instant::now();
        let result = codec::write_block(&mut socket, &block, false).and_then(|()| {
            shared.diagnostics.sent(before.elapsed());
            let before = Instant::now();
            codec::read_block(&mut socket, &mut block, true)?;
            Ok(before.elapsed())
        });
        match result {
            Ok(elapsed)
                if identity == (block.sequence, block.frames, block.transport, block.reset)
                    && block.bus_count == shared.info.audio_buses.outputs.len()
                    && (block.event_count == 0
                        || (shared.midi_output && shared.info.note_output)) =>
            {
                shared.diagnostics.replied(elapsed, block.processing_micros);
                if block.restart_required {
                    shared.restart_required.store(true, Ordering::Release);
                }
                next_sequence = block.sequence + 1;
                shared.recycle(block, &shared.completed);
            }
            Ok(_) => {
                shared.stop(Status::InvalidResponse);
                break;
            }
            Err(error) => {
                shared.stop(match error.kind() {
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Status::TimedOut,
                    io::ErrorKind::InvalidData => Status::InvalidResponse,
                    io::ErrorKind::Other => Status::PluginFault,
                    _ => Status::Crashed,
                });
                break;
            }
        }
    }
    // ChildGuard reaps even if a plugin is stuck in process(), shutdown or a destructor.
}

pub(super) struct TimedSocket {
    pub(super) stream: UnixStream,
    pub(super) deadline: Instant,
}
impl TimedSocket {
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|v| !v.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "VST3 stream deadline exceeded"))
    }

    fn read_timeout(&self) -> io::Result<()> {
        let result = self.stream.set_read_timeout(Some(self.remaining()?));
        // Darwin returns EINVAL for SO_RCVTIMEO after the peer closes, even with a final
        // response still buffered. A confirmed hangup makes reading that response nonblocking.
        #[cfg(target_os = "macos")]
        if result
            .as_ref()
            .is_err_and(|e| e.raw_os_error() == Some(libc::EINVAL))
        {
            use std::os::fd::AsRawFd;
            let mut fd = libc::pollfd {
                fd: self.stream.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: one initialized pollfd, live owned socket, zero timeout on the IO thread.
            if unsafe { libc::poll(&mut fd, 1, 0) } == 1 && fd.revents & libc::POLLHUP != 0 {
                return Ok(());
            }
        }
        result
    }
}
impl Read for TimedSocket {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_timeout()?;
        let count = self.stream.read(buffer)?;
        // A complete final syscall can still resume after the absolute deadline under load.
        self.remaining()?;
        Ok(count)
    }
}
impl Write for TimedSocket {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        let count = self.stream.write(buffer)?;
        self.remaining()?;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_peer_keeps_its_final_response_readable_under_the_original_deadline() {
        let (parent, mut peer) = UnixStream::pair().unwrap();
        codec::write_json(&mut peer, &serde_json::json!({"error":"rejected"})).unwrap();
        drop(peer);
        let mut socket = TimedSocket {
            stream: parent,
            deadline: Instant::now() + Duration::from_secs(1),
        };
        assert_eq!(
            codec::read_json(&mut socket).unwrap(),
            serde_json::json!({"error":"rejected"})
        );
        let mut byte = [0; 1];
        assert_eq!(socket.read(&mut byte).unwrap(), 0);
        socket.deadline = Instant::now() - Duration::from_millis(1);
        assert_eq!(
            socket.read(&mut byte).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
}
