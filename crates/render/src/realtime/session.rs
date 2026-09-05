//! Realtime playback session (03-audio-runtime-spec.md §低延迟和设备,
//! §线程模型与平滑播放). Owns the render worker (buffered mode) or the
//! direct-render callback, the SPSC ring, the output sink, and the device
//! monitor thread. All rebuilds happen on the monitor thread; the control
//! thread only enqueues commands.
//!
//! Threading map:
//!
//! ```text
//! control (N-API)            worker (RT)              HAL callback (RT)
//!  transport/setParameter ──► SpscQueue ──► RenderGraph::process_block
//!  compile ─────────────────► ReplaceGraph        stereo→device layout
//!  getDiagnostics ◄── events ◄── counters          (resample when needed)
//!                                                    │ SpscRing (frames)
//!                                                    ▼
//!                                              ring→output copy,
//!                                              underrun → silence+xruns
//! monitor thread: device events (mpsc) → stop stream → renegotiate →
//!   Reconfigure worker / rebuild direct core → start stream
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{DeviceChangePolicy, DeviceRatePolicy, LatencyMode};
use oxitone_io_macos::{
    device_for_uid, DeviceEvent, PreparedStream, Pull, StreamInfo, StreamRequest,
};
use oxitone_transport::tempo::CompiledTempoMap;

use super::diagnostics::{event_codes, DiagnosticEvent, DiagnosticsSnapshot, RtCounters, Severity};
use super::direct::DirectCore;
use super::ring::{SpscQueue, SpscRing};
use super::sink::{RunningSink, SimulatedSink, SimulatedSinkConfig};
use super::worker::{
    make_buffered_pull, JitterConfig, Reconfig, ResamplerPair, ReturnSlot, TransportCmd,
    TransportMirror, WorkerCore, WorkerMsg,
};
use crate::graph::RenderGraph;
use crate::transport::TransportState;

const COMMAND_QUEUE_CAPACITY: usize = 256;
const EVENT_QUEUE_CAPACITY: usize = 256;

/// Engine options that shape the realtime chain (04-api-contracts.md
/// `EngineOptions`; defaults applied by the caller).
#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    pub render_ahead_blocks: u32,
    pub latency_mode: LatencyMode,
    pub device_rate_policy: DeviceRatePolicy,
    pub device_change_policy: DeviceChangePolicy,
    pub output_device_id: Option<String>,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            render_ahead_blocks: 4,
            latency_mode: LatencyMode::Buffered,
            device_rate_policy: DeviceRatePolicy::AdaptDevice,
            device_change_policy: DeviceChangePolicy::FollowDefault,
            output_device_id: None,
        }
    }
}

/// `getOutputLatency` breakdown (04-api-contracts.md `OutputLatency`).
/// Frames are reported at the project sample rate.
#[derive(Debug, Clone, Copy)]
pub struct OutputLatencyReport {
    pub frames: u64,
    pub seconds: f64,
    pub ring: u64,
    pub resampler: u64,
    pub device_buffer: u64,
    pub safety_offset: u64,
    pub device_latency: u64,
}

/// Failed session start: returns the graph so the engine keeps it.
pub struct SessionStartError {
    pub error: OxitoneError,
    pub graph: Box<RenderGraph>,
}

impl SessionStartError {
    fn new(error: OxitoneError, graph: Box<RenderGraph>) -> Self {
        Self { error, graph }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Buffered,
    Direct,
}

/// A device prepared for streaming (real HAL or simulated).
enum Prepared {
    CoreAudio(PreparedStream),
    Simulated(SimulatedSinkConfig),
}

impl Prepared {
    fn info(&self) -> StreamInfo {
        match self {
            Prepared::CoreAudio(stream) => stream.info().clone(),
            Prepared::Simulated(config) => StreamInfo {
                device: 0,
                sample_rate: config.sample_rate,
                frames_per_slice: config.frames_per_slice,
                channels: config.channels,
                device_latency_frames: config.latency_frames,
                safety_offset_frames: config.safety_offset_frames,
                rate_adapted: false,
                diagnostics: Vec::new(),
            },
        }
    }

