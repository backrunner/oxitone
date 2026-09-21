//! Background-only request/completion processing; reset itself only marks state dirty.
use super::{error, invalid, RegistrationOptions};
use oxitone_core::OxitoneError;
use oxitone_graph::execution::ProcessPosition;
use oxitone_graph::{HostContext, PluginInstance, ProcessContext};
use oxitone_vst3_host::{
    stream::{RealtimePort, Session, SessionOptions},
    stream_wire::Start,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

mod events;
mod midi;
mod processing;

pub(super) struct Instance {
    options: RegistrationOptions,
    start: Start,
    session: Option<Session>,
    port: Option<RealtimePort>,
    audio_outputs: Vec<[Vec<f32>; 2]>,
    audio_inputs: Vec<[Vec<f32>; 2]>,
    silence: Vec<f32>,
    events: Vec<oxitone_vst3_host::wire::Event>,
    midi: midi::MidiState,
    latency: u64,
    tail: u64,
    processed: bool,
    dirty: bool,
    failed: bool,
    faults: Arc<AtomicU64>,
    control: Arc<super::control::Control>,
}
impl Instance {
    pub(super) fn new(
        options: RegistrationOptions,
        host: &HostContext,
        faults: Arc<AtomicU64>,
    ) -> Self {
        let start = options.start(host.sample_rate as u32, host.max_block_size as usize);
        Self {
            options,
            start,
            session: None,
            port: None,
            audio_outputs: Vec::new(),
            audio_inputs: Vec::new(),
            silence: Vec::new(),
            events: Vec::with_capacity(oxitone_vst3_host::stream_wire::MAX_EVENTS),
            midi: Default::default(),
            latency: 0,
            tail: 0,
            processed: false,
            dirty: false,
            failed: false,
            faults,
            control: Arc::new(super::control::Control::default()),
        }
    }
    pub(super) fn configure(
        &mut self,
        parameters: &BTreeMap<String, f64>,
        state: Option<&serde_json::Value>,
    ) -> Result<(), OxitoneError> {
        self.start.options.configuration = state
            .map(|state| serde_json::from_value(state.clone()).map_err(|e| invalid(e.to_string())))
            .transpose()?;
        self.start.options.parameters = parameters.clone();
        self.start.validate().map_err(error)
    }
    fn spawn(&mut self, preserve_latency: bool) -> Result<(), OxitoneError> {
        self.control.replace(None);
        // Waits/destruction only happen in prepare or the explicit isolated executor.
        self.port.take();
        self.session.take();
        let (session, port) = Session::spawn(
            &self.options.helper_path,
            self.start.clone(),
            SessionOptions::default(),
        )
        .map_err(error)?;
        self.options.metadata.check_ready(session.info())?;
        let latency = u64::from(session.info().latency_frames);
        if preserve_latency && latency != self.latency {
            return Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "VST3 restart changed latency; recompile the graph",
            ));
        }
        self.latency = latency;
        self.tail = u64::from(session.info().tail_frames);
        self.control.replace(Some(session.controller()));
        self.session = Some(session);
        self.port = Some(port);
        self.dirty = false;
        self.failed = false;
        self.processed = false;
        Ok(())
    }
}
impl PluginInstance for Instance {
    fn configure_midi_output(&mut self, enabled: bool) -> Result<(), OxitoneError> {
        if self.port.is_some() || (enabled && !self.options.metadata.note_output) {
            return Err(invalid(
                "MIDI capture requires output capability and configuration before prepare",
            ));
        }
        self.start.options.midi_output = enabled;
        Ok(())
    }
    fn stage_midi(
        &mut self,
        events: &[oxitone_graph::midi::MidiEvent],
        payload: &[u8],
    ) -> Result<(), OxitoneError> {
        if !self.options.metadata.note_input {
            return Err(invalid("VST3 has no MIDI input"));
        }
        self.midi.stage(events, payload, self.silence.len())
    }
    fn output_midi(&self) -> (&[oxitone_graph::midi::MidiEvent], &[u8]) {
        if self.failed {
            (&[], &[])
        } else {
            (&self.midi.output, &self.midi.output_payload)
        }
    }
    fn configure_input_buses(&mut self, buses: &[usize]) -> Result<(), OxitoneError> {
        if self.port.is_some() {
            return Err(invalid("VST3 input routes must be set before prepare"));
        }
        let activation = self
            .start
            .options
            .bus_activation
            .as_mut()
            .expect("graph activation");
        if buses
            .iter()
            .any(|i| *i == 0 || *i >= activation.inputs.len())
        {
            return Err(invalid("invalid VST3 auxiliary input bus"));
        }
        for (i, active) in activation.inputs.iter_mut().enumerate() {
            *active = i == 0 || buses.contains(&i);
        }
        Ok(())
    }
    fn input_bus_mut(&mut self, index: usize) -> Option<[&mut [f32]; 2]> {
        if index == 0 {
            return None;
        }
        let [left, right] = self.audio_inputs.get_mut(index)?;
        Some([left, right])
    }
    fn initial_parameters_applied(&self) -> bool {
        true
    }
    fn native_control(&self) -> Option<Arc<dyn oxitone_graph::control::NativeControl>> {
        Some(self.control.clone())
    }
    fn configure_output_buses(&mut self, buses: &[usize]) -> Result<(), OxitoneError> {
        if self.port.is_some() {
            return Err(invalid("VST3 output routes must be set before prepare"));
        }
        let activation = self
            .start
            .options
            .bus_activation
            .as_mut()
            .expect("graph activation");
        if buses
            .iter()
            .any(|i| *i == 0 || *i >= activation.outputs.len())
        {
            return Err(invalid("invalid VST3 auxiliary output bus"));
        }
        for (i, active) in activation.outputs.iter_mut().enumerate() {
            *active = i == 0 || buses.contains(&i);
        }
        Ok(())
    }
    fn output_bus(&self, index: usize) -> Option<[&[f32]; 2]> {
        if !self.processed || self.failed {
            return None;
        }
        self.audio_outputs
            .get(index)
            .map(|[left, right]| [left.as_slice(), right.as_slice()])
    }
    fn requires_isolation(&self) -> bool {
        true
    }
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        let _ = self.try_prepare(sample_rate, max_block_size);
    }
    fn try_prepare(&mut self, sample_rate: f64, max_block_size: u32) -> Result<(), OxitoneError> {
        if sample_rate.fract() != 0.0 || !(8000.0..=192000.0).contains(&sample_rate) {
            return Err(invalid("invalid VST3 sample rate"));
        }
        self.start.options.sample_rate = sample_rate as u32;
        self.start.options.block_size = max_block_size as usize;
        self.start.validate().map_err(error)?;
        self.silence = vec![0.0; max_block_size as usize];
        self.audio_inputs = (0..self.options.metadata.audio_buses.inputs.len())
            .map(|_| [self.silence.clone(), self.silence.clone()])
            .collect();
        self.audio_outputs = (0..self.options.metadata.audio_buses.outputs.len())
            .map(|_| [self.silence.clone(), self.silence.clone()])
            .collect();
        self.spawn(false)
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        // Fail closed if a caller bypasses RenderGraph's direct-mode capability check.
        for output in ctx.outputs.iter_mut() {
            output[..ctx.frames].fill(0.0);
        }
        if !self.failed {
            self.faults.fetch_add(1, Ordering::Relaxed);
            self.failed = true;
        }
    }
    fn process_isolated(
        &mut self,
        ctx: &mut ProcessContext<'_>,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        let result = self.process_external(ctx, position);
        if result.is_err() {
            self.midi.clear();
            for output in ctx.outputs.iter_mut() {
                output[..ctx.frames].fill(0.0);
            }
            if !self.failed {
                self.faults.fetch_add(1, Ordering::Relaxed);
                self.failed = true;
            }
        }
        result
    }
    fn reset(&mut self) {
        self.midi.clear();
        self.dirty |= self.processed || self.failed;
    }
    fn latency_frames(&self) -> u64 {
        self.latency
    }
    fn tail_frames(&self) -> u64 {
        self.tail
    }
}
