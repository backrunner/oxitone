//! DSP-only cost of new effects, including decaying denormal tails. No output device.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_graph::{HostContext, ParameterEvent, ProcessContext};

fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("effects/electronic");
    for name in [
        "oxitone.distortion",
        "oxitone.multiband",
        "oxitone.delay",
        "oxitone.nonlinear-filter",
        "oxitone.compactor",
        "oxitone.multiband-dynamics",
        "oxitone.resonator",
        "oxitone.frequency-shifter",
        "oxitone.pitch-shifter",
        "oxitone.flanger",
        "oxitone.convolver",
        "oxitone.bitcrush",
        "oxitone.tape",
        "oxitone.spreader",
        "oxitone.limiter",
        "oxitone.compressor",
        "oxitone.gate",
        "oxitone.reverb",
        "oxitone.filter",
        "oxitone.limit",
    ] {
        for signal in ["tone", "decay"] {
            let plugin = oxitone_mixer::builtin_effect_plugins()
                .into_iter()
                .find(|p| p.descriptor().plugin_id == name)
                .unwrap();
            let mut instance = plugin.create(&HostContext {
                sample_rate: 48000.,
                max_block_size: 128,
            });
            let events = if name == "oxitone.delay" {
                vec![
                    ParameterEvent {
                        frame_offset: 0,
                        parameter_id: "pingPong",
                        value: 1.,
                    },
                    ParameterEvent {
                        frame_offset: 0,
                        parameter_id: "timeSeconds",
                        value: 0.002,
                    },
                    ParameterEvent {
                        frame_offset: 0,
                        parameter_id: "ducking",
                        value: 0.5,
                    },
                ]
            } else {
                vec![]
            };
            let (mut left, mut right) = ([0.; 128], [0.; 128]);
            let input = std::array::from_fn::<_, 128, _>(|i| {
                0.4 * (std::f32::consts::TAU * i as f32 / 48.).sin()
            });
            instance.process(&mut ProcessContext {
                frames: 128,
                sample_rate: 48000.,
                inputs: &[&input, &input],
                outputs: &mut [&mut left, &mut right],
                note_events: &[],
                parameter_events: &events,
                sidechain: None,
            });
            let mut block = 0u64;
            let mut buffer = input;
            group.bench_function(format!("{name}_{signal}_128"), |b| {
                b.iter(|| {
                    let _ftz = oxitone_dsp::ftz::FtzGuard::new();
                    if signal == "decay" {
                        let gain = 2f32.powf(-((block % 512) as f32) / 3.);
                        for i in 0..128 {
                            buffer[i] = input[i] * gain;
                        }
                    }
                    instance.process(&mut ProcessContext {
                        frames: 128,
                        sample_rate: 48000.,
                        inputs: &[&buffer, &buffer],
                        outputs: &mut [&mut left, &mut right],
                        note_events: &[],
                        parameter_events: &[],
                        sidechain: None,
                    });
                    block += 1;
                    black_box((&left, &right));
                })
            });
            assert!(left.iter().chain(&right).all(|v| v.is_finite()));
        }
    }
    group.finish();
}
criterion_group!(benches, benchmark);
criterion_main!(benches);