    fn start(self, pull: Pull) -> Result<Box<dyn RunningSink>, OxitoneError> {
        match self {
            Prepared::CoreAudio(stream) => {
                let stream = stream.start(pull)?;
                Ok(Box::new(stream))
            }
            Prepared::Simulated(config) => Ok(Box::new(SimulatedSink::start(config, pull)?)),
        }
    }
}

/// Everything shared between the session, worker, callback and monitor.
struct Shared {
    commands: Arc<SpscQueue<WorkerMsg>>,
    worker_events: Arc<SpscQueue<DiagnosticEvent>>,
    callback_events: Arc<SpscQueue<DiagnosticEvent>>,
    counters: Arc<RtCounters>,
    mirror: Arc<TransportMirror>,
    sink: Mutex<Option<Box<dyn RunningSink>>>,
    mode: Mutex<Mode>,
    worker: Mutex<Option<JoinHandle<()>>>,
    return_slot: ReturnSlot,
    /// Ring depth in device-rate frames (0 in direct mode).
    ring_depth_frames: AtomicUsize,
    /// Resampler group delay in project-rate frames (f64 bits).
    resampler_delay_bits: AtomicU64,
    monitor_stop: AtomicBool,
    pending_events: Mutex<Vec<DiagnosticEvent>>,
}

impl Shared {
    fn push_pending(&self, event: DiagnosticEvent) {
        if let Ok(mut pending) = self.pending_events.lock() {
            pending.push(event);
        }
    }

    fn unpark_worker(&self) {
        if let Ok(guard) = self.worker.lock() {
            if let Some(join) = guard.as_ref() {
                join.thread().unpark();
            }
        }
    }

    fn send(&self, msg: WorkerMsg) {
        if self.commands.push(msg).is_err() {
            self.counters.queue_drops.fetch_add(1, Ordering::Relaxed);
        }
        self.unpark_worker();
    }
}

/// Live realtime playback session. Dropping stops the device stream and
/// joins all threads.
pub struct RealtimeSession {
    shared: Arc<Shared>,
    project_sample_rate: f64,
    tempo: Mutex<CompiledTempoMap>,
    channel_ids: Vec<String>,
    /// Kept alive so device listeners can always send.
    _device_event_tx: Sender<DeviceEvent>,
    monitor: Option<JoinHandle<()>>,
    predicted: Mutex<(TransportState, u64)>,
}

fn device_unavailable(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::DEVICE_UNAVAILABLE, message)
}

/// Frames of one project block at the device rate.
fn dev_block_frames(block_size: usize, project_rate: f64, device_rate: f64) -> usize {
    (block_size as f64 * device_rate / project_rate).ceil() as usize
}

/// Ring depth in device frames (03 §低延迟和设备):
/// `max(renderAheadBlocks, ceil((deviceBuffer + deviceLatency) / blockSize))`.
fn ring_depth_frames(render_ahead_blocks: u32, dev_block: usize, info: &StreamInfo) -> usize {
    let ahead = render_ahead_blocks as usize * dev_block;
    ahead.max(info.frames_per_slice as usize + info.device_latency_frames as usize)
}

/// Build the ring and optional resampler for a stream config (control
/// thread allocation).
fn build_chain(
    info: &StreamInfo,
    config: &RealtimeConfig,
    project_sample_rate: f64,
    block_size: usize,
) -> (Arc<SpscRing>, Option<Box<ResamplerPair>>, usize) {
    let dev_block = dev_block_frames(block_size, project_sample_rate, info.sample_rate);
    let resample = (info.sample_rate - project_sample_rate).abs() > 0.5;
    let depth = ring_depth_frames(config.render_ahead_blocks, dev_block, info);
    let resampler = resample.then(|| {
        Box::new(ResamplerPair::new(
            project_sample_rate,
            info.sample_rate,
            block_size,
            dev_block,
        ))
    });
    (
        Arc::new(SpscRing::new(depth, info.channels as usize)),
        resampler,
        dev_block,
    )
}

