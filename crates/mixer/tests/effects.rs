//! Descriptor validation, smoke, determinism and behavior tests for the 12
//! Phase 1 built-in effect plugins.

use oxitone_graph::abi::{HostContext, ParameterEvent, PluginInstance, ProcessContext};
use oxitone_mixer::builtin_effect_plugins;

const SR: f64 = 48_000.0;
const BLOCK: u32 = 128;

const EXPECTED: [(&str, u64); 26] = [
    ("oxitone.eq", 0),
    ("oxitone.limit", 264), // 5 ms lookahead (240) + 2x stage (24)
    ("oxitone.clipper", 24),
    ("oxitone.filter", 0),
    ("oxitone.phaser", 24),
    ("oxitone.reverb", 0),
    ("oxitone.compressor", 0),
    ("oxitone.delay", 0),
    ("oxitone.gate", 0),
    ("oxitone.chorus", 0),
    ("oxitone.saturator", 36),
    ("oxitone.utility", 0),
    ("oxitone.distortion", 36),
    ("oxitone.multiband", 0),
    ("oxitone.nonlinear-filter", 24),
    ("oxitone.compactor", 0),
    ("oxitone.multiband-dynamics", 0),
    ("oxitone.resonator", 0),
    ("oxitone.frequency-shifter", 128),
    ("oxitone.pitch-shifter", 1208),
    ("oxitone.flanger", 0),
    ("oxitone.convolver", 256),
    ("oxitone.bitcrush", 0),
    ("oxitone.tape", 264),
    ("oxitone.spreader", 0),
    ("oxitone.limiter", 276),
];

fn host() -> HostContext {
    HostContext {
        sample_rate: SR,
        max_block_size: BLOCK,
    }
}

/// Render `blocks` blocks of a 1 kHz sine (amplitude 0.5) through the
/// instance; returns the concatenated left output.
fn render(
    instance: &mut dyn PluginInstance,
    parameters: &[(&str, f64)],
    blocks: usize,
    sidechain_amp: Option<f32>,
) -> Vec<f32> {
    let frames = BLOCK as usize;
    let in_l: Vec<f32> = (0..frames)
        .map(|i| 0.5 * (i as f32 * 1000.0 * 2.0 * core::f32::consts::PI / SR as f32).sin())
        .collect();
    let in_r = in_l.clone();
    let sc_l: Vec<f32> = vec![sidechain_amp.unwrap_or(0.0); frames];
    let sc_r = sc_l.clone();
    let events: Vec<ParameterEvent<'_>> = parameters
        .iter()
        .map(|(id, value)| ParameterEvent {
            frame_offset: 0,
            parameter_id: id,
            value: *value,
        })
        .collect();
    let mut rendered = Vec::new();
    for block in 0..blocks {
        let mut out_l = vec![0.0f32; frames];
        let mut out_r = vec![0.0f32; frames];
        {
            let inputs: [&[f32]; 2] = [&in_l, &in_r];
            let sc: [&[f32]; 2] = [&sc_l, &sc_r];
            let mut outputs: [&mut [f32]; 2] = [&mut out_l, &mut out_r];
            let mut ctx = ProcessContext {
                frames,
                sample_rate: SR,
                inputs: &inputs,
                outputs: &mut outputs,
                note_events: &[],
                parameter_events: if block == 0 { &events } else { &[] },
                sidechain: sidechain_amp.map(|_| &sc[..]),
            };
            instance.process(&mut ctx);
        }
        rendered.extend_from_slice(&out_l);
    }
    rendered
}

