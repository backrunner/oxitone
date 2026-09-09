//! Playlist automation spans at the supported per-lane placement budget.
mod common;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_core::wire::*;
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

fn bench(c: &mut Criterion) {
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    for count in [32, 1024] {
        let mut s = common::typical_snapshot(4, 4, 0);
        s.automation.clear();
        s.automation.push(AutomationLaneSpec {
            id: "auto_playlist".into(),
            target: AutomationTarget {
                entity_id: s.channels[0].id.clone(),
                parameter_id: "level".into(),
                scope: None,
            },
            playback: Some(AutomationPlayback::Playlist),
            source: common::wave_source(WaveKind::Sine, (4, 1)),
            combine: None,
            loop_spec: None,
            last_beat: None,
        });
        s.automation_clips = Some(
            (0..count)
                .map(|i| AutomationClipSpec {
                    id: format!("acl_{i:04}"),
                    lane_id: "auto_playlist".into(),
                    track_id: s.tracks[0].id.clone(),
                    start_beat: common::beat(i, 64),
                    duration_beats: Some(common::beat(1, 8)),
                    enabled: None,
                })
                .collect(),
        );
        let mut g =
            RenderGraph::compile(&s, &registry, &store, &RenderGraphOptions::default()).unwrap();
        g.transport_mut().play_from(0, Some((0, 96000)));
        let (mut l, mut r) = ([0.; 128], [0.; 128]);
        let mut durations = vec![0_u128; 2048];
        for duration in &mut durations {
            let start = std::time::Instant::now();
            g.process_block(&mut l, &mut r);
            *duration = start.elapsed().as_nanos();
        }
        durations.sort_unstable();
        eprintln!("Playlist DSP profile: {count} placements; 48000 Hz; 128 frames; p95={} ns; p99={} ns; offline sink (no device callback or xruns)", durations[1945], durations[2027]);
        c.bench_function(&format!("playlist/{count}_placements/render_128"), |b| {
            b.iter(|| {
                g.process_block(black_box(&mut l), black_box(&mut r));
                black_box((&l, &r));
            })
        });
    }
}
criterion_group!(benches, bench);
criterion_main!(benches);
