mod common;
use common::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

fn preview(c: &mut Criterion) {
    let mut s = base_snapshot();
    for i in 0..4 {
        let channel_id = format!("chn_{i}");
        let track_id = format!("trk_{i}");
        let pattern_id = format!("pat_{i}");
        let clip_id = format!("pcl_{i}");
        s.channels.push(channel(
            &channel_id,
            "mix_master",
            wavetable_ref(&[]),
            vec![],
        ));
        s.tracks.push(track(&track_id, &[&channel_id], &[&clip_id]));
        s.patterns
            .push(sixteenth_pattern(&pattern_id, (4, 1), 60 + i));
        s.pattern_clips.push(pattern_clip(
            &clip_id,
            &pattern_id,
            &track_id,
            (0, 1),
            (4, 1),
        ));
    }
    let mut group = c.benchmark_group("preview/4_channels_128");
    for enabled in [false, true] {
        let mut graph = RenderGraph::compile(
            &s,
            &builtin_registry().unwrap(),
            &SampleStore::new(None),
            &RenderGraphOptions::default(),
        )
        .unwrap();
        let telemetry = enabled.then(|| graph.enable_preview());
        graph.transport_mut().begin_render(0);
        let (mut l, mut r, mut drain) = ([0.; 128], [0.; 128], [0.; 8192]);
        let mut blocks = 0;
        group.bench_function(if enabled { "telemetry" } else { "baseline" }, |b| {
            b.iter(|| {
                if blocks % 180 == 0 {
                    graph.seek(0);
                }
                graph.process_block(&mut l, &mut r);
                // Simulate 30 Hz ring consumption; excludes UI FFT/painting.
                if blocks % 12 == 0 {
                    if let Some(t) = &telemetry {
                        for n in t.channels.iter().chain(&t.buses) {
                            n.audio.read_frames(&mut drain);
                            while n.notes.pop().is_some() {}
                            black_box(n.meter());
                        }
                    }
                }
                blocks += 1;
                black_box((&l, &r));
            })
        });
    }
    group.finish();
}
criterion_group!(benches, preview);
criterion_main!(benches);
