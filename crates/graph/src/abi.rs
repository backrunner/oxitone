//! Plugin ABI v1 as Rust traits (08-plugin-abi.md). Statically linked plugins
//! implement [`Plugin`]; built-in instruments/effects and future dynamically
//! loaded `.dylib` plugins share this exact contract — there is no private
//! built-in path. Dynamic loading uses the same semantics through the C ABI.

use crate::descriptor::PluginDescriptor;

/// Host information handed to `Plugin::create`. Carries the host's current
/// audio configuration at creation time; `prepare` is authoritative and is
/// always called before the first `process`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostContext {
    pub sample_rate: f64,
    pub max_block_size: u32,
}

/// Note event kind; velocity is meaningful for both (off-velocity on `NoteOff`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum NoteEventKind {
    NoteOn,
    NoteOff,
}

/// Sample-accurate note event inside one block.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct NoteEvent {
    /// Frame offset within the block, `< ProcessContext::frames`.
    pub frame_offset: u32,
    pub kind: NoteEventKind,
    pub pitch: u8,
    /// 0..=1 (04-api-contracts.md velocity domain).
    pub velocity: f32,
}

/// Sample-accurate parameter change inside one block. `value` is the physical
/// value in the parameter's declared unit and range; the host has already
/// applied the 0..1 → physical mapping, and the plugin applies its own
/// `ParameterSpec::smoothing` (the ABI does no implicit smoothing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterEvent<'a> {
    /// Frame offset within the block, `< ProcessContext::frames`.
    pub frame_offset: u32,
    /// Stable parameter ID from the descriptor.
    pub parameter_id: &'a str,
    pub value: f64,
}

/// Per-block processing view. All buffers are non-interleaved `f32` with at
/// least `frames` samples per bus; bus counts match the descriptor's
/// `input_layout`/`output_layout`. All buffers are preallocated and owned by
/// the host: the plugin must not retain the slices beyond the `process` call
/// and must not write outside them. `note_events` and `parameter_events` are
/// sorted by `frame_offset` (stable for equal offsets). `sidechain` carries
/// the detector input for effects declaring `sidechain_input`; it never
/// enters the bus audio sum.
pub struct ProcessContext<'a> {
    pub frames: usize,
    pub sample_rate: f64,
    pub inputs: &'a [&'a [f32]],
    pub outputs: &'a mut [&'a mut [f32]],
    pub note_events: &'a [NoteEvent],
    pub parameter_events: &'a [ParameterEvent<'a>],
    pub sidechain: Option<&'a [&'a [f32]]>,
}

/// A statically registered plugin factory. Implementations must be cheap to
/// query: `descriptor` borrows immutable metadata from the factory.
pub trait Plugin: Send + Sync {
    /// Static metadata; identical across calls and instances.
    fn descriptor(&self) -> &PluginDescriptor;

    /// Create an instance. Runs on the control thread; may allocate.
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance>;

    fn try_create(
        &self,
        host: &HostContext,
    ) -> Result<Box<dyn PluginInstance>, oxitone_core::OxitoneError> {
        Ok(self.create(host))
    }
}

/// One running plugin instance.
///
/// Threading contract (01-architecture.md §Plugin model, 08-plugin-abi.md):
/// `prepare` and destruction run on the control thread; `process`, `reset`,
/// `tail_frames`, and `latency_frames` must be realtime-safe.
pub trait PluginInstance: Send {
    /// (Re)configure for a new sample rate / maximum block size. Control
    /// thread only; may allocate. All latency changes happen here — changing
    /// `latency_frames` during `process` is a fault. Always called before the
    /// first `process` and after any host configuration change.
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32);

    fn try_prepare(
        &mut self,
        sample_rate: f64,
        max_block_size: u32,
    ) -> Result<(), oxitone_core::OxitoneError> {
        self.prepare(sample_rate, max_block_size);
        Ok(())
    }

    /// Render one block. Runs on the audio thread and is hard realtime:
    /// **no allocation, no locks, no blocking I/O, no logging, no clocks, no
    /// N-API or JS calls**. Only preallocated state may be touched; pointers
    /// from the context must not be retained. Output buses must be fully
    /// written for `frames` samples every call (silence included).
    fn process(&mut self, ctx: &mut ProcessContext<'_>);

    /// Clear voices and delay lines on seek/loop. Realtime-safe: reuse only
    /// preallocated state, with no allocation, deallocation, locks, or I/O.
    fn reset(&mut self);

    /// Remaining tail length in frames; used by offline render and graph
    /// swap. Only meaningful when the descriptor sets `reports_tail`;
    /// otherwise the instance is treated as silent immediately after reset.
    fn tail_frames(&self) -> u64;

    /// Processing latency in frames (lookahead, oversampling, …). Feeds the
    /// host's plugin delay compensation; may only change inside `prepare`.
    fn latency_frames(&self) -> u64;
}
