//! The render-ahead worker (03-audio-runtime-spec.md §线程模型与平滑播放):
//! a dedicated realtime thread renders `RenderGraph` blocks at the project
//! rate, optionally resamples to the device rate (`deviceRatePolicy:
//! 'resample'` or an `adapt-device` fallback), converts to the device
//! channel layout, and pushes whole frames into the SPSC ring. It shares
//! no locks with the control thread — commands arrive over a bounded SPSC
//! queue, diagnostics leave over another, and the worker is woken with
//! `Thread::unpark`.
//!
//! Timing instrumentation (`Instant::now`, block-time histogram,
//! `engineLoad` EMA) lives here and never in the HAL callback.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use oxitone_core::wire::TransportCommandKind;
use oxitone_dsp::resample::SincResampler;
use oxitone_io_macos::Pull;

use super::diagnostics::{event_codes, DiagnosticEvent, EventProducer, RtCounters, Severity};
use super::layout::stereo_to_device;
use super::ring::{SpscQueue, SpscRing};
use crate::graph::RenderGraph;
use crate::params::QueuedParameterEvent;
use crate::transport::TransportState;

/// Transport commands applied by the render thread; audible at the ring
/// horizon (03 §线程模型: 命令在 ring horizon 生效).
#[derive(Debug)]
pub enum TransportCmd {
    Play {
        from: Option<u64>,
        loop_region: Option<(u64, u64)>,
    },
    Pause,
    Stop,
    Seek {
        frame: u64,
    },
}

impl TransportCmd {
    pub fn kind(&self) -> TransportCommandKind {
        match self {
            TransportCmd::Play { .. } => TransportCommandKind::Play,
            TransportCmd::Pause => TransportCommandKind::Pause,
            TransportCmd::Stop => TransportCommandKind::Stop,
            TransportCmd::Seek { .. } => TransportCommandKind::Seek,
        }
    }
}

/// Messages from the control thread to the render thread. `Box` payloads
/// are allocated on the control side; the queue itself is preallocated.
pub enum WorkerMsg {
    Transport(TransportCmd),
    Param(QueuedParameterEvent),
    ReplaceGraph(Box<RenderGraph>),
    /// New device chain after a hot-swap rebuild (buffered mode).
    Reconfigure(Box<Reconfig>),
    Shutdown,
}

/// Deterministic worker-preemption injection for tests and the jitter
/// benchmark (05-performance-and-benchmarks.md: 注入人工调度抖动模拟抢占).
/// Never set in production sessions.
#[derive(Debug, Clone, Copy)]
pub struct JitterConfig {
    /// Maximum extra stall as a multiple of the block deadline.
    pub max_extra_periods: f64,
    /// Probability a block gets stalled (0..1).
    pub probability: f64,
    pub seed: u64,
}

/// Buffered-mode device-chain replacement, fully allocated on the
/// control/monitor thread.
pub struct Reconfig {
    pub ring: Arc<SpscRing>,
    pub resampler: Option<Box<ResamplerPair>>,
    pub dev_block_frames: usize,
    pub channels: usize,
    pub convert_buf: Vec<f32>,
}

/// Control-thread mirror of the render-thread transport, sampled for
/// responses and diagnostics (written by the render thread only).
#[derive(Default)]
pub struct TransportMirror {
    state: AtomicU8,
    cursor: AtomicU64,
}

impl TransportMirror {
    pub(crate) fn store(&self, state: TransportState, cursor: u64) {
        let code = match state {
            TransportState::Stopped => 0,
            TransportState::Playing => 1,
            TransportState::Paused => 2,
            TransportState::Rendering => 3,
        };
        self.state.store(code, Ordering::Relaxed);
        self.cursor.store(cursor, Ordering::Relaxed);
    }

    pub fn load(&self) -> (TransportState, u64) {
        let state = match self.state.load(Ordering::Relaxed) {
            1 => TransportState::Playing,
            2 => TransportState::Paused,
            3 => TransportState::Rendering,
            _ => TransportState::Stopped,
        };
        (state, self.cursor.load(Ordering::Relaxed))
    }
}

