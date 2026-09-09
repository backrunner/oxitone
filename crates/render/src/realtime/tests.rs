//! Core-level realtime tests: direct/buffered parity and the device-rate
//! resampler path. Session-level behavior (simulated sink, jitter,
//! underrun) lives in `crates/render/tests/realtime.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_core::beat::Beat;
use oxitone_core::wire::{
    ChannelSpec, InstrumentRef, NoteSpec, PatternClipSpec, PatternSpec, ProjectSnapshot,
    TempoSegment, TimeSignatureSegment, TrackSpec,
};
use oxitone_core::PROTOCOL_VERSION;

use super::diagnostics::RtCounters;
use super::direct::DirectCore;
use super::layout::stereo_to_device;
use super::ring::{SpscQueue, SpscRing};
use super::worker::{make_buffered_pull, ResamplerPair, TransportMirror, WorkerCore, WorkerMsg};
use crate::realtime::diagnostics::DiagnosticEvent;
use crate::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

fn playable_snapshot() -> ProjectSnapshot {
    let note = |pitch: u8, start: (i64, u32)| NoteSpec {
        id: None,
        pitch,
        start: beat(start.0, start.1),
        duration: beat(1, 1),
        velocity: 0.9,
        off_velocity: None,
        chance: None,
        voice: None,
        tags: None,
    };
    ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION.to_string(),
        revision: 1,
        id: "prj_rt".into(),
        name: None,
        sample_rate: 48_000,
        block_size: 128,
        seed: 7,
        tempo_map: vec![TempoSegment {
            start_beat: Beat::ZERO,
            bpm: 120.0,
            curve: None,
        }],
        time_signature_map: vec![TimeSignatureSegment {
            start_bar: 1,
            numerator: 4,
            denominator: 4,
        }],
        markers: vec![],
        tracks: vec![TrackSpec {
            id: "trk_a".into(),
            name: None,
            channel_ids: vec!["chn_a".into()],
            tempo: None,
            pattern_clip_ids: vec!["pcl_a".into()],
            sample_clip_ids: vec![],
            enabled: None,
            mute: None,
            solo: None,
            midi_channel: None,
        }],
        patterns: vec![PatternSpec {
            id: "pat_a".into(),
            name: None,
            length_beats: beat(4, 1),
            notes: vec![note(60, (0, 1)), note(64, (1, 1)), note(67, (2, 1))],
            parts: None,
        }],
        pattern_clips: vec![PatternClipSpec {
            id: "pcl_a".into(),
            pattern_id: "pat_a".into(),
            track_id: "trk_a".into(),
            start_beat: beat(0, 1),
            duration_beats: Some(beat(4, 1)),
            loop_count: None,
            last_beat: None,
            transpose: None,
            velocity_scale: None,
            probability: None,
            enabled: None,
        }],
        sample_clips: vec![],
        samples: vec![],
        channels: vec![ChannelSpec {
            id: "chn_a".into(),
            name: None,
            instrument: InstrumentRef {
                instance_id: None,
                plugin_id: "oxitone.wavetable".into(),
                plugin_version: "1.0.0".into(),
                parameters: BTreeMap::new(),
                resources: None,
                state: None,
            },
            effect_chain: vec![],
            level: 1.0,
            pan: 0.0,
            swing: None,
            mixer_channel_id: "mix_master".into(),
            mute: None,
            solo: None,
        }],
        mixer_channels: vec![],
        automation: vec![],
        automation_clips: None,
    }
}

pub(super) fn compile_graph() -> Box<RenderGraph> {
    let mut graph = Box::new(
        RenderGraph::compile(
            &playable_snapshot(),
            &builtin_registry().unwrap(),
            &SampleStore::new(None),
            &RenderGraphOptions::default(),
        )
        .unwrap(),
    );
    graph.transport_mut().play_from(0, None);
    graph
}

type TestQueues = (
    Arc<SpscQueue<WorkerMsg>>,
    Arc<SpscQueue<DiagnosticEvent>>,
    Arc<SpscQueue<DiagnosticEvent>>,
    Arc<RtCounters>,
    Arc<TransportMirror>,
);

pub(super) fn fresh_queues() -> TestQueues {
    (
        Arc::new(SpscQueue::new(16)),
        Arc::new(SpscQueue::new(16)),
        Arc::new(SpscQueue::new(16)),
        Arc::new(RtCounters::new()),
        Arc::new(TransportMirror::default()),
    )
}

