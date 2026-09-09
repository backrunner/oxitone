//! Mixer engine tests: send pre/post semantics, masterSendRatio, mute/solo,
//! sidechain detector routing, topological determinism and sample-level PDC
//! alignment.

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_core::wire::{EffectRef, MixerChannelSpec, SendSpec};
use oxitone_graph::registry::PluginRegistry;
use oxitone_mixer::{builtin_effect_plugins, ChannelInput, MixerEngine};

const SR: f64 = 48_000.0;
const BLOCK: usize = 128;

fn registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    for plugin in builtin_effect_plugins() {
        registry.register(Arc::from(plugin)).unwrap();
    }
    registry
}

#[test]
fn compressor_uses_self_detector_unless_an_external_route_exists() {
    let mut destination = plain("mix_bus");
    destination.inserts.push(effect(
        "oxitone.compressor",
        &[("thresholdDb", -30.), ("ratio", 10.), ("attackMs", 1.)],
    ));
    let mut own =
        MixerEngine::build(SR, BLOCK as u32, &[destination.clone()], &registry()).unwrap();
    let compressed = render(&mut own, 120, &|_| vec![("mix_bus", 0.5)]);
    let mut detector = plain("mix_detector");
    detector.master_send_ratio = Some(0.);
    detector.sends.push(send("mix_bus", 1., true, true));
    let mut external =
        MixerEngine::build(SR, BLOCK as u32, &[destination, detector], &registry()).unwrap();
    let untouched = render(&mut external, 120, &|_| vec![("mix_bus", 0.5)]);
    let energy = |x: &[f32]| x[x.len() / 2..].iter().map(|v| v * v).sum::<f32>();
    assert!(energy(&compressed) < energy(&untouched) * 0.1);
}

fn effect(plugin_id: &str, parameters: &[(&str, f64)]) -> EffectRef {
    EffectRef {
        instance_id: None,
        plugin_id: plugin_id.to_string(),
        plugin_version: "1.0.0".to_string(),
        parameters: parameters
            .iter()
            .map(|(id, v)| (id.to_string(), *v))
            .collect::<BTreeMap<_, _>>(),
        resources: None,
        bypass: None,
        mix: None,
    }
}

struct BusSpec {
    id: &'static str,
    level: Option<f64>,
    sends: Vec<SendSpec>,
    inserts: Vec<EffectRef>,
    master_send_ratio: Option<f64>,
    mute: Option<bool>,
    solo: Option<bool>,
}

fn bus(spec: BusSpec) -> MixerChannelSpec {
    MixerChannelSpec {
        id: spec.id.to_string(),
        name: None,
        level: spec.level.unwrap_or(1.0),
        balance: 0.0,
        master_send_ratio: spec.master_send_ratio,
        inserts: spec.inserts,
        sends: spec.sends,
        mute: spec.mute,
        solo: spec.solo,
    }
}

fn plain(id: &'static str) -> MixerChannelSpec {
    bus(BusSpec {
        id,
        level: None,
        sends: vec![],
        inserts: vec![],
        master_send_ratio: Some(1.0),
        mute: None,
        solo: None,
    })
}

fn send(destination: &str, ratio: f64, pre_fader: bool, sidechain: bool) -> SendSpec {
    SendSpec {
        destination_id: destination.to_string(),
        ratio,
        pre_fader: Some(pre_fader),
        sidechain: Some(sidechain),
    }
}

fn sine_block(amp: f32, offset: usize) -> (Vec<f32>, Vec<f32>) {
    let left: Vec<f32> = (0..BLOCK)
        .map(|i| {
            amp * ((offset + i) as f32 * 1000.0 * 2.0 * core::f32::consts::PI / SR as f32).sin()
        })
        .collect();
    (left.clone(), left)
}

