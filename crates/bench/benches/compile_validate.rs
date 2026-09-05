//! `compile/validate` — snapshot validation and render-plan compilation
//! latency for a typical project (05-performance-and-benchmarks.md).
//!
//! Fixture: 16 tracks / 16 channels (wavetable + EQ insert), one
//! sixteenth-note pattern clip per track over 256 beats, a shared FX bus,
//! and 32 automation lanes (gate / wave / chance / polyline). Programmatic
//! fixture, fixed seed; see `benches/common/mod.rs`.

mod common;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use common::typical_snapshot;

fn bench_compile(c: &mut Criterion) {
    let registry = oxitone_render::builtin_registry().unwrap();
    let snapshot = typical_snapshot(16, 256, 32);
    let store = oxitone_render::SampleStore::new(None);

    let mut group = c.benchmark_group("compile/validate");
    group.bench_function("validate_16tracks_32lanes", |b| {
        b.iter(|| oxitone_graph::validate(black_box(&snapshot), black_box(&registry)).unwrap());
    });
    group.bench_function("compile_plan_16tracks_32lanes", |b| {
        b.iter(|| {
            oxitone_graph::compile_plan(
                black_box(&snapshot),
                black_box(&registry),
                black_box(&store),
                &oxitone_graph::CompileOptions::default(),
            )
            .unwrap()
        });
    });

    // Scaling reference: half and double size, validate only.
    for tracks in [8usize, 32] {
        let snapshot = typical_snapshot(tracks, 256, tracks * 2);
        group.bench_with_input(
            BenchmarkId::new("validate_scale", format!("{tracks}tracks")),
            &snapshot,
            |b, snapshot| {
                b.iter(|| {
                    oxitone_graph::validate(black_box(snapshot), black_box(&registry)).unwrap()
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_compile);
criterion_main!(benches);
