//! Compile, resolve, and process a channel/bus/master insert automation graph.
mod common;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_core::wire::*;
use oxitone_render::{
    builtin_registry, resolve_parameter_event, ParamTargetIndex, ParameterEventInput, RenderGraph,
    RenderGraphOptions, SampleStore,
};

fn scenario() -> ProjectSnapshot {
    let mut s = common::typical_snapshot(1, 4, 0);
    s.automation.clear();
    let effect = EffectRef {
        instance_id: None,
        plugin_id: "oxitone.utility".into(),
        plugin_version: "1.0.0".into(),
        parameters: Default::default(),
        resources: None,
        mix: None,
        bypass: None,
    };
    s.channels[0].effect_chain = vec![effect.clone()];
    for bus in &mut s.mixer_channels {
        bus.inserts = vec![effect.clone()];
    }
    if !s.mixer_channels.iter().any(|b| b.id == "mix_master") {
        s.mixer_channels.push(MixerChannelSpec {
            id: "mix_master".into(),
            name: None,
            level: 1.,
            balance: 0.,
            master_send_ratio: None,
            inserts: vec![effect],
            sends: vec![],
            mute: None,
            solo: None,
        });
    }
    let owners: Vec<_> = std::iter::once(s.channels[0].id.clone())
        .chain(s.mixer_channels.iter().map(|b| b.id.clone()))
        .collect();
    for (i, owner) in owners.into_iter().enumerate() {
        s.automation.push(AutomationLaneSpec {
            playback: None,
            id: format!("auto_insert_{i}"),
            target: AutomationTarget {
                scope: None,
                entity_id: owner,
                parameter_id: "insert.0.parameter.polarity".into(),
            },
            source: AutomationSourceSpec::Gate {
                period_beats: common::beat(1, 100),
                phase: None,
                duty: 0.5,
                on: Some(1.),
                off: Some(0.),
            },
            combine: None,
            loop_spec: None,
            last_beat: None,
        });
    }
    s
}

fn bench(c: &mut Criterion) {
    let s = scenario();
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let options = RenderGraphOptions::default();
    let mut g = RenderGraph::compile(&s, &registry, &store, &options).unwrap();
    let index = ParamTargetIndex::from_graph(&g);
    let event = ParameterEventInput {
        entity_id: "mix_master".into(),
        parameter_id: "insert.0.parameter.polarity".into(),
        value: 1.,
        at_frame: None,
    };
    let mut group = c.benchmark_group("insert/automation");
    group.bench_function("compile", |b| {
        b.iter(|| RenderGraph::compile(black_box(&s), &registry, &store, &options).unwrap())
    });
    group.bench_function("resolve", |b| {
        b.iter(|| resolve_parameter_event(black_box(&index), &event, 0).unwrap())
    });
    let (mut l, mut r) = ([0.; 128], [0.; 128]);
    g.transport_mut().play_from(0, Some((0, 96_000)));
    group.bench_function("render_128", |b| {
        b.iter(|| {
            g.process_block(black_box(&mut l), black_box(&mut r));
            black_box(&l);
        })
    });
    group.finish();
    let mut typed = s.clone();
    typed.protocol_version = "1.1".into();
    for channel in &mut typed.channels {
        channel.effect_chain[0].instance_id = Some(format!("ins_{}", channel.id));
    }
    for bus in &mut typed.mixer_channels {
        bus.inserts[0].instance_id = Some(format!("ins_{}", bus.id));
    }
    for lane in &mut typed.automation {
        lane.target.scope = Some(ParameterScope::Plugin);
        lane.target.entity_id = format!("ins_{}", lane.target.entity_id);
        lane.target.parameter_id = "polarity".into();
    }
    let mut typed_graph = RenderGraph::compile(&typed, &registry, &store, &options).unwrap();
    typed_graph.transport_mut().play_from(0, Some((0, 96_000)));
    let mut group = c.benchmark_group("insert/typed_automation");
    group.bench_function("compile", |b| {
        b.iter(|| RenderGraph::compile(black_box(&typed), &registry, &store, &options).unwrap())
    });
    group.bench_function("render_128", |b| {
        b.iter(|| {
            typed_graph.process_block(black_box(&mut l), black_box(&mut r));
            black_box(&l);
        })
    });
    group.finish();
}
criterion_group!(benches, bench);
criterion_main!(benches);
