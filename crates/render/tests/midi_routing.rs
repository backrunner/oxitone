//! In-memory isolated processors exercise graph ordering and failure cleanup without a device.
#[path = "common/allocations.rs"]
mod allocations;
mod common;
use oxitone_core::OxitoneError;
use oxitone_graph::{execution::ProcessPosition, midi::MidiEvent, *};
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

struct Factory {
    descriptor: PluginDescriptor,
    role: u8,
    enabled: Arc<AtomicBool>,
    fail: Arc<AtomicBool>,
    received: Arc<AtomicUsize>,
}
impl Plugin for Factory {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
    fn midi_input(&self) -> bool {
        true
    }
    fn midi_output(&self) -> bool {
        true
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance {
            role: self.role,
            enabled: self.enabled.clone(),
            fail: self.fail.clone(),
            received: self.received.clone(),
            input: Vec::with_capacity(256),
            output: Vec::with_capacity(256),
        })
    }
}
struct Instance {
    role: u8,
    enabled: Arc<AtomicBool>,
    fail: Arc<AtomicBool>,
    received: Arc<AtomicUsize>,
    input: Vec<MidiEvent>,
    output: Vec<MidiEvent>,
}
impl PluginInstance for Instance {
    fn configure_midi_output(&mut self, _: bool) -> Result<(), OxitoneError> {
        Ok(())
    }
    fn stage_midi(&mut self, events: &[MidiEvent], _: &[u8]) -> Result<(), OxitoneError> {
        self.input.extend_from_slice(events);
        Ok(())
    }
    fn output_midi(&self) -> (&[MidiEvent], &[u8]) {
        (&self.output, &[])
    }
    fn requires_isolation(&self) -> bool {
        true
    }
    fn prepare(&mut self, _: f64, _: u32) {}
    fn process(&mut self, _: &mut ProcessContext<'_>) {
        panic!("MIDI entered callback")
    }
    fn process_isolated(
        &mut self,
        ctx: &mut ProcessContext<'_>,
        _: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        self.output.clear();
        for output in ctx.outputs.iter_mut() {
            output[..ctx.frames].fill(0.);
        }
        if self.role == 0 && self.enabled.load(Ordering::Relaxed) {
            self.output.push(MidiEvent {
                frame_offset: (ctx.frames - 1) as u32,
                message: midi::MidiMessage::Channel([0x9f, 60, 127]),
            });
        }
        if self.role == 1 && self.fail.swap(false, Ordering::Relaxed) {
            return Err(OxitoneError::new("PluginHostCrashed", "injected failure"));
        }
        if self.role == 2 {
            for event in &self.input {
                self.received.fetch_add(1, Ordering::Relaxed);
                for output in ctx.outputs.iter_mut() {
                    output[event.frame_offset as usize] = 0.5;
                }
            }
        }
        self.input.clear();
        Ok(())
    }
    fn reset(&mut self) {
        self.input.clear();
        self.output.clear();
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
fn setup(fail: bool) -> (RenderGraph, Arc<AtomicBool>, Arc<AtomicUsize>) {
    let mut registry = oxitone_render::builtin_registry().unwrap();
    let mut snapshot = common::base_snapshot();
    let enabled = Arc::new(AtomicBool::new(true));
    let fail = Arc::new(AtomicBool::new(fail));
    let received = Arc::new(AtomicUsize::new(0));
    for (role, id) in ["chn_a", "chn_b", "chn_z"].iter().enumerate() {
        let plugin_id = format!("fixture.midi{role}");
        registry
            .register(Arc::new(Factory {
                descriptor: PluginDescriptor {
                    plugin_id: plugin_id.clone(),
                    plugin_version: "1.0.0".into(),
                    kind: PluginKind::Instrument,
                    input_layout: ChannelLayout::None,
                    output_layout: ChannelLayout::Stereo,
                    parameters: vec![],
                    capabilities: Default::default(),
                    state_schema: None,
                    max_polyphony: Some(16),
                },
                role: role as u8,
                enabled: enabled.clone(),
                fail: fail.clone(),
                received: received.clone(),
            }))
            .unwrap();
        let mut reference = common::wavetable_ref(&[]);
        reference.plugin_id = plugin_id;
        reference.instance_id = Some(format!("ins_{role}"));
        snapshot
            .channels
            .push(common::channel(id, "mix_master", reference, vec![]));
    }
    snapshot.channels[0].midi_routes = Some([("ins_0".into(), vec!["chn_z".into()])].into());
    let graph = RenderGraph::compile(
        &snapshot,
        &registry,
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap();
    (graph, enabled, received)
}
#[test]
fn same_segment_delivery_preserves_offset_for_short_segments_and_seek() {
    let (mut graph, _, received) = setup(false);
    for frames in [1, 17, 128, 63] {
        let mut left = vec![0.; frames];
        let mut right = left.clone();
        let counts = allocations::count(|| {
            graph.seek(0);
            graph.transport_mut().begin_render(0);
            graph.process_offline_block(&mut left, &mut right).unwrap();
        });
        assert_eq!(counts, (0, 0));
        assert!(left[..frames - 1].iter().all(|v| *v == 0.));
        assert!(left[frames - 1] > 0.);
    }
    assert_eq!(received.load(Ordering::Relaxed), 4);
}
#[test]
fn partial_graph_failure_flushes_midi_queued_for_unprocessed_destinations() {
    for process in [
        RenderGraph::process_offline_block,
        RenderGraph::process_isolated_block,
    ] {
        let (mut graph, enabled, received) = setup(true);
        graph.transport_mut().begin_render(0);
        let (mut left, mut right) = ([1.; 128], [1.; 128]);
        assert_eq!(
            process(&mut graph, &mut left, &mut right).unwrap_err().code,
            "PluginHostCrashed"
        );
        assert_eq!(graph.state(), oxitone_render::TransportState::Stopped);
        assert_eq!(left, [0.; 128]);
        assert_eq!(right, left);
        assert_eq!(received.load(Ordering::Relaxed), 0);
        enabled.store(false, Ordering::Relaxed);
        // Reusing the stopped graph must not replay input queued before another source failed.
        graph.transport_mut().begin_render(128);
        process(&mut graph, &mut left, &mut right).unwrap();
        assert_eq!(received.load(Ordering::Relaxed), 0);
        assert_eq!(left, [0.; 128]);
    }
}

#[test]
fn loop_segments_deliver_once_and_direct_callback_never_runs_midi_processors() {
    let (mut graph, _, received) = setup(false);
    graph.transport_mut().play_from(0, Some((0, 19)));
    let (mut left, mut right) = ([0.; 37], [0.; 37]);
    let counts = allocations::count(|| {
        graph.process_isolated_block(&mut left, &mut right).unwrap();
    });
    assert_eq!(counts, (0, 0));
    assert_eq!(received.load(Ordering::Relaxed), 2);
    for (frame, value) in left.iter().enumerate() {
        assert_eq!(*value > 0., frame == 18 || frame == 36);
    }
    let counts = allocations::count(|| graph.process_block(&mut left, &mut right));
    assert_eq!(counts, (0, 0));
    assert!(graph.faulted());
    assert_eq!(received.load(Ordering::Relaxed), 2);
    assert_eq!(left, [0.; 37]);
    assert_eq!(right, left);
}