/// 03 §线程模型: 同一 graph 在 direct 与 buffered 模式下输出必须
/// sample-accurate 一致.
#[test]
fn direct_and_buffered_modes_are_sample_accurate() {
    let blocks = 64;
    let block = 128;
    let channels = 2;

    // Reference: plain process_block + layout conversion.
    let mut reference = compile_graph();
    reference.transport_mut().play_from(0, None);
    let mut expected = Vec::with_capacity(blocks * block * channels);
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    for _ in 0..blocks {
        reference.process_block(&mut l, &mut r);
        let start = expected.len();
        expected.resize(start + block * channels, 0.0);
        stereo_to_device(&l, &r, &mut expected[start..], channels);
    }

    // Direct mode.
    let (commands, worker_events, _, counters, mirror) = fresh_queues();
    let return_slot = Arc::new(std::sync::Mutex::new(None));
    let mut direct = DirectCore::new(
        compile_graph(),
        return_slot,
        commands,
        Arc::new(SpscQueue::new(256)),
        worker_events,
        counters,
        mirror,
        channels,
    );
    let mut direct_out = Vec::with_capacity(blocks * block * channels);
    let mut chunk = vec![0.0; block * channels];
    for _ in 0..blocks {
        direct.pull(&mut chunk);
        direct_out.extend_from_slice(&chunk);
    }

    // Buffered mode: worker renders into the ring, the pull closure
    // drains it in device-sized chunks.
    let (commands, worker_events, callback_events, counters, mirror) = fresh_queues();
    let ring = Arc::new(SpscRing::new(4 * block, channels));
    let return_slot = Arc::new(std::sync::Mutex::new(None));
    let mut worker = WorkerCore::new(
        compile_graph(),
        ring.clone(),
        None,
        commands,
        Arc::new(SpscQueue::new(256)),
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
        worker_events,
        counters.clone(),
        mirror.clone(),
        return_slot,
        channels,
        block,
    );
    let mut pull = make_buffered_pull(ring, counters, callback_events, mirror);
    let mut buffered_out = Vec::with_capacity(blocks * block * channels);
    // Interleave render/drain like the real threads do, so the ring
    // never fills.
    for _ in 0..blocks {
        worker.render_block();
        pull(&mut chunk);
        buffered_out.extend_from_slice(&chunk);
    }

    assert_eq!(direct_out.len(), expected.len());
    assert_eq!(
        direct_out, expected,
        "direct mode diverged from process_block output"
    );
    assert_eq!(
        buffered_out, expected,
        "buffered mode diverged from process_block output"
    );
}

/// Resample policy: 48 kHz project rendered to a 44.1 kHz device keeps
/// the pitch and roughly the energy (coarse quality check; the
/// SincResampler itself has dedicated SNR tests in oxitone-dsp).
#[test]
fn resampler_pair_preserves_sine() {
    let project_rate = 48_000.0;
    let device_rate = 44_100.0;
    let block = 128;
    let mut pair = ResamplerPair::new(project_rate, device_rate, block, block);
    let mut out = vec![0.0; 2 * block];
    let mut produced = Vec::new();
    let blocks = 200;
    for b in 0..blocks {
        let l: Vec<f32> = (0..block)
            .map(|i| {
                let t = (b * block + i) as f64 / project_rate;
                (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32
            })
            .collect();
        let r = l.clone();
        let frames = pair.process_block(&l, &r, &mut out, 2);
        for f in 0..frames {
            produced.push(out[2 * f]);
        }
    }
    // 1.7067 s of project audio → ~1.7067 s of device audio (±1 block).
    let expected = (blocks * block) as f64 * device_rate / project_rate;
    assert!(
        (produced.len() as f64 - expected).abs() < 2.0 * block as f64,
        "produced {} frames, expected ≈{expected}",
        produced.len()
    );
    // After the group delay, output frame j corresponds to project
    // input position `latency + j * step`.
    let step = project_rate / device_rate;
    let delay = pair.group_delay_project_frames();
    let start = (delay / step) as usize + 64;
    let check = &produced[start..start + 2048];
    let max_err = check
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let input_pos = delay + (start + i) as f64 * step;
            let t = input_pos / project_rate;
            (x - (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32).abs()
        })
        .fold(0.0f32, f32::max);
    assert!(max_err < 0.02, "max sine deviation {max_err}");
}
