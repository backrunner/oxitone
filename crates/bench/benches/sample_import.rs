//! Control-thread file read + SHA-256 + decode + PCM disposal (warm filesystem cache).
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oxitone_core::{wire::InspectSampleRequest, PROTOCOL_VERSION};
use oxitone_render::{render_wav::BitDepth, wav::WavWriter};
use oxitone_samples::inspect_sample;

fn sample_import(c: &mut Criterion) {
    let directory =
        std::env::temp_dir().join(format!("oxitone-import-bench-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut group = c.benchmark_group("samples/inspect");
    for (name, seconds, depth) in [
        ("stereo_pcm16_1s", 1, BitDepth::Pcm16),
        ("stereo_float32_10s", 10, BitDepth::Float32),
    ] {
        let path = directory.join(format!("{name}.wav"));
        let frames = 48000 * seconds;
        let audio: Vec<f32> = (0..frames)
            .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / 48000.0).sin() * 0.5)
            .collect();
        let mut writer = WavWriter::create(&path, 48000, depth, None, 128).unwrap();
        for chunk in audio.chunks(128) {
            writer.write_block(chunk, chunk).unwrap();
        }
        writer.finish().unwrap();
        let request = InspectSampleRequest {
            protocol_version: PROTOCOL_VERSION.into(),
            path: path.display().to_string(),
        };
        assert_eq!(inspect_sample(&request).unwrap().frames, frames as u64);
        group.throughput(Throughput::Bytes(std::fs::metadata(&path).unwrap().len()));
        group.bench_function(name, |b| {
            b.iter(|| black_box(inspect_sample(black_box(&request)).unwrap()));
        });
    }
    group.finish();
    std::fs::remove_dir_all(directory).unwrap();
}

criterion_group!(benches, sample_import);
criterion_main!(benches);
