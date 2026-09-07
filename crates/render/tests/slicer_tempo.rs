#[path = "common/allocations.rs"]
mod allocations;
mod common;

use common::*;
use oxitone_core::wire::{AutomationLaneSpec, ProjectSnapshot, TempoCurve, TempoSegment};
use oxitone_render::{
    builtin_registry, resolve_parameter_event, ParamTargetIndex, ParameterEventInput, RenderGraph,
    RenderGraphOptions, SampleStore,
};

fn snapshot(name: &str, sync: &str) -> ProjectSnapshot {
    let mut s = base_snapshot();
    let signal: Vec<f32> = (0..48000)
        .map(|i| (std::f64::consts::TAU * 440.0 * i as f64 / 48000.0).sin() as f32 * 0.5)
        .collect();
    s.samples
        .push(sample_asset(&out_dir(name), "smp_loop", &signal, (2, 1)));
    let mut instrument = wavetable_ref(&[]);
    instrument.plugin_id = "oxitone.slicer".into();
    instrument.state = Some(
        serde_json::json!({ "sampleId": "smp_loop", "slices": {"grid": 1},
        "playMode": "oneshot", "tempoSync": sync }),
    );
    s.channels
        .push(channel("chn_slice", "mix_master", instrument, vec![]));
    s.tracks
        .push(track("trk_slice", &["chn_slice"], &["pcl_slice"], &[]));
    s.patterns.push(pattern(
        "pat_slice",
        (4, 1),
        vec![note(60, (0, 1), (4, 1), 1.0)],
    ));
    s.pattern_clips.push(pattern_clip(
        "pcl_slice",
        "pat_slice",
        "trk_slice",
        (0, 1),
        (4, 1),
    ));
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

fn hz(buffer: &[f32]) -> f64 {
    let crossings = buffer
        .windows(2)
        .filter(|p| p[0] <= 0.0 && p[1] > 0.0)
        .count();
    crossings as f64 * 48000.0 / buffer.len() as f64
}

fn render(g: &mut RenderGraph, frames: usize) -> Vec<f32> {
    let mut out = vec![0.; frames];
    for chunk in out.chunks_mut(64) {
        g.process_block(chunk, &mut [0.; 64][..chunk.len()]);
    }
    out
}

#[test]
fn repitch_updates_an_active_slice_across_tempo_changes_and_block_sizes() {
    for block in [64, 128, 256] {
        let mut s = snapshot(&format!("slicer-step-{block}"), "repitch");
        s.block_size = block;
        s.tempo_map.push(TempoSegment {
            start_beat: beat(1, 2),
            bpm: 240.,
            curve: None,
        });
        let (sync, _, _) = render_frames(&s, 52000);
        assert!((hz(&sync[3000..10000]) - 440.).abs() < 10.);
        assert!((hz(&sync[18000..28000]) - 880.).abs() < 10.);
        assert!(sync[34000..].iter().all(|v| v.abs() < 1e-6));
        s.channels[0].instrument.state.as_mut().unwrap()["tempoSync"] = "off".into();
        let (off, _, _) = render_frames(&s, 52000);
        assert!((hz(&off[18000..28000]) - 440.).abs() < 10.);
        assert!(off[34000..44000].iter().any(|v| v.abs() > 0.01));
    }
}

#[test]
fn ramps_tempo_lanes_and_missing_musical_length_use_the_effective_project_clock() {
    for curve in [TempoCurve::Linear, TempoCurve::Exponential] {
        let mut s = snapshot(&format!("slicer-ramp-{curve:?}"), "repitch");
        s.tempo_map[0].curve = Some(curve);
        s.tempo_map.push(TempoSegment {
            start_beat: beat(2, 1),
            bpm: 240.,
            curve: None,
        });
        let mut g = graph(&s);
        g.transport_mut().begin_render(0);
        let clock = oxitone_transport::TempoMap::compile(&s.tempo_map, 48000).unwrap();
        let audio = render(&mut g, 32000);
        let expected = 440. * clock.bpm_at_frame(20000) / 120.;
        assert!((hz(&audio[18000..22000]) - expected).abs() < 20.);
        s.samples[0].musical_length_beats = None;
        let mut fallback = graph(&s);
        fallback.transport_mut().begin_render(0);
        assert_eq!(audio, render(&mut fallback, 32000));
    }
    let mut s = snapshot("slicer-tempo-lane", "repitch");
    let normalized = (240_f64 / 20.).ln() / (999_f64 / 20.).ln();
    let lane: AutomationLaneSpec = serde_json::from_value(serde_json::json!({
        "id": "auto_tempo", "target": { "entityId": s.id, "parameterId": "tempo" },
        "source": { "kind": "constant", "value": normalized }, "combine": "replace"
    }))
    .unwrap();
    s.automation.push(lane);
    // Track clock only changes scheduling; the shared Channel follows Project tempo.
    s.tracks[0].tempo = Some(90.);
    let (audio, _, _) = render_frames(&s, 32000);
    assert!((hz(&audio[5000..20000]) - 880.).abs() < 10.);
    assert!(audio[28000..].iter().all(|v| v.abs() < 1e-6));
}

#[test]
fn edited_duration_sets_native_tempo_and_slice_rate_multiplies_it() {
    let mut s = snapshot("slicer-edited", "repitch");
    s.samples[0].edits = Some(
        serde_json::from_value(serde_json::json!({
        "startFrame": "12000", "endFrame": "36000" }))
        .unwrap(),
    );
    s.samples[0].musical_length_beats = Some(beat(1, 1));
    let state = s.channels[0].instrument.state.as_mut().unwrap();
    state["slices"] =
        serde_json::json!([{ "start": {"frames": "0"}, "rate": 2., "reverse": true }]);
    let (audio, _, _) = render_frames(&s, 24000);
    assert!((hz(&audio[3000..10000]) - 880.).abs() < 10.);
    assert!(audio[16000..].iter().all(|v| v.abs() < 1e-6));
}

#[test]
fn invalid_sync_and_reserved_factor_fail_before_processing() {
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let s = snapshot("slicer-invalid", "repitch");
    for bad in [
        serde_json::json!("stretch"),
        serde_json::json!(1),
        serde_json::Value::Null,
    ] {
        let mut bad_s = s.clone();
        bad_s.channels[0].instrument.state.as_mut().unwrap()["tempoSync"] = bad;
        assert!(RenderGraph::compile(&bad_s, &registry, &store, &Default::default()).is_err());
    }
    let mut reserved = s.clone();
    reserved.channels[0]
        .instrument
        .parameters
        .insert("tempoFactor".into(), 2.);
    assert!(RenderGraph::compile(&reserved, &registry, &store, &Default::default()).is_err());
    let mut extreme = s.clone();
    extreme.tempo_map.push(TempoSegment {
        start_beat: beat(1, 1),
        bpm: 999.,
        curve: None,
    });
    assert!(RenderGraph::compile(&extreme, &registry, &store, &Default::default()).is_err());
    let mut g = graph(&s);
    let event = ParameterEventInput {
        entity_id: "chn_slice".into(),
        parameter_id: "tempoFactor".into(),
        value: 2.,
        at_frame: None,
    };
    assert_eq!(
        g.enqueue_parameter(&event).unwrap_err().code,
        "AutomationTargetInvalid"
    );
    assert!(resolve_parameter_event(&ParamTargetIndex::from_graph(&g), &event, 0).is_err());
}

#[test]
fn seek_replays_deterministically_and_processing_neither_allocates_nor_frees() {
    let mut s = snapshot("slicer-reset", "repitch");
    s.tempo_map[0].bpm = 240.;
    let mut g = graph(&s);
    g.transport_mut().begin_render(0);
    let first = render(&mut g, 8000);
    g.seek(0);
    let again = render(&mut g, 8000);
    assert!(
        first == again,
        "first different frame: {:?}",
        first
            .iter()
            .zip(&again)
            .enumerate()
            .find(|(_, (a, b))| a != b)
    );
    assert_eq!(
        allocations::count(|| {
            g.seek(0);
            for _ in 0..80 {
                g.process_block(&mut [0.; 128], &mut [0.; 128]);
            }
            g.seek(0);
            g.process_block(&mut [0.; 128], &mut [0.; 128]);
        }),
        (0, 0)
    );
    // Sampler shares this voice pool and must flush a sustaining envelope too.
    s.channels[0].instrument.plugin_id = "oxitone.sampler".into();
    s.channels[0].instrument.state = None;
    s.channels[0].instrument.resources = Some([("sample".into(), "smp_loop".into())].into());
    let mut sampler = graph(&s);
    sampler.transport_mut().begin_render(0);
    let first = render(&mut sampler, 8000);
    sampler.seek(0);
    assert!(first == render(&mut sampler, 8000));
}
