//! Native synth at 48 kHz / stereo / 128 frames. No device or mixer in this microbench.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_graph::{HostContext, NoteEvent, NoteEventKind, PluginInstance, ProcessContext};
use oxitone_instruments::{wavetable::WavetableSynthPlugin, InstrumentConfig};
use std::collections::BTreeMap;

fn benchmark(c: &mut Criterion) {
    let host = HostContext {
        sample_rate: 48000.,
        max_block_size: 128,
    };
    let mut group = c.benchmark_group("instruments/synth_motion");
    for voices in [8, 32] {
        for motion in [false, true] {
            let mut values = BTreeMap::from([("amp.sustain".into(), 0.7)]);
            if motion {
                values.extend(
                    [
                        ("oscA.unison", 7.),
                        ("oscB.unison", 3.),
                        ("osc.mix", 0.2),
                        ("oscA.morphTo", 4.),
                        ("oscA.position", 0.3),
                        ("oscA.phaseSpread", 0.7),
                        ("oscB.morphTo", 5.),
                        ("oscB.position", 0.2),
                        ("sub.level", 0.1),
                        ("noise.level", 0.01),
                        ("lfo.rateHz", 4.7),
                        ("lfo.pitch", 0.1),
                        ("lfo.cutoff", 12.),
                        ("lfo.positionA", 0.2),
                        ("lfo.positionB", -0.1),
                        ("lfo.level", 0.2),
                    ]
                    .map(|(k, v)| (k.into(), v)),
                );
            }
            let mut synth = WavetableSynthPlugin
                .create_configured(&host, &InstrumentConfig::new(&values))
                .unwrap();
            let events = (0..voices)
                .map(|i| NoteEvent {
                    frame_offset: 17,
                    kind: NoteEventKind::NoteOn,
                    pitch: 48 + i,
                    velocity: 0.6,
                })
                .collect::<Vec<_>>();
            let (mut left, mut right) = ([0.; 128], [0.; 128]);
            synth.process(&mut ProcessContext {
                frames: 128,
                sample_rate: 48000.,
                inputs: &[],
                outputs: &mut [&mut left, &mut right],
                note_events: &events,
                parameter_events: &[],
                sidechain: None,
            });
            group.bench_function(
                format!(
                    "{}_{}_voices_128",
                    if motion { "full" } else { "default" },
                    voices
                ),
                |b| {
                    b.iter(|| {
                        synth.process(&mut ProcessContext {
                            frames: 128,
                            sample_rate: 48000.,
                            inputs: &[],
                            outputs: &mut [&mut left, &mut right],
                            note_events: &[],
                            parameter_events: &[],
                            sidechain: None,
                        });
                        black_box((&left, &right));
                    });
                },
            );
            assert!(left.iter().any(|v| v.abs() > 0.001));
        }
    }
    group.finish();
}
criterion_group!(benches, benchmark);
criterion_main!(benches);