#[test]
fn descriptors_are_valid_and_complete() {
    let plugins = builtin_effect_plugins();
    assert_eq!(plugins.len(), EXPECTED.len());
    let mut ids: Vec<&str> = plugins
        .iter()
        .map(|p| p.descriptor().plugin_id.as_str())
        .collect();
    ids.sort_unstable();
    let mut expected: Vec<&str> = EXPECTED.iter().map(|(id, _)| *id).collect();
    expected.sort_unstable();
    assert_eq!(ids, expected);
    for plugin in &plugins {
        let descriptor = plugin.descriptor();
        assert_eq!(descriptor.plugin_version, "1.0.0");
        descriptor
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", descriptor.plugin_id));
        assert!(
            !descriptor.parameters.is_empty(),
            "{}",
            descriptor.plugin_id
        );
        for spec in &descriptor.parameters {
            assert_eq!(
                spec.automation,
                Some(true),
                "{}.{}",
                descriptor.plugin_id,
                spec.id
            );
        }
        // Static across calls.
        assert!(std::ptr::eq(plugin.descriptor(), descriptor));
    }
    // Capability contracts.
    let compressor = plugins
        .iter()
        .find(|p| p.descriptor().plugin_id == "oxitone.compressor")
        .unwrap();
    assert!(compressor.descriptor().capabilities.sidechain_input);
    for tail_id in ["oxitone.reverb", "oxitone.delay"] {
        let plugin = plugins
            .iter()
            .find(|p| p.descriptor().plugin_id == tail_id)
            .unwrap();
        assert!(plugin.descriptor().capabilities.reports_tail, "{tail_id}");
    }
}

#[test]
fn every_effect_smoke_and_deterministic() {
    for plugin in builtin_effect_plugins() {
        let id = plugin.descriptor().plugin_id.as_str();
        let mut instance = plugin.create(&host());
        instance.prepare(SR, BLOCK);
        assert_eq!(
            instance.latency_frames(),
            { EXPECTED.iter().find(|(eid, _)| *eid == id).unwrap().1 },
            "{id} latency"
        );
        // Delay outputs 100% wet at a default 0.25 s time; shorten it so
        // the echo lands inside the render window.
        let params: &[(&str, f64)] = if id == "oxitone.delay" {
            &[("timeSeconds", 0.002)]
        } else {
            &[]
        };
        // Apply parameters, then snap smoothers via reset (control thread).
        render(&mut *instance, params, 1, None);
        instance.reset();
        let first = render(&mut *instance, &[], 32, None);
        assert!(first.iter().all(|x| x.is_finite()), "{id} finite");
        let energy: f64 = first.iter().map(|&x| (x as f64).powi(2)).sum();
        assert!(energy > 1e-6, "{id} non-silent (energy {energy})");
        // Byte-identical second run after reset.
        instance.reset();
        let second = render(&mut *instance, &[], 32, None);
        assert_eq!(first.len(), second.len());
        for (i, (a, b)) in first.iter().zip(second.iter()).enumerate() {
            assert_eq!(a.to_bits(), b.to_bits(), "{id} determinism at {i}");
        }
    }
}

#[test]
fn compressor_sidechain_drives_gain_reduction() {
    let plugin = builtin_effect_plugins()
        .into_iter()
        .find(|p| p.descriptor().plugin_id == "oxitone.compressor")
        .unwrap();
    let params: [(&str, f64); 4] = [
        ("thresholdDb", -40.0),
        ("ratio", 10.0),
        ("attackMs", 1.0),
        ("releaseMs", 50.0),
    ];
    // Without sidechain the 0.5-amplitude main input sits above threshold
    // only moderately; with a hot sidechain the reduction is much deeper.
    let mut plain = plugin.create(&host());
    plain.prepare(SR, BLOCK);
    let without = render(&mut *plain, &params, 12, None);
    let mut ducked = plugin.create(&host());
    ducked.prepare(SR, BLOCK);
    let with = render(&mut *ducked, &params, 12, Some(1.0));
    let rms = |buf: &[f32]| (buf.iter().map(|&x| (x as f64).powi(2)).sum::<f64>()).sqrt();
    let tail = BLOCK as usize * 4;
    let without_rms = rms(&without[without.len() - tail..]);
    let with_rms = rms(&with[with.len() - tail..]);
    assert!(
        with_rms < without_rms * 0.8,
        "sidechain should duck harder: {with_rms} vs {without_rms}"
    );
}