/// Wait (bounded) until the worker has primed the ring, so the sink's
/// first callbacks don't underrun on startup silence. Runs on control
/// threads only (session start / monitor rebuild).
fn prime_ring(ring: &Arc<SpscRing>, target_frames: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while ring.available_to_read_frames() < target_frames {
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Spawn the buffered-mode worker. The graph comes back through
/// `shared.return_slot` on shutdown.
#[allow(clippy::too_many_arguments)]
fn spawn_worker(
    shared: &Arc<Shared>,
    graph: Box<RenderGraph>,
    ring: Arc<SpscRing>,
    resampler: Option<Box<ResamplerPair>>,
    dev_block: usize,
    channels: usize,
    jitter: Option<JitterConfig>,
) {
    shared.resampler_delay_bits.store(
        resampler
            .as_ref()
            .map(|r| r.group_delay_project_frames())
            .unwrap_or(0.0)
            .to_bits(),
        Ordering::Relaxed,
    );
    let mut core = WorkerCore::new(
        graph,
        ring,
        resampler,
        shared.commands.clone(),
        shared.worker_events.clone(),
        shared.counters.clone(),
        shared.mirror.clone(),
        shared.return_slot.clone(),
        channels,
        dev_block,
    );
    core.set_jitter(jitter);
    let join = std::thread::Builder::new()
        .name("oxitone-render-worker".into())
        .spawn(move || core.run())
        .expect("failed to spawn render worker");
    *shared.worker.lock().expect("worker mutex") = Some(join);
}

/// Stop the worker (if any) and recover the graph it parked.
fn stop_worker_and_recover(shared: &Arc<Shared>) -> Option<Box<RenderGraph>> {
    shared.send(WorkerMsg::Shutdown);
    let join = shared.worker.lock().expect("worker mutex").take();
    if let Some(join) = join {
        let _ = join.join();
    }
    shared.return_slot.lock().expect("return slot").take()
}

impl RealtimeSession {
    /// Start a session on a real CoreAudio output device.
    pub fn start(
        graph: Box<RenderGraph>,
        config: RealtimeConfig,
    ) -> Result<Self, SessionStartError> {
        Self::start_inner(graph, config, None, None)
    }

    /// Start a session on the simulated sink (tests and the soak harness;
    /// macOS has no null output device). `worker_jitter` injects
    /// deterministic worker-side preemption for the jitter benchmark.
    pub fn start_simulated(
        graph: Box<RenderGraph>,
        config: RealtimeConfig,
        sink: SimulatedSinkConfig,
        worker_jitter: Option<JitterConfig>,
    ) -> Result<Self, SessionStartError> {
        Self::start_inner(graph, config, Some(sink), worker_jitter)
    }

    fn start_inner(
        graph: Box<RenderGraph>,
        config: RealtimeConfig,
        simulated: Option<SimulatedSinkConfig>,
        worker_jitter: Option<JitterConfig>,
    ) -> Result<Self, SessionStartError> {
        let project_sample_rate = graph.sample_rate();
        let block_size = graph.block_size();
        let tempo = graph.plan().tempo.clone();
        let channel_ids = graph.channel_ids();
        let (device_event_tx, device_event_rx) = channel();

        let prepared = match &simulated {
            Some(config) => Prepared::Simulated(SimulatedSinkConfig {
                sample_rate: config.sample_rate,
                frames_per_slice: config.frames_per_slice,
                channels: config.channels,
                latency_frames: config.latency_frames,
                safety_offset_frames: config.safety_offset_frames,
            }),
            None => {
                let device = match device_for_uid(config.output_device_id.as_deref()) {
                    Ok(device) => device,
                    Err(error) => return Err(SessionStartError::new(error, graph)),
                };
                let adapt_sample_rate = match config.device_rate_policy {
                    DeviceRatePolicy::AdaptDevice => Some(project_sample_rate),
                    DeviceRatePolicy::Resample => None,
                };
                let request = StreamRequest {
                    device,
                    adapt_sample_rate,
                    target_buffer_frames: block_size as u32,
                    min_buffer_frames: 64,
                    events: device_event_tx.clone(),
                };
                match PreparedStream::prepare(request) {
                    Ok(prepared) => Prepared::CoreAudio(prepared),
                    Err(error) => return Err(SessionStartError::new(error, graph)),
                }
            }
        };
        let info = prepared.info();

        let shared = Arc::new(Shared {
            commands: Arc::new(SpscQueue::new(COMMAND_QUEUE_CAPACITY)),
            worker_events: Arc::new(SpscQueue::new(EVENT_QUEUE_CAPACITY)),
            callback_events: Arc::new(SpscQueue::new(EVENT_QUEUE_CAPACITY)),
            counters: Arc::new(RtCounters::new()),
            mirror: Arc::new(TransportMirror::default()),
            sink: Mutex::new(None),
            mode: Mutex::new(Mode::Buffered),
            worker: Mutex::new(None),
            return_slot: Arc::new(Mutex::new(None)),
            ring_depth_frames: AtomicUsize::new(0),
            resampler_delay_bits: AtomicU64::new(0f64.to_bits()),
            monitor_stop: AtomicBool::new(false),
            pending_events: Mutex::new(Vec::new()),
        });
        for note in &info.diagnostics {
            shared.push_pending(DiagnosticEvent::new(
                event_codes::DEVICE_CHANGE,
                Severity::Warning,
                0,
                note,
            ));
        }

        let resample = (info.sample_rate - project_sample_rate).abs() > 0.5;
        let direct_ok = !resample && info.frames_per_slice as usize == block_size;
        let mode = match config.latency_mode {
            LatencyMode::Direct if direct_ok => Mode::Direct,
            LatencyMode::Direct => {
                shared.push_pending(DiagnosticEvent::new(
                    event_codes::MODE_FALLBACK,
                    Severity::Warning,
                    0,
                    "latencyMode 'direct' requires the device rate to equal the project rate and frames-per-slice == blockSize; using buffered mode",
                ));
                Mode::Buffered
            }
            LatencyMode::Buffered => Mode::Buffered,
        };
        *shared.mode.lock().expect("mode mutex") = mode;

        let mut graph = Some(graph);
        let pull = match mode {
            Mode::Buffered => {
                let (ring, resampler, dev_block) =
                    build_chain(&info, &config, project_sample_rate, block_size);
                shared.ring_depth_frames.store(
                    ring_depth_frames(config.render_ahead_blocks, dev_block, &info),
                    Ordering::Relaxed,
                );
                spawn_worker(
                    &shared,
                    graph.take().expect("graph present"),
                    ring.clone(),
                    resampler,
                    dev_block,
                    info.channels as usize,
                    worker_jitter,
                );
                prime_ring(&ring, ring.capacity_frames() / 2);
                make_buffered_pull(
                    ring,
                    shared.counters.clone(),
                    shared.callback_events.clone(),
                    shared.mirror.clone(),
                )
            }
            Mode::Direct => {
                shared.ring_depth_frames.store(0, Ordering::Relaxed);
                let core = DirectCore::new(
                    graph.take().expect("graph present"),
                    shared.return_slot.clone(),
                    shared.commands.clone(),
                    shared.worker_events.clone(),
                    shared.counters.clone(),
                    shared.mirror.clone(),
                    info.channels as usize,
                );
                core.into_pull()
            }
        };

        let sink = match prepared.start(pull) {
            Ok(sink) => sink,
            Err(error) => {
                // The pull closure (and, in direct mode, its DirectCore)
                // was dropped with the failed start, so the graph is
                // already parked; a buffered worker must be shut down
                // first.
                let graph = stop_worker_and_recover(&shared)
                    .expect("graph recovered after sink start failure");
                return Err(SessionStartError::new(error, graph));
            }
        };
        *shared.sink.lock().expect("sink mutex") = Some(sink);

        let monitor = spawn_monitor(
            shared.clone(),
            config.clone(),
            project_sample_rate,
            block_size,
            device_event_rx,
            device_event_tx.clone(),
            simulated.is_some(),
        );

        Ok(Self {
            shared,
            project_sample_rate,
            tempo: Mutex::new(tempo),
            channel_ids,
            _device_event_tx: device_event_tx,
            monitor: Some(monitor),
            predicted: Mutex::new((TransportState::Stopped, 0)),
        })
    }

    /// Send a transport command; takes effect at the ring horizon. The
    /// returned state is the predicted post-command state.
    pub fn transport(&self, cmd: TransportCmd) -> (TransportState, u64) {
        let mut predicted = self.predicted.lock().expect("predicted mutex");
        let next = match &cmd {
            TransportCmd::Play { from } => (TransportState::Playing, from.unwrap_or(predicted.1)),
            TransportCmd::Pause => (TransportState::Paused, predicted.1),
            TransportCmd::Stop => (TransportState::Stopped, 0),
            TransportCmd::Seek { frame } => (predicted.0, *frame),
        };
        *predicted = next;
        self.shared.send(WorkerMsg::Transport(cmd));
        next
    }

    /// Enqueue a pre-resolved parameter event (resolved against the
    /// session's `ParamTargetIndex` by the caller).
    pub fn enqueue_parameter(&self, event: crate::params::QueuedParameterEvent) {
        self.shared.send(WorkerMsg::Param(event));
    }

    /// Swap the compiled graph at the next block boundary.
    pub fn replace_graph(&self, graph: Box<RenderGraph>) {
        if let Ok(mut tempo) = self.tempo.lock() {
            *tempo = graph.plan().tempo.clone();
        }
        self.shared.send(WorkerMsg::ReplaceGraph(graph));
    }

    /// Current cursor as the render thread reports it.
    pub fn cursor(&self) -> u64 {
        self.shared.mirror.load().1
    }

    /// Transport state as the render thread reports it.
    pub fn transport_state(&self) -> TransportState {
        self.shared.mirror.load().0
    }

    pub fn beat_to_frame(&self, beat: Beat) -> u64 {
        self.tempo.lock().expect("tempo mutex").beat_to_frame(beat)
    }

    /// Output latency breakdown in project-rate frames plus seconds.
    pub fn output_latency(&self) -> Result<OutputLatencyReport, OxitoneError> {
        let sink = self.shared.sink.lock().expect("sink mutex");
        let sink = sink
            .as_ref()
            .ok_or_else(|| device_unavailable("no output device is active"))?;
        let info = sink.info();
        let ratio = self.project_sample_rate / info.sample_rate;
        let scale = |frames: u64| (frames as f64 * ratio).round() as u64;
        let mode = *self.shared.mode.lock().expect("mode mutex");
        let ring = match mode {
            Mode::Direct => 0,
            Mode::Buffered => scale(self.shared.ring_depth_frames.load(Ordering::Relaxed) as u64),
        };
        let resampler =
            f64::from_bits(self.shared.resampler_delay_bits.load(Ordering::Relaxed)).round() as u64;
        let device_buffer = scale(u64::from(info.frames_per_slice));
        let safety_offset = scale(u64::from(info.safety_offset_frames));
        let device_latency = scale(u64::from(info.device_latency_frames));
        let frames = ring + resampler + device_buffer + safety_offset + device_latency;
        Ok(OutputLatencyReport {
            frames,
            seconds: frames as f64 / self.project_sample_rate,
            ring,
            resampler,
            device_buffer,
            safety_offset,
            device_latency,
        })
    }

    /// Counters plus all diagnostic events raised since the last call.
    /// Underrun events are enriched with the busiest channel (best-effort
    /// attribution, 03 §线程模型).
    pub fn snapshot_diagnostics(&self) -> DiagnosticsSnapshot {
        let mut events: Vec<DiagnosticEvent> = self
            .shared
            .pending_events
            .lock()
            .map(|mut pending| pending.drain(..).collect())
            .unwrap_or_default();
        while let Some(event) = self.shared.worker_events.pop() {
            events.push(event);
        }
        while let Some(event) = self.shared.callback_events.pop() {
            events.push(event);
        }
        let hottest = self.shared.counters.hottest_channel.load(Ordering::Relaxed);
        let hottest_name = self
            .channel_ids
            .get(hottest)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let events = events
            .into_iter()
            .map(|event| {
                if event.code != event_codes::UNDERRUN {
                    return event;
                }
                DiagnosticEvent::new(
                    event.code,
                    event.severity,
                    event.frame,
                    &format!(
                        "ring underrun; slice muted, transport continues (busiest channel: {hottest_name})"
                    ),
                )
            })
            .collect();
        self.shared.counters.snapshot(events)
    }
}

fn spawn_monitor(
    shared: Arc<Shared>,
    config: RealtimeConfig,
    project_sample_rate: f64,
    block_size: usize,
    device_rx: Receiver<DeviceEvent>,
    device_tx: Sender<DeviceEvent>,
    simulated: bool,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("oxitone-device-monitor".into())
        .spawn(move || {
            let mut ctx = MonitorCtx {
                shared,
                config,
                project_sample_rate,
                block_size,
                device_rx,
                device_tx,
                simulated,
            };
            ctx.run();
        })
        .expect("failed to spawn device monitor")
}

struct MonitorCtx {
    shared: Arc<Shared>,
    config: RealtimeConfig,
    project_sample_rate: f64,
    block_size: usize,
    device_rx: Receiver<DeviceEvent>,
    device_tx: Sender<DeviceEvent>,
    simulated: bool,
}

impl MonitorCtx {
    fn run(&mut self) {
        while !self.shared.monitor_stop.load(Ordering::Relaxed) {
            match self.device_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => self.handle(event),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(event) = self.device_rx.try_recv() {
                self.handle(event);
            }
        }
    }

    fn handle(&mut self, _event: DeviceEvent) {
        if self.simulated {
            return;
        }
        if self.config.device_change_policy == DeviceChangePolicy::Pause {
            self.shared.push_pending(DiagnosticEvent::new(
                event_codes::DEVICE_CHANGE,
                Severity::Warning,
                self.shared.mirror.load().1,
                "output device changed; transport paused (deviceChangePolicy: 'pause')",
            ));
            self.shared.send(WorkerMsg::Transport(TransportCmd::Pause));
            return;
        }
        self.rebuild();
    }

    /// Rebuild the whole output chain on this control thread (03 §低延迟
    /// 和设备: 所有重建都在控制线程完成，HAL callback 只读 atomic 状态).
    fn rebuild(&mut self) {
        self.shared.push_pending(DiagnosticEvent::new(
            event_codes::DEVICE_CHANGE,
            Severity::Warning,
            self.shared.mirror.load().1,
            "output device changed; rebuilding the output chain",
        ));
        // Stop the old stream (direct mode hands its graph back via Drop).
        let old = self.shared.sink.lock().expect("sink mutex").take();
        drop(old);

        let device = match device_for_uid(self.config.output_device_id.as_deref()) {
            Ok(device) => device,
            Err(error) => {
                self.device_unavailable(&format!(
                    "no output device available after change: {}",
                    error.message
                ));
                return;
            }
        };
        let adapt_sample_rate = match self.config.device_rate_policy {
            DeviceRatePolicy::AdaptDevice => Some(self.project_sample_rate),
            DeviceRatePolicy::Resample => None,
        };
        let prepared = match PreparedStream::prepare(StreamRequest {
            device,
            adapt_sample_rate,
            target_buffer_frames: self.block_size as u32,
            min_buffer_frames: 64,
            events: self.device_tx.clone(),
        }) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.device_unavailable(&format!(
                    "failed to open the new output device: {}",
                    error.message
                ));
                return;
            }
        };
        let info = prepared.info().clone();
        for note in &info.diagnostics {
            self.shared.push_pending(DiagnosticEvent::new(
                event_codes::DEVICE_CHANGE,
                Severity::Warning,
                0,
                note,
            ));
        }

        let mode = *self.shared.mode.lock().expect("mode mutex");
        let pull = match mode {
            Mode::Buffered => {
                let (ring, resampler, dev_block) = build_chain(
                    &info,
                    &self.config,
                    self.project_sample_rate,
                    self.block_size,
                );
                self.shared.ring_depth_frames.store(
                    ring_depth_frames(self.config.render_ahead_blocks, dev_block, &info),
                    Ordering::Relaxed,
                );
                self.shared.resampler_delay_bits.store(
                    resampler
                        .as_ref()
                        .map(|r| r.group_delay_project_frames())
                        .unwrap_or(0.0)
                        .to_bits(),
                    Ordering::Relaxed,
                );
                self.shared.send(WorkerMsg::Reconfigure(Box::new(Reconfig {
                    ring: ring.clone(),
                    resampler,
                    dev_block_frames: dev_block,
                    channels: info.channels as usize,
                    convert_buf: vec![0.0; dev_block * info.channels as usize],
                })));
                prime_ring(&ring, ring.capacity_frames() / 2);
                make_buffered_pull(
                    ring,
                    self.shared.counters.clone(),
                    self.shared.callback_events.clone(),
                    self.shared.mirror.clone(),
                )
            }
            Mode::Direct => {
                let graph = self.shared.return_slot.lock().expect("return slot").take();
                let Some(graph) = graph else {
                    self.device_unavailable(
                        "direct-mode graph was lost during device rebuild; playback paused",
                    );
                    return;
                };
                let resample = (info.sample_rate - self.project_sample_rate).abs() > 0.5;
                if resample || info.frames_per_slice as usize != self.block_size {
                    // Direct is no longer possible on the new device: fall
                    // back to buffered and keep playing (03 §低延迟和设备).
                    self.shared.push_pending(DiagnosticEvent::new(
                        event_codes::MODE_FALLBACK,
                        Severity::Warning,
                        0,
                        "new device is incompatible with direct mode; falling back to buffered",
                    ));
                    *self.shared.mode.lock().expect("mode mutex") = Mode::Buffered;
                    let (ring, resampler, dev_block) = build_chain(
                        &info,
                        &self.config,
                        self.project_sample_rate,
                        self.block_size,
                    );
                    self.shared.ring_depth_frames.store(
                        ring_depth_frames(self.config.render_ahead_blocks, dev_block, &info),
                        Ordering::Relaxed,
                    );
                    spawn_worker(
                        &self.shared,
                        graph,
                        ring.clone(),
                        resampler,
                        dev_block,
                        info.channels as usize,
                        None,
                    );
                    prime_ring(&ring, ring.capacity_frames() / 2);
                    make_buffered_pull(
                        ring,
                        self.shared.counters.clone(),
                        self.shared.callback_events.clone(),
                        self.shared.mirror.clone(),
                    )
                } else {
                    let core = DirectCore::new(
                        graph,
                        self.shared.return_slot.clone(),
                        self.shared.commands.clone(),
                        self.shared.worker_events.clone(),
                        self.shared.counters.clone(),
                        self.shared.mirror.clone(),
                        info.channels as usize,
                    );
                    core.into_pull()
                }
            }
        };

        match prepared.start(pull) {
            Ok(sink) => {
                *self.shared.sink.lock().expect("sink mutex") =
                    Some(Box::new(sink) as Box<dyn RunningSink>);
            }
            Err(error) => {
                self.device_unavailable(&format!(
                    "failed to start the new output device: {}",
                    error.message
                ));
            }
        }
        // Drop listener events raised by our own negotiation.
        while self.device_rx.try_recv().is_ok() {}
    }

    /// No usable device: transport pauses and the error surfaces as a
    /// diagnostic (03 §错误与恢复).
    fn device_unavailable(&mut self, message: &str) {
        self.shared.push_pending(DiagnosticEvent::new(
            event_codes::DEVICE_UNAVAILABLE,
            Severity::Error,
            0,
            message,
        ));
        self.shared.send(WorkerMsg::Transport(TransportCmd::Pause));
    }
}

impl Drop for RealtimeSession {
    fn drop(&mut self) {
        self.shared.monitor_stop.store(true, Ordering::Relaxed);
        if let Some(monitor) = self.monitor.take() {
            let _ = monitor.join();
        }
        self.shared.send(WorkerMsg::Shutdown);
        let worker = self.shared.worker.lock().expect("worker mutex").take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        let sink = self.shared.sink.lock().expect("sink mutex").take();
        drop(sink);
    }
}