/// Apply a transport command to the graph and mirror the result. Shared
/// by the buffered worker and the direct-mode callback.
pub(crate) fn apply_transport(
    graph: &mut RenderGraph,
    cmd: &TransportCmd,
    mirror: &TransportMirror,
) {
    match cmd {
        TransportCmd::Play { from, loop_region } => {
            if let Some(frame) = from {
                graph.seek(*frame);
            }
            let cursor = graph.transport().cursor;
            graph.transport_mut().play_from(cursor, *loop_region);
        }
        TransportCmd::Pause => graph.transport_mut().pause(),
        TransportCmd::Stop => {
            graph.transport_mut().stop();
            graph.seek(0);
        }
        TransportCmd::Seek { frame } => graph.seek(*frame),
    }
    mirror.store(graph.state(), graph.transport().cursor);
}

/// Stereo polyphase resampler pair for the device-rate policy (worker
/// side; ≥100 dB SNR per 03 §数值精度 via the shared `SincResampler`).
pub struct ResamplerPair {
    left: SincResampler,
    right: SincResampler,
    /// Project frames per device frame (> 1 when downsampling).
    step: f64,
    out_left: Vec<f32>,
    out_right: Vec<f32>,
}

impl ResamplerPair {
    /// `dev_block_frames` bounds one block's device-rate output.
    pub fn new(
        project_rate: f64,
        device_rate: f64,
        block_size: usize,
        dev_block_frames: usize,
    ) -> Self {
        let step = project_rate / device_rate;
        Self {
            left: SincResampler::new(step.max(1.0), block_size),
            right: SincResampler::new(step.max(1.0), block_size),
            step,
            out_left: vec![0.0; dev_block_frames + 8],
            out_right: vec![0.0; dev_block_frames + 8],
        }
    }

    /// Group delay in project-rate frames (counted into the latency
    /// report, 03 §低延迟和设备).
    pub fn group_delay_project_frames(&self) -> f64 {
        self.left.latency_input_frames(self.step)
    }

    /// Push one project-rate block; returns the produced interleaved
    /// device-rate frame count after layout conversion into `out`.
    pub(crate) fn process_block(
        &mut self,
        left: &[f32],
        right: &[f32],
        out: &mut [f32],
        channels: usize,
    ) -> usize {
        let produced_left = self.left.process(left, &mut self.out_left, self.step);
        let produced_right = self.right.process(right, &mut self.out_right, self.step);
        let frames = produced_left.produced.min(produced_right.produced);
        stereo_to_device(
            &self.out_left[..frames],
            &self.out_right[..frames],
            &mut out[..frames * channels],
            channels,
        );
        frames
    }
}

/// Everything the buffered worker needs; assembled on the control thread.
pub(crate) struct WorkerCore {
    graph: Option<Box<RenderGraph>>,
    ring: Arc<SpscRing>,
    resampler: Option<Box<ResamplerPair>>,
    commands: Arc<SpscQueue<WorkerMsg>>,
    events: Arc<SpscQueue<DiagnosticEvent>>,
    counters: Arc<RtCounters>,
    mirror: Arc<TransportMirror>,
    return_slot: ReturnSlot,
    channels: usize,
    dev_block_frames: usize,
    block_size: usize,
    deadline: Duration,
    block_left: Vec<f32>,
    block_right: Vec<f32>,
    convert_buf: Vec<f32>,
    resample_buf: Vec<f32>,
    fault_latched: bool,
    load_ema: f64,
    jitter: Option<JitterConfig>,
}

