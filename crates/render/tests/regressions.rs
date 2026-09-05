//! Audit probes: temporarily place in crates/render/tests to reproduce.
mod common;

use common::*;
use oxitone_core::{wire::ProjectSnapshot, Beat};
use oxitone_render::realtime::{
    RealtimeConfig, RealtimeSession, SimulatedSinkConfig, TransportCmd,
};
use oxitone_render::{
    builtin_registry, RenderGraph, RenderGraphOptions, SampleStore, TransportState,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::{Duration, Instant};

struct CountingAllocator;
thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn snapshot() -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.tracks.push(track("trk_a", &["chn_a"], &["pcl_a"], &[]));
    s.patterns.push(pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (4, 1), 0.9)],
    ));
    s.pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (4, 1)));
    s.channels
        .push(channel("chn_a", "mix_master", wavetable_ref(&[]), vec![]));
    s
}
fn graph(s: &ProjectSnapshot) -> RenderGraph {
    RenderGraph::compile(
        s,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap()
}
fn session(s: &ProjectSnapshot) -> RealtimeSession {
    RealtimeSession::start_simulated(
        Box::new(graph(s)),
        RealtimeConfig::default(),
        SimulatedSinkConfig {
            sample_rate: 48_000.0,
            frames_per_slice: 128,
            channels: 2,
            latency_frames: 0,
            safety_offset_frames: 0,
        },
        None,
    )
    .map_err(|e| e.error)
    .unwrap()
}
fn wait(mut predicate: impl FnMut() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(start.elapsed() < Duration::from_secs(3), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn small_beat_roundtrip() {
    let x = 1.0 / 24_000.0;
    let actual = Beat::from_f64(x).unwrap().to_f64();
    assert!(
        (actual - x).abs() < 1e-10,
        "one-frame beat: expected {x}, actual {actual}"
    );
}

#[test]
fn inverse_tempo_is_monotone_at_start() {
    let g = graph(&base_snapshot());
    for frame in 1..1000 {
        let previous = g.plan().tempo.frame_to_beat(frame - 1).to_f64();
        let next = g.plan().tempo.frame_to_beat(frame).to_f64();
        assert!(
            next >= previous,
            "frame {} beat={previous}, frame {frame} beat={next}",
            frame - 1
        );
    }
}

#[test]
fn first_audio_block_does_not_allocate() {
    let mut g = graph(&snapshot());
    g.transport_mut().begin_render(0);
    let (mut l, mut r) = ([0.0; 128], [0.0; 128]);
    COUNT.with(|c| c.set(Some(0)));
    g.process_block(&mut l, &mut r);
    let count = COUNT.with(|c| c.replace(None).unwrap());
    assert_eq!(count, 0, "heap allocations in first audio block");
}

#[test]
fn resolved_parameter_insertion_does_not_allocate() {
    let mut g = graph(&snapshot());
    let index = oxitone_render::ParamTargetIndex::from_graph(&g);
    let input = oxitone_render::ParameterEventInput {
        entity_id: "chn_a".into(),
        parameter_id: "mute".into(),
        value: 1.0,
        at_frame: Some(64),
    };
    let event = oxitone_render::resolve_parameter_event(&index, &input, 0).unwrap();
    COUNT.with(|c| c.set(Some(0)));
    g.insert_queued_parameter(event);
    let count = COUNT.with(|c| c.replace(None).unwrap());
    assert_eq!(
        count, 0,
        "heap allocations in audio-thread parameter insertion"
    );
}

#[test]
fn replace_graph_preserves_playback() {
    let s = snapshot();
    let live = session(&s);
    live.transport(TransportCmd::Play {
        from: None,
        loop_region: None,
    })
    .unwrap();
    wait(|| live.cursor() > 2048);
    live.replace_graph(Box::new(graph(&s))).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(live.transport_state(), TransportState::Playing);
    assert!(live.cursor() > 2048);
}

#[test]
fn replace_graph_refreshes_tempo_mapping() {
    let mut s = snapshot();
    let live = session(&s);
    s.tempo_map[0].bpm = 60.0;
    live.replace_graph(Box::new(graph(&s))).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(live.beat_to_frame(beat(1, 1)), 48_000);
}

#[test]
fn parameter_inside_block_applies_at_target_frame() {
    let mut g = graph(&snapshot());
    g.enqueue_parameter(&oxitone_render::ParameterEventInput {
        entity_id: "chn_a".into(),
        parameter_id: "mute".into(),
        value: 1.0,
        at_frame: Some(64),
    })
    .unwrap();
    g.transport_mut().begin_render(0);
    let (mut l, mut r) = ([0.0; 128], [0.0; 128]);
    g.process_block(&mut l, &mut r);
    assert!(l[..64].iter().any(|x| x.abs() > 0.0));
    assert_eq!(
        g.pending_parameter_events(),
        0,
        "frame-64 event still queued after rendering frame 127"
    );
}

#[test]
fn gate_boundary_inside_block_is_sample_accurate() {
    let mut s = snapshot();
    s.automation.push(
        serde_json::from_value(serde_json::json!({
            "id": "auto_mute", "target": {"entityId": "chn_a", "parameterId": "mute"},
            "source": {"kind": "gate", "periodBeats": {"numerator": 1, "denominator": 100},
                "duty": 0.5, "on": 0.0, "off": 1.0}
        }))
        .unwrap(),
    );
    let mut g = graph(&s);
    g.transport_mut().begin_render(0);
    let (mut l, mut r) = ([0.0; 128], [0.0; 128]);
    g.process_block(&mut l, &mut r);
    assert!(l[..120].iter().any(|x| x.abs() > 0.0));
    assert!(
        l[120..].iter().all(|x| *x == 0.0),
        "mute gate did not switch at frame 120"
    );
}

#[test]
fn mixer_insert_bypass_is_honored() {
    let registry = builtin_registry().unwrap();
    let mut effect = effect_ref("oxitone.utility", &[("polarity", 1.0)]);
    effect.bypass = Some(true);
    let mut mixer = oxitone_mixer::MixerEngine::build(
        48_000.0,
        128,
        &[mixer_channel("mix_master", vec![effect], vec![])],
        &registry,
    )
    .unwrap();
    let input = [0.5; 128];
    let (mut l, mut r) = ([0.0; 128], [0.0; 128]);
    mixer.process_block_with(
        [oxitone_mixer::ChannelInput {
            bus_id: "mix_master",
            left: &input,
            right: &input,
        }],
        &mut l,
        &mut r,
    );
    assert!(
        l.iter().all(|x| *x > 0.0),
        "bypassed polarity insert still inverted the bus"
    );
}

#[test]
fn empty_track_stem_is_silent() {
    let mut s = snapshot();
    s.tracks.push(track("trk_empty", &[], &[], &[]));
    let mut options = oxitone_render::RenderOptions::new(out_dir("review-empty-stem"));
    options.stems = oxitone_render::render_wav::StemMode::Tracks;
    let report = oxitone_render::render_wav(
        &s,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &options,
    )
    .unwrap();
    let empty = report
        .files
        .iter()
        .find(|f| f.stem.as_deref() == Some("trk_empty"))
        .unwrap();
    assert!(
        empty.peak_dbfs < -90.0,
        "empty track contains master audio: {} dBFS",
        empty.peak_dbfs
    );
}

#[test]
fn track_routed_directly_to_master_has_audible_stem() {
    let s = snapshot();
    let mut options = oxitone_render::RenderOptions::new(out_dir("review-master-stem"));
    options.stems = oxitone_render::render_wav::StemMode::Tracks;
    let report = oxitone_render::render_wav(
        &s,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &options,
    )
    .unwrap();
    let stem = report
        .files
        .iter()
        .find(|f| f.stem.as_deref() == Some("trk_a"))
        .unwrap();
    assert!(
        stem.peak_dbfs > -90.0,
        "audible track stem is silent: {} dBFS",
        stem.peak_dbfs
    );
}

#[test]
fn transport_loop_does_not_render_events_past_loop_end() {
    let mut s = snapshot();
    s.patterns[0].notes[0].start = beat(1, 250); // frame 96 at 120 bpm
    let mut g = graph(&s);
    g.transport_mut().play_from(0, Some((0, 64)));
    let (mut l, mut r) = ([0.0; 128], [0.0; 128]);
    g.process_block(&mut l, &mut r);
    assert!(
        l.iter().all(|x| *x == 0.0),
        "note at frame 96 sounded outside loop [0,64)"
    );
}

#[test]
fn extensible_wav_24_valid_bits_in_32_bit_container() {
    let mut fmt = vec![0u8; 40];
    fmt[0..2].copy_from_slice(&0xfffe_u16.to_le_bytes());
    fmt[2..4].copy_from_slice(&1u16.to_le_bytes());
    fmt[4..8].copy_from_slice(&48_000u32.to_le_bytes());
    fmt[8..12].copy_from_slice(&192_000u32.to_le_bytes());
    fmt[12..14].copy_from_slice(&4u16.to_le_bytes());
    fmt[14..16].copy_from_slice(&32u16.to_le_bytes());
    fmt[16..18].copy_from_slice(&22u16.to_le_bytes());
    fmt[18..20].copy_from_slice(&24u16.to_le_bytes());
    fmt[24..40].copy_from_slice(&[1, 0, 0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&76u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&fmt);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    for _ in 0..4 {
        bytes.extend_from_slice(&0x4000_0000u32.to_le_bytes());
    }
    let decoded =
        oxitone_samples::decode_bytes(&bytes, oxitone_core::wire::SampleFormat::Wav).unwrap();
    assert_eq!(
        decoded.channels[0].len(),
        4,
        "container width must determine frame stride"
    );
    assert!(decoded.channels[0].iter().all(|x| (*x - 0.5).abs() < 1e-6));
}

#[test]
fn midi_export_uses_effective_tempo_lane() {
    let mut s = snapshot();
    s.automation.push(
        serde_json::from_value(serde_json::json!({
            "id": "auto_tempo", "target": {"entityId": s.id, "parameterId": "tempo"},
            "source": {"kind": "constant", "value": (60.0f64 / 20.0).ln() / (999.0f64 / 20.0).ln()}
        }))
        .unwrap(),
    );
    assert!((graph(&s).plan().tempo.bpm_at_frame(0) - 60.0).abs() < 1e-8);
    let exported = oxitone_render::midi::export_midi(&s, &Default::default()).unwrap();
    let event = exported
        .bytes
        .windows(6)
        .find(|bytes| bytes[..3] == [0xff, 0x51, 3])
        .unwrap();
    let micros = u32::from_be_bytes([0, event[3], event[4], event[5]]);
    assert_eq!(
        micros, 1_000_000,
        "MIDI must use the same 60 bpm clock as audio"
    );
}

#[test]
fn zero_note_chance_produces_no_note_events() {
    let mut s = snapshot();
    s.patterns[0].notes[0].chance = Some(0.0);
    assert!(
        graph(&s).plan().scheduler.is_empty(),
        "chance=0 note still scheduled"
    );
}

#[test]
fn exclusive_last_beat_clips_note_off() {
    let mut s = snapshot();
    s.pattern_clips[0].duration_beats = None;
    s.pattern_clips[0].last_beat = Some(beat(1, 1));
    let g = graph(&s);
    let last = g.plan().scheduler.events().last().unwrap();
    assert!(
        last.frame <= 24_000,
        "note-off {} is beyond exclusive clip end 24000",
        last.frame
    );
}

#[test]
fn valid_deep_restart_chance_does_not_panic() {
    use oxitone_core::wire::{AutomationSourceSpec, ChanceSpec, RandomPhase};
    let mut source = AutomationSourceSpec::Chance(ChanceSpec {
        probability: 0.5,
        seed: 9_007_199_254_740_991,
        smooth_beats: None,
        random_phase: Some(RandomPhase::Restart),
        rate: Some(1.0),
        interval_beats: None,
    });
    for _ in 0..63 {
        source = AutomationSourceSpec::Map {
            input: Box::new(source),
            min: 0.0,
            max: 1.0,
        };
    }
    let evaluator =
        oxitone_transport::automation::CompiledAutomation::compile(&source, 9_007_199_254_740_991)
            .unwrap();
    let value = evaluator.value_at(0.0, &Default::default());
    assert!(value.is_finite());
}