/// Render `blocks` blocks; `inputs_for` produces the channel inputs of each
/// block. Returns the concatenated master left output.
fn render(
    engine: &mut MixerEngine,
    blocks: usize,
    inputs_for: &dyn Fn(usize) -> Vec<(&'static str, f32)>,
) -> Vec<f32> {
    let mut rendered = Vec::new();
    for block in 0..blocks {
        let mut out_l = vec![0.0f32; BLOCK];
        let mut out_r = vec![0.0f32; BLOCK];
        let owned: Vec<(Vec<f32>, Vec<f32>, &'static str)> = inputs_for(block)
            .into_iter()
            .map(|(id, amp)| {
                let (l, r) = sine_block(amp, block * BLOCK);
                (l, r, id)
            })
            .collect();
        let inputs: Vec<ChannelInput<'_>> = owned
            .iter()
            .map(|(l, r, id)| ChannelInput {
                bus_id: id,
                left: l,
                right: r,
            })
            .collect();
        engine.process_block(&inputs, &mut out_l, &mut out_r);
        rendered.extend_from_slice(&out_l);
    }
    rendered
}

fn rms(buf: &[f32]) -> f64 {
    (buf.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / buf.len().max(1) as f64).sqrt()
}

#[test]
fn pre_fader_send_independent_of_level_post_fader_follows_it() {
    let registry = registry();
    // a --pre--> b --master; a.level = 0. Pre send still passes signal.
    let mut pre_engine = MixerEngine::build(
        SR,
        BLOCK as u32,
        &[
            bus(BusSpec {
                id: "a",
                level: Some(0.0),
                sends: vec![send("b", 1.0, true, false)],
                inserts: vec![],
                master_send_ratio: Some(0.0),
                mute: None,
                solo: None,
            }),
            plain("b"),
        ],
        &registry,
    )
    .unwrap();
    let pre = render(&mut pre_engine, 8, &|_| vec![("a", 0.5)]);
    let pre_rms = rms(&pre[BLOCK * 4..]);

    // Same routing, post-fader: level 0 silences the send.
    let mut post_engine = MixerEngine::build(
        SR,
        BLOCK as u32,
        &[
            bus(BusSpec {
                id: "a",
                level: Some(0.0),
                sends: vec![send("b", 1.0, false, false)],
                inserts: vec![],
                master_send_ratio: Some(0.0),
                mute: None,
                solo: None,
            }),
            plain("b"),
        ],
        &registry,
    )
    .unwrap();
    let post = render(&mut post_engine, 8, &|_| vec![("a", 0.5)]);
    let post_rms = rms(&post[BLOCK * 4..]);

    assert!(pre_rms > 0.05, "pre-fader send passes: {pre_rms}");
    assert!(post_rms < 1e-6, "post-fader send silenced: {post_rms}");
}

#[test]
fn master_send_ratio_scales_master_contribution() {
    let registry = registry();
    let mut engine = MixerEngine::build(SR, BLOCK as u32, &[plain("a")], &registry).unwrap();
    let full = render(&mut engine, 4, &|_| vec![("a", 0.5)]);
    engine.set_master_send_ratio("a", 0.5).unwrap();
    let half = render(&mut engine, 4, &|_| vec![("a", 0.5)]);
    engine.set_master_send_ratio("a", 0.0).unwrap();
    let none = render(&mut engine, 4, &|_| vec![("a", 0.5)]);
    let full_rms = rms(&full);
    let half_rms = rms(&half);
    // 0.5 amplitude through two equal-power (0.707) fader stages.
    assert!(full_rms > 0.15, "full {full_rms}");
    assert!(
        (half_rms - full_rms * 0.5).abs() < 0.02,
        "half {half_rms} vs full {full_rms}"
    );
    assert!(none.iter().all(|&x| x == 0.0), "ratio 0 is silent");
}

#[test]
fn mute_silences_and_solo_is_a_monitoring_policy() {
    let registry = registry();
    let specs = || vec![plain("a"), plain("b")];
    // Mute a → only b.
    let mut engine = MixerEngine::build(SR, BLOCK as u32, &specs(), &registry).unwrap();
    engine.set_mute("a", true).unwrap();
    let muted = render(&mut engine, 4, &|_| vec![("a", 0.5), ("b", 0.25)]);
    // Solo a with respect_solo on → b is silenced.
    let mut solo_engine = MixerEngine::build(SR, BLOCK as u32, &specs(), &registry).unwrap();
    solo_engine.set_respect_solo(true);
    solo_engine.set_solo("a", true).unwrap();
    let soloed = render(&mut solo_engine, 4, &|_| vec![("a", 0.5), ("b", 0.25)]);
    // respect_solo off (export default): both pass.
    let mut export_engine = MixerEngine::build(SR, BLOCK as u32, &specs(), &registry).unwrap();
    export_engine.set_solo("a", true).unwrap();
    let exported = render(&mut export_engine, 4, &|_| vec![("a", 0.5), ("b", 0.25)]);

    let block = &muted[BLOCK..BLOCK * 2];
    // a is 1 kHz; b is the same 1 kHz here, so compare levels instead:
    // muted output RMS equals b-only RMS, soloed equals a-only RMS.
    let muted_rms = rms(block);
    let soloed_rms = rms(&soloed[BLOCK..BLOCK * 2]);
    let exported_rms = rms(&exported[BLOCK..BLOCK * 2]);
    // Equal-power balance applies 0.707 at the channel fader and again at
    // the master fader: effective amplitude factor 0.5.
    let b_only = 0.25 * 0.5 / 2f64.sqrt();
    let a_only = 0.5 * 0.5 / 2f64.sqrt();
    assert!((muted_rms - b_only).abs() < 0.02, "muted {muted_rms}");
    assert!((soloed_rms - a_only).abs() < 0.02, "soloed {soloed_rms}");
    assert!(
        exported_rms > a_only,
        "export ignores solo by default: {exported_rms}"
    );
}

#[test]
fn sidechain_send_drives_detector_not_audio_sum() {
    let registry = registry();
    let specs = vec![
        bus(BusSpec {
            id: "key",
            level: None,
            sends: vec![send("b", 1.0, false, true)],
            inserts: vec![],
            master_send_ratio: Some(0.0),
            mute: None,
            solo: None,
        }),
        bus(BusSpec {
            id: "b",
            level: None,
            sends: vec![],
            inserts: vec![effect(
                "oxitone.compressor",
                &[
                    ("thresholdDb", -40.0),
                    ("ratio", 20.0),
                    ("attackMs", 0.1),
                    ("releaseMs", 50.0),
                ],
            )],
            master_send_ratio: Some(1.0),
            mute: None,
            solo: None,
        }),
    ];
    // key is loud, b is quiet: b must be ducked, and key audio must never
    // reach the master sum (its masterSendRatio is 0).
    let mut ducked = MixerEngine::build(SR, BLOCK as u32, &specs, &registry).unwrap();
    let with_key = render(&mut ducked, 16, &|_| vec![("key", 1.0), ("b", 0.25)]);
    let mut open = MixerEngine::build(SR, BLOCK as u32, &specs, &registry).unwrap();
    let without_key = render(&mut open, 16, &|_| vec![("b", 0.25)]);
    let tail = BLOCK * 8;
    let ducked_rms = rms(&with_key[with_key.len() - tail..]);
    let open_rms = rms(&without_key[without_key.len() - tail..]);
    assert!(
        ducked_rms < open_rms * 0.7,
        "sidechain ducks b: {ducked_rms} vs {open_rms}"
    );
    // key audio (amplitude 1.0) is absent: output stays around b's level.
    assert!(
        ducked_rms < 0.2,
        "sidechain audio must not enter the sum: {ducked_rms}"
    );
}

#[test]
fn pdc_aligns_parallel_paths_sample_exactly() {
    let registry = registry();
    // a has a clipper insert (24 frames latency); both a and b send to c.
    let make = || {
        MixerEngine::build(
            SR,
            BLOCK as u32,
            &[
                bus(BusSpec {
                    id: "a",
                    level: None,
                    sends: vec![send("c", 1.0, false, false)],
                    inserts: vec![effect("oxitone.clipper", &[("driveDb", 0.0)])],
                    master_send_ratio: Some(0.0),
                    mute: None,
                    solo: None,
                }),
                bus(BusSpec {
                    id: "b",
                    level: None,
                    sends: vec![send("c", 1.0, false, false)],
                    inserts: vec![],
                    master_send_ratio: Some(0.0),
                    mute: None,
                    solo: None,
                }),
                plain("c"),
            ],
            &registry,
        )
        .unwrap()
    };
    let engine = make();
    assert_eq!(engine.graph_latency_frames(), 24);
    let plan = engine.pdc_plan().clone();
    assert_eq!(
        plan.edge_delays[&("b".to_string(), "c".to_string(), false)],
        24
    );
    assert_eq!(
        plan.edge_delays[&("a".to_string(), "c".to_string(), false)],
        0
    );

    // Impulse through each path alone: the peaks must land on the same
    // absolute frame.
    let impulse = |engine: &mut MixerEngine, on_a: bool| {
        let mut out_l = vec![0.0f32; BLOCK];
        let mut out_r = vec![0.0f32; BLOCK];
        let in_l = {
            let mut v = vec![0.0f32; BLOCK];
            v[0] = 1.0;
            v
        };
        let (a_l, a_r, b_l, b_r) = if on_a {
            (
                in_l.clone(),
                in_l.clone(),
                vec![0.0; BLOCK],
                vec![0.0; BLOCK],
            )
        } else {
            (
                vec![0.0; BLOCK],
                vec![0.0; BLOCK],
                in_l.clone(),
                in_l.clone(),
            )
        };
        let inputs = [
            ChannelInput {
                bus_id: "a",
                left: &a_l,
                right: &a_r,
            },
            ChannelInput {
                bus_id: "b",
                left: &b_l,
                right: &b_r,
            },
        ];
        engine.process_block(&inputs, &mut out_l, &mut out_r);
        out_l
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.abs().partial_cmp(&y.1.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap()
    };
    let mut engine_a = make();
    let peak_a = impulse(&mut engine_a, true);
    let mut engine_b = make();
    let peak_b = impulse(&mut engine_b, false);
    assert_eq!(peak_a, 24, "clipper path impulse lands at 24");
    assert_eq!(peak_a, peak_b, "compensated path lands on the same frame");
}

#[test]
fn topology_order_and_output_are_deterministic() {
    let registry = registry();
    // Declaration order must not change processing order or output.
    let forward = vec![
        bus(BusSpec {
            id: "z_src",
            level: None,
            sends: vec![send("a_dst", 0.5, false, false)],
            inserts: vec![],
            master_send_ratio: Some(1.0),
            mute: None,
            solo: None,
        }),
        plain("a_dst"),
        plain("m_mid"),
    ];
    let mut shuffled = forward.clone();
    shuffled.reverse();
    let mut engine_f = MixerEngine::build(SR, BLOCK as u32, &forward, &registry).unwrap();
    let mut engine_s = MixerEngine::build(SR, BLOCK as u32, &shuffled, &registry).unwrap();
    assert_eq!(engine_f.bus_order(), engine_s.bus_order());
    let input_for = |_: usize| vec![("z_src", 0.5), ("m_mid", 0.25)];
    let out_f = render(&mut engine_f, 8, &input_for);
    let out_s = render(&mut engine_s, 8, &input_for);
    assert_eq!(out_f.len(), out_s.len());
    for (i, (a, b)) in out_f.iter().zip(out_s.iter()).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "bitwise determinism at {i}");
    }
    // And a second run of the same engine after reset is identical.
    engine_f.reset();
    let out_f2 = render(&mut engine_f, 8, &input_for);
    for (i, (a, b)) in out_f.iter().zip(out_f2.iter()).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "reset determinism at {i}");
    }
}

#[test]
fn send_ratio_parameter_scales_send() {
    let registry = registry();
    let specs = || {
        vec![
            bus(BusSpec {
                id: "a",
                level: None,
                sends: vec![send("b", 1.0, false, false)],
                inserts: vec![],
                master_send_ratio: Some(0.0),
                mute: None,
                solo: None,
            }),
            plain("b"),
        ]
    };
    let mut full = MixerEngine::build(SR, BLOCK as u32, &specs(), &registry).unwrap();
    let full_out = render(&mut full, 4, &|_| vec![("a", 0.5)]);
    let mut quarter = MixerEngine::build(SR, BLOCK as u32, &specs(), &registry).unwrap();
    quarter.set_send_ratio("a", "b", 0.25).unwrap();
    let quarter_out = render(&mut quarter, 4, &|_| vec![("a", 0.5)]);
    let full_rms = rms(&full_out);
    let quarter_rms = rms(&quarter_out);
    assert!(
        (quarter_rms - full_rms * 0.25).abs() < 0.02,
        "quarter {quarter_rms} vs full {full_rms}"
    );
}