impl WorkerCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        graph: Box<RenderGraph>,
        ring: Arc<SpscRing>,
        resampler: Option<Box<ResamplerPair>>,
        commands: Arc<SpscQueue<WorkerMsg>>,
        events: Arc<SpscQueue<DiagnosticEvent>>,
        counters: Arc<RtCounters>,
        mirror: Arc<TransportMirror>,
        return_slot: ReturnSlot,
        channels: usize,
        dev_block_frames: usize,
    ) -> Self {
        let block_size = graph.block_size();
        let deadline = Duration::from_secs_f64(block_size as f64 / graph.sample_rate());
        Self {
            graph: Some(graph),
            ring,
            resampler,
            commands,
            events,
            counters,
            mirror,
            return_slot,
            channels,
            dev_block_frames,
            block_size,
            deadline,
            block_left: vec![0.0; block_size],
            block_right: vec![0.0; block_size],
            convert_buf: vec![0.0; dev_block_frames * channels],
            resample_buf: vec![0.0; (dev_block_frames + 8) * channels],
            fault_latched: false,
            load_ema: 0.0,
            jitter: None,
        }
    }

    /// Test/benchmark hook: inject worker-side preemption.
    pub(crate) fn set_jitter(&mut self, jitter: Option<JitterConfig>) {
        self.jitter = jitter;
    }

    /// Returns true when the worker should exit.
    fn handle(&mut self, msg: WorkerMsg) -> bool {
        match msg {
            WorkerMsg::Shutdown => return true,
            WorkerMsg::Transport(cmd) => {
                if let Some(graph) = self.graph.as_mut() {
                    apply_transport(graph, &cmd, &self.mirror);
                }
            }
            WorkerMsg::Param(event) => {
                if let Some(graph) = self.graph.as_mut() {
                    graph.insert_queued_parameter(event);
                }
            }
            WorkerMsg::ReplaceGraph(mut graph) => {
                if let Some(old) = self.graph.as_ref() {
                    graph.transport.state = old.transport.state;
                    graph.transport.cursor = old.transport.cursor;
                    graph.transport.loop_region = old.transport.loop_region;
                }
                self.block_size = graph.block_size();
                self.block_left.resize(self.block_size, 0.0);
                self.block_right.resize(self.block_size, 0.0);
                self.deadline =
                    Duration::from_secs_f64(self.block_size as f64 / graph.sample_rate());
                self.fault_latched = false;
                let state = graph.state();
                let cursor = graph.transport().cursor;
                self.graph = Some(graph);
                self.mirror.store(state, cursor);
            }
            WorkerMsg::Reconfigure(reconfig) => {
                let Reconfig {
                    ring,
                    resampler,
                    dev_block_frames,
                    channels,
                    convert_buf,
                } = *reconfig;
                self.ring = ring;
                self.resampler = resampler;
                self.dev_block_frames = dev_block_frames;
                self.channels = channels;
                self.convert_buf = convert_buf;
                self.resample_buf = vec![0.0; (dev_block_frames + 8) * channels];
            }
        }
        false
    }

    pub(crate) fn render_block(&mut self) {
        let events = self.events.clone();
        let counters = self.counters.clone();
        let push_event = |event: DiagnosticEvent| {
            EventProducer {
                queue: &events,
                drops: &counters.queue_drops,
            }
            .push(event);
        };
        let Some(graph) = self.graph.as_mut() else {
            return;
        };
        let started = Instant::now();
        graph.process_block(&mut self.block_left, &mut self.block_right);
        counters.blocks.fetch_add(1, Ordering::Relaxed);

        if graph.faulted() && !self.fault_latched {
            self.fault_latched = true;
            counters.nan_blocks.fetch_add(1, Ordering::Relaxed);
            push_event(DiagnosticEvent::new(
                event_codes::REALTIME_FAULT,
                Severity::Error,
                graph.transport().cursor,
                "non-finite samples detected; block muted and transport stopped",
            ));
            graph.transport_mut().stop();
        }

        let frames = match self.resampler.as_mut() {
            Some(resampler) => resampler.process_block(
                &self.block_left[..self.block_size],
                &self.block_right[..self.block_size],
                &mut self.resample_buf,
                self.channels,
            ),
            None => {
                stereo_to_device(
                    &self.block_left[..self.block_size],
                    &self.block_right[..self.block_size],
                    &mut self.convert_buf[..self.dev_block_frames * self.channels],
                    self.channels,
                );
                self.dev_block_frames
            }
        };
        let written = match self.resampler.as_ref() {
            Some(_) => self
                .ring
                .write_frames(&self.resample_buf[..frames * self.channels]),
            None => self
                .ring
                .write_frames(&self.convert_buf[..frames * self.channels]),
        };
        debug_assert_eq!(written, frames, "caller checked ring headroom");
        counters
            .ring_occupancy_frames
            .store(self.ring.available_to_read_frames(), Ordering::Relaxed);
        if let Some(hottest) = graph.busiest_channel() {
            counters.hottest_channel.store(hottest, Ordering::Relaxed);
        }
        self.mirror.store(graph.state(), graph.transport().cursor);

        let nanos = started.elapsed().as_nanos() as u64;
        counters.record_block_time(nanos);
        let deadline_nanos = self.deadline.as_nanos() as u64;
        let load = nanos as f64 / deadline_nanos.max(1) as f64;
        self.load_ema = if self.load_ema == 0.0 {
            load
        } else {
            0.9 * self.load_ema + 0.1 * load
        };
        counters.store_engine_load(self.load_ema);
        if nanos > deadline_nanos {
            counters.deadline_misses.fetch_add(1, Ordering::Relaxed);
            let consecutive = counters.consecutive_misses.fetch_add(1, Ordering::Relaxed) + 1;
            if consecutive >= 3 {
                counters
                    .performance_warnings
                    .fetch_add(1, Ordering::Relaxed);
                counters.consecutive_misses.store(0, Ordering::Relaxed);
                push_event(DiagnosticEvent::new(
                    event_codes::PERFORMANCE_WARNING,
                    Severity::Warning,
                    graph.transport().cursor,
                    "render worker exceeded its deadline 3 blocks in a row; underrun is likely",
                ));
            }
        } else {
            counters.consecutive_misses.store(0, Ordering::Relaxed);
        }
    }

    /// Worker thread body. Sets FTZ/DAZ and a time-constraint scheduling
    /// policy before entering the loop (03 §CPU 尖峰防线, §线程模型).
    pub(crate) fn run(mut self) {
        oxitone_dsp::ftz::set_ftz_daz(true);
        #[cfg(target_os = "macos")]
        {
            let period = self.deadline;
            let _ = oxitone_io_macos::thread::set_time_constraint(period, period / 2, period);
        }
        let mut jitter_state = self.jitter.map(|j| j.seed.max(1));
        let mut stalled_last = false;
        loop {
            while let Some(msg) = self.commands.pop() {
                if self.handle(msg) {
                    self.park_graph();
                    return;
                }
            }
            // The resampler can overshoot one block's nominal frame count
            // by a few frames; require the slack up front so the ring
            // write never truncates a block.
            if self.graph.is_some()
                && self.ring.available_to_write_frames() >= self.dev_block_frames + 8
            {
                self.render_block();
                // Test-only preemption injection (05-performance-and-
                // benchmarks.md: 注入人工调度抖动模拟抢占). Sleeps on the
                // worker, exactly like an OS scheduling stall would.
                // Stalls are capped at one consecutive block — they model
                // the isolated scheduler jitter the render-ahead ring is
                // specified to absorb (03 §线程模型), not sustained
                // overload, which the underrun test covers with long
                // single stalls instead.
                if let (Some(jitter), Some(state)) = (self.jitter, jitter_state.as_mut()) {
                    *state ^= *state >> 12;
                    *state ^= *state << 25;
                    *state ^= *state >> 27;
                    let roll = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
                    let unit = (roll >> 11) as f64 / (1u64 << 53) as f64;
                    if !stalled_last && unit < jitter.probability {
                        stalled_last = true;
                        let extra = self
                            .deadline
                            .mul_f64(jitter.max_extra_periods * (unit / jitter.probability));
                        std::thread::sleep(extra);
                    } else {
                        stalled_last = false;
                    }
                }
            } else {
                std::thread::park_timeout(self.deadline / 4);
            }
        }
    }

    /// Hand the graph back to the control thread on shutdown (the stream
    /// may have failed to start; the engine keeps the graph).
    fn park_graph(&mut self) {
        if let Ok(mut slot) = self.return_slot.lock() {
            *slot = self.graph.take();
        }
    }
}

