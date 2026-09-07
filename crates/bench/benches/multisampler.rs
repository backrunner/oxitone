//! 39 mapped regions / 32 voices. Assets and lookup tables are prepared outside timing.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_core::wire::SampleFormat;
use oxitone_graph::{HostContext, NoteEvent, NoteEventKind, PluginInstance, ProcessContext};
use oxitone_instruments::{multisampler::MultisamplerPlugin, InstrumentConfig, SampleProvider};
use oxitone_samples::{ChannelLayoutAction, PreparedSample, SampleMetadata};
use std::{collections::BTreeMap, sync::Arc};

struct Samples(Arc<PreparedSample>);
impl SampleProvider for Samples {
    fn prepared_sample(&self, _: &str) -> Option<Arc<PreparedSample>> {
        Some(self.0.clone())
    }
}
fn benchmark(c: &mut Criterion) {
    let signal = (0..48000)
        .map(|i| (std::f64::consts::TAU * 440. * i as f64 / 48000.).sin() as f32 * 0.05)
        .collect();
    let samples = Samples(Arc::new(PreparedSample {
        channels: vec![signal],
        sample_rate: 48000,
        loop_points: None,
        musical_length_beats: None,
        metadata: SampleMetadata {
            sha256: "00".repeat(32),
            format: SampleFormat::Wav,
            source_sample_rate: 48000,
            source_channels: 1,
            source_bit_depth: Some(32),
            channel_layout_action: ChannelLayoutAction::Kept,
            decoder: None,
        },
    }));
    let regions: Vec<_> = (0..13)
        .flat_map(|i| {
            (0..3).map(move |layer| {
                serde_json::json!({
                    "resource":"a", "rootKey":42+i*4, "keyRange":[40+i*4,43+i*4],
                    "velocityRange":([[1,50],[51,88],[89,127]][layer]), "gain":0.8
                })
            })
        })
        .collect();
    let state = serde_json::json!({"version":1,"regions":regions});
    let resources = BTreeMap::from([("a".into(), "smp_bench".into())]);
    let parameters = BTreeMap::from([("amp.attack".into(), 0.)]);
    let host = HostContext {
        sample_rate: 48000.,
        max_block_size: 128,
    };
    let mut instance = MultisamplerPlugin
        .create_configured(
            &host,
            &InstrumentConfig {
                parameters: &parameters,
                resources: Some(&resources),
                state: Some(&state),
            },
            &samples,
        )
        .unwrap();
    let events: Vec<_> = (0..32)
        .map(|i| NoteEvent {
            frame_offset: 0,
            kind: NoteEventKind::NoteOn,
            pitch: 48 + i,
            velocity: [0.3, 0.6, 0.9][i as usize % 3],
        })
        .collect();
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    let mut group = c.benchmark_group("instruments/multisampler_39_regions_32_voices");
    for reset_each_block in [true, false] {
        let mut block = 0;
        group.bench_function(
            if reset_each_block {
                "reset_first_128"
            } else {
                "sounding_128"
            },
            |b| {
                b.iter(|| {
                    let trigger = reset_each_block || block % 120 == 0;
                    if trigger {
                        instance.reset();
                    }
                    instance.process(&mut ProcessContext {
                        frames: 128,
                        sample_rate: 48000.,
                        inputs: &[],
                        outputs: &mut [&mut left, &mut right],
                        note_events: if trigger { &events } else { &[] },
                        parameter_events: &[],
                        sidechain: None,
                    });
                    black_box((&left, &right));
                    block += 1;
                })
            },
        );
    }
    assert!(left.iter().any(|v| v.abs() > 0.001));
    group.finish();
}
criterion_group!(benches, benchmark);
criterion_main!(benches);
