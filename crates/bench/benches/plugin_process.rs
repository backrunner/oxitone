//! C ABI adapter cost, including event conversion and finite-output checks.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxitone_core::wire::{AllowPlugins, RegisterPluginOptions};
use oxitone_graph::{HostContext, ParameterEvent, Plugin, ProcessContext};
use oxitone_render::plugins::load_plugin;
use std::{path::PathBuf, process::Command};

fn bench_plugin(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("target/tmp/plugin-bench");
    std::fs::create_dir_all(&dir).unwrap();
    let library = dir.join(format!("gain{}", std::env::consts::DLL_SUFFIX));
    assert!(Command::new("cc")
        .args(["-shared", "-fPIC", "-O3", "-I"])
        .arg(root.join("include"))
        .arg(root.join("crates/render/tests/fixtures/gain.c"))
        .arg("-o")
        .arg(&library)
        .status()
        .unwrap()
        .success());
    let options = RegisterPluginOptions {
        library_path: library.display().to_string(),
        expected_hash: None,
        manifest: serde_json::from_str(include_str!("../../render/tests/fixtures/gain.json"))
            .unwrap(),
    };
    let plugin = unsafe { load_plugin(&options, AllowPlugins::Any) }.unwrap();
    let mut group = c.benchmark_group("plugin/c_abi_gain");
    for frames in [64, 128, 256] {
        group.throughput(Throughput::Elements(frames as u64));
        let host = HostContext {
            sample_rate: 48000.,
            max_block_size: frames as u32,
        };
        let mut instance = plugin.try_create(&host).unwrap();
        instance.try_prepare(48000., frames as u32).unwrap();
        let input = vec![0.25; frames];
        let mut left = vec![0.; frames];
        let mut right = vec![0.; frames];
        let events = [ParameterEvent {
            frame_offset: 0,
            parameter_id: "gain",
            value: 0.5,
        }];
        group.bench_with_input(
            BenchmarkId::from_parameter(frames),
            &frames,
            |b, &frames| {
                b.iter(|| {
                    instance.process(&mut ProcessContext {
                        frames,
                        sample_rate: 48000.,
                        inputs: &[&input, &input],
                        outputs: &mut [&mut left, &mut right],
                        note_events: &[],
                        parameter_events: &events,
                        sidechain: None,
                    });
                    black_box(&left);
                });
            },
        );
        assert_eq!(left[0], 0.125);
    }
    assert_eq!(plugin.fault_count(), 0);
    group.finish();
}

criterion_group!(benches, bench_plugin);
criterion_main!(benches);
