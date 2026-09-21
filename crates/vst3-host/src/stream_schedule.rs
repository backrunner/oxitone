//! Fixed sample latency over an asynchronous port. Prepare and drop on the control thread.
//! Only process/invalidate/accessors are realtime operations. A fault requires a fresh session.
use super::{RealtimePort, Status};
use crate::{schedule_wire::Schedule, stream_wire::Block, wire::Event};

#[path = "stream_schedule_process.rs"]
mod processing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    RestartRequired,
    Invalidated,
    Discontinuity,
    InvalidBlock,
    EventOverflow,
    FrameOverflow,
    DeadlineMissed,
    InvalidResponse,
    QueueFull,
    Stream(Status),
}

/// Frame is relative to this epoch, starting at zero. Events are relative to this call.
/// These fields do not set the plugin's absolute project position.
pub struct Input<'a> {
    pub epoch: u64,
    pub frame: u64,
    pub left: &'a [f32],
    pub right: &'a [f32],
    pub events: &'a [Event],
    pub payload: &'a [u8],
}

struct Output {
    sequence: Option<u64>,
    audio: Box<[f32]>,
}

pub struct ScheduledPort {
    port: RealtimePort,
    config: Schedule,
    delay_frames: u32,
    total_latency: u32,
    frame: u64,
    next_completion: u64,
    fault: Option<Fault>,
    input: Block,
    output: Box<[Output]>,
    receive_left: Box<[f32]>,
    receive_right: Box<[f32]>,
}
impl ScheduledPort {
    /// Control thread only, taking a fresh, unused port. Latency includes packet assembly:
    /// at least two full plugin blocks, plus the processor's reported intrinsic latency.
    /// Queue capacity must be at least latency_blocks; it does not select the latency.
    pub fn prepare(port: RealtimePort, config: Schedule) -> crate::Result<Self> {
        config.validate()?;
        if port.shared.midi_output {
            return Err(crate::unsupported("ScheduledPort transports PCM only; MIDI capture requires RealtimePort or the isolated project graph"));
        }
        if port.info().audio_buses.input_count() != 1 || port.info().audio_buses.outputs.len() != 1
        {
            return Err(crate::unsupported("ScheduledPort requires a single bus; use RealtimePort bus operations for multibus audio"));
        }
        if port.status() != Status::Running || port.next_sequence != 0 {
            return Err(crate::invalid(
                "VST3 scheduling requires a fresh running port",
            ));
        }
        if config.latency_blocks > port.shared.free.capacity() {
            return Err(crate::invalid(
                "VST3 scheduling latency exceeds queue capacity",
            ));
        }
        let frames = port.info().block_size;
        let delay_frames = (frames * config.latency_blocks) as u32;
        let total_latency = delay_frames
            .checked_add(port.info().latency_frames)
            .ok_or_else(|| crate::invalid("VST3 total latency overflow"))?;
        Ok(Self {
            port,
            config,
            delay_frames,
            total_latency,
            frame: 0,
            next_completion: 0,
            fault: None,
            input: Block::new(frames),
            output: (0..=config.latency_blocks)
                .map(|_| Output {
                    sequence: None,
                    audio: vec![0.; frames * 2].into_boxed_slice(),
                })
                .collect(),
            receive_left: vec![0.; frames].into_boxed_slice(),
            receive_right: vec![0.; frames].into_boxed_slice(),
        })
    }
    pub fn latency_frames(&self) -> u32 {
        self.total_latency
    }
    pub fn buffering_latency_frames(&self) -> u32 {
        self.delay_frames
    }
    pub fn epoch(&self) -> u64 {
        self.config.epoch
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
    pub fn fault(&self) -> Option<Fault> {
        self.fault
    }
    pub fn stream_status(&self) -> Status {
        self.port.status()
    }
    /// Realtime-safe terminal invalidation for seek/loop/graph replacement. It neither frees
    /// buffers nor joins the worker. Retire this entire object and Session on the control thread.
    pub fn invalidate(&mut self) {
        self.fail(Fault::Invalidated);
    }
    /// Bounded, nonblocking processing of 1..=block_size frames. Startup emits silence for
    /// buffering_latency_frames. Every completed block is used only at its assigned sample frame.
    /// Missing its deadline latches a fault and silences this entire call and all later calls.
    /// Output lengths must equal input lengths; malformed buffers clear at most block_size samples.
    pub fn process(
        &mut self,
        input: Input<'_>,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), Fault> {
        self.silence(left, right);
        if let Some(fault) = self.fault {
            return Err(fault);
        }
        let result = self.process_inner(input, left, right);
        if let Err(fault) = result {
            self.silence(left, right);
            self.fail(fault);
        }
        result
    }
    /// Process a complete aligned plugin block with an exact first-sample project context.
    /// Partial assembly cannot accept a new context; use process() for epoch-relative segments.
    pub fn process_at(
        &mut self,
        input: Input<'_>,
        transport: crate::transport_wire::Transport,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), Fault> {
        if let Some(fault) = self.fault {
            self.silence(left, right);
            return Err(fault);
        }
        if self.input.frames != 0
            || input.left.len() != self.port.info().block_size
            || !transport.valid()
        {
            self.silence(left, right);
            self.fail(Fault::InvalidBlock);
            return Err(Fault::InvalidBlock);
        }
        self.input.transport = Some(transport);
        self.process(input, left, right)
    }
    fn silence(&self, left: &mut [f32], right: &mut [f32]) {
        for value in left
            .iter_mut()
            .take(self.port.info().block_size)
            .chain(right.iter_mut().take(self.port.info().block_size))
        {
            *value = 0.;
        }
    }
    fn fail(&mut self, fault: Fault) {
        self.fault.get_or_insert(fault);
        if fault == Fault::RestartRequired {
            return;
        }
        self.port.shared.stop(match fault {
            Fault::DeadlineMissed => Status::DeadlineMissed,
            Fault::Invalidated | Fault::Discontinuity => Status::Invalidated,
            _ => Status::ScheduleFault,
        });
    }
}

#[cfg(test)]
#[path = "stream_schedule_fault_tests.rs"]
mod fault_tests;
#[cfg(test)]
#[path = "sysex_schedule_tests.rs"]
mod sysex_tests;
#[cfg(test)]
#[path = "stream_schedule_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "stream_schedule_transport_tests.rs"]
mod transport_tests;