#[test]
fn reverb_reports_and_decays_tail() {
    let plugin = builtin_effect_plugins()
        .into_iter()
        .find(|p| p.descriptor().plugin_id == "oxitone.reverb")
        .unwrap();
    let mut instance = plugin.create(&host());
    instance.prepare(SR, BLOCK);
    let out = render(
        &mut *instance,
        &[("decaySeconds", 0.5), ("predelayMs", 0.0)],
        16,
        None,
    );
    let tail_after_input = instance.tail_frames();
    assert!(tail_after_input > 0, "tail reported after input");
    // Tail must be audible after the input region.
    let tail_energy: f64 = out[BLOCK as usize * 2..]
        .iter()
        .map(|&x| (x as f64).powi(2))
        .sum();
    assert!(tail_energy > 0.0);
    assert!(tail_energy > 1e-8, "reverb tail audible: {tail_energy}");
    // Long silence exhausts the tail.
    let frames = BLOCK as usize;
    let silence = vec![0.0f32; frames];
    let mut out_l = vec![0.0f32; frames];
    let mut out_r = vec![0.0f32; frames];
    let mut exhausted = tail_after_input;
    for _ in 0..400 {
        let inputs: [&[f32]; 2] = [&silence, &silence];
        let mut outputs: [&mut [f32]; 2] = [&mut out_l, &mut out_r];
        let mut ctx = ProcessContext {
            frames,
            sample_rate: SR,
            inputs: &inputs,
            outputs: &mut outputs,
            note_events: &[],
            parameter_events: &[],
            sidechain: None,
        };
        instance.process(&mut ctx);
        exhausted = instance.tail_frames();
        if exhausted == 0 {
            break;
        }
    }
    assert_eq!(exhausted, 0, "tail decays to zero");
    // Reverb output is finite and decays in level over time.
    let late_energy: f64 = out_l.iter().map(|&x| (x as f64).powi(2)).sum();
    assert!(late_energy < tail_energy, "tail decays: {late_energy}");
}

#[test]
fn delay_time_seconds_parameter_moves_the_echo() {
    let plugin = builtin_effect_plugins()
        .into_iter()
        .find(|p| p.descriptor().plugin_id == "oxitone.delay")
        .unwrap();
    let mut instance = plugin.create(&host());
    instance.prepare(SR, BLOCK);
    let frames = BLOCK as usize;
    let silence = vec![0.0f32; frames];
    let mut out_l = vec![0.0f32; frames];
    let mut out_r = vec![0.0f32; frames];
    let events = [ParameterEvent {
        frame_offset: 0,
        parameter_id: "timeSeconds",
        value: 0.01,
    }];
    // Settle the 250 ms smoother for >10 time constants. Stereo advances it
    // once per frame, so one second still leaves a measurable timing offset.
    for block in 0..1000 {
        let inputs: [&[f32]; 2] = [&silence, &silence];
        let mut outputs: [&mut [f32]; 2] = [&mut out_l, &mut out_r];
        let mut ctx = ProcessContext {
            frames,
            sample_rate: SR,
            inputs: &inputs,
            outputs: &mut outputs,
            note_events: &[],
            parameter_events: if block == 0 { &events[..] } else { &[] },
            sidechain: None,
        };
        instance.process(&mut ctx);
    }
    // Impulse: the echo must land at 0.01 s = 480 frames later.
    let mut impulse = vec![0.0f32; frames * 8];
    impulse[0] = 1.0;
    let mut echo_l = vec![0.0f32; frames * 8];
    let mut echo_r = vec![0.0f32; frames * 8];
    {
        let inputs: [&[f32]; 2] = [&impulse, &impulse];
        let mut outputs: [&mut [f32]; 2] = [&mut echo_l, &mut echo_r];
        let mut ctx = ProcessContext {
            frames: frames * 8,
            sample_rate: SR,
            inputs: &inputs,
            outputs: &mut outputs,
            note_events: &[],
            parameter_events: &[],
            sidechain: None,
        };
        instance.process(&mut ctx);
    }
    let peak = echo_l
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    assert!(
        (440..=520).contains(&peak),
        "echo lands near frame 480: {peak}"
    );
}

#[test]
fn gate_closes_below_threshold() {
    let plugin = builtin_effect_plugins()
        .into_iter()
        .find(|p| p.descriptor().plugin_id == "oxitone.gate")
        .unwrap();
    let mut instance = plugin.create(&host());
    instance.prepare(SR, BLOCK);
    // Threshold 0 dB: nothing passes, output converges to silence.
    let out = render(&mut *instance, &[("thresholdDb", 0.0)], 20, None);
    let tail = &out[out.len() - BLOCK as usize..];
    let peak = tail.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
    assert!(peak < 1e-3, "gate closed: peak {peak}");
}