/// Buffered-mode pull closure: pure ring → output copy with zero-fill on
/// underrun (03 §线程模型: callback 输出静音、xruns++、transport 不停止).
pub(crate) fn make_buffered_pull(
    ring: Arc<SpscRing>,
    counters: Arc<RtCounters>,
    events: Arc<SpscQueue<DiagnosticEvent>>,
    mirror: Arc<TransportMirror>,
) -> Pull {
    Box::new(move |out: &mut [f32]| {
        let channels = ring.channels();
        let frames = out.len() / channels;
        let got = ring.read_frames(out);
        if got < frames {
            out[got * channels..].fill(0.0);
            counters.xruns.fetch_add(1, Ordering::Relaxed);
            let (_, cursor) = mirror.load();
            EventProducer {
                queue: &events,
                drops: &counters.queue_drops,
            }
            .push(DiagnosticEvent::new(
                event_codes::UNDERRUN,
                Severity::Error,
                cursor,
                "ring underrun; slice muted, transport continues",
            ));
        }
    })
}

/// Graph return channel for the direct-mode rebuild and worker shutdown
/// (locked on control threads only; the callback never touches it).
pub(crate) type GraphReturnSlot = Arc<Mutex<Option<Box<RenderGraph>>>>;
pub(crate) type ReturnSlot = GraphReturnSlot;
