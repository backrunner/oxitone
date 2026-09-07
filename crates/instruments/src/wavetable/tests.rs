//! WavetableSynth tests: descriptor validation, configured creation,
//! synthesis determinism, note/voice-mode semantics, stealing, and rendering
//! sanity (non-zero, finite RMS/peak).

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_graph::abi::{ParameterEvent, Plugin, PluginInstance};
use oxitone_graph::PluginRegistry;

use super::{parameter_specs, WavetableSynthInstance, WavetableSynthPlugin};
use crate::testutil::*;
use crate::{builtin_plugins, InstrumentConfig};

fn empty_params() -> BTreeMap<String, f64> {
    BTreeMap::new()
}

fn configured(params: &BTreeMap<String, f64>) -> WavetableSynthInstance {
    let plugin = WavetableSynthPlugin;
    let config = InstrumentConfig::new(params);
    let mut instance = plugin
        .create_configured(&host(), &config)
        .expect("valid config");
    instance.prepare(48_000.0, 128);
    instance
}

#[test]
fn descriptor_validates_and_registers() {
    super::descriptor().validate().expect("descriptor is valid");
    let mut registry = PluginRegistry::new();
    for plugin in builtin_plugins() {
        registry.register(plugin).expect("built-ins register");
    }
    assert_eq!(registry.len(), 4);
    assert!(registry.contains_id(crate::WAVETABLE_PLUGIN_ID));
    assert!(registry.contains_id(crate::SAMPLER_PLUGIN_ID));
    assert!(registry.contains_id(crate::SLICER_PLUGIN_ID));
    assert!(registry.contains_id(oxitone_graph::multisampler::PLUGIN_ID));
}

#[test]
fn plugin_abi_create_matches_configured_defaults() {
    let plugin = WavetableSynthPlugin;
    assert_eq!(plugin.descriptor().plugin_id, crate::WAVETABLE_PLUGIN_ID);
    assert_eq!(plugin.descriptor().plugin_version, "1.0.0");
    assert_eq!(plugin.descriptor().max_polyphony, Some(64));
    let mut a = plugin.create(&host());
    a.prepare(48_000.0, 128);
    let mut b = configured(&empty_params());
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 60, 0.9)],
        params: vec![],
    })
    .chain(silence(7))
    .collect();
    assert_eq!(run(a.as_mut(), &blocks), run(&mut b, &blocks));
}

#[test]
fn unknown_or_out_of_range_initial_parameter_is_rejected() {
    let plugin = WavetableSynthPlugin;
    let mut params = BTreeMap::new();
    params.insert("filter.cutoff".to_string(), 100_000.0);
    let config = InstrumentConfig::new(&params);
    let err = plugin.create_configured(&host(), &config).err().unwrap();
    assert_eq!(err.code, oxitone_core::codes::INVALID_PROJECT);

    let mut params = BTreeMap::new();
    params.insert("not.a.parameter".to_string(), 1.0);
    let config = InstrumentConfig::new(&params);
    let err = plugin.create_configured(&host(), &config).err().unwrap();
    assert_eq!(err.code, oxitone_core::codes::INVALID_PROJECT);

    let mut params = BTreeMap::new();
    params.insert("voiceMode".to_string(), 0.5);
    let config = InstrumentConfig::new(&params);
    assert!(plugin.create_configured(&host(), &config).is_err());
}

#[test]
fn synthesis_is_byte_deterministic() {
    let events: Vec<Block> = vec![
        Block {
            notes: vec![note_on(0, 60, 0.8), note_on(16, 64, 0.7)],
            params: vec![ParameterEvent {
                frame_offset: 8,
                parameter_id: "filter.cutoff",
                value: 800.0,
            }],
        },
        Block {
            notes: vec![note_off(64, 60)],
            params: vec![ParameterEvent {
                frame_offset: 0,
                parameter_id: "osc.mix",
                value: 0.7,
            }],
        },
    ]
    .into_iter()
    .chain(silence(30))
    .collect();
    let mut a = configured(&empty_params());
    let mut b = configured(&empty_params());
    let (al, ar) = run(&mut a, &events);
    let (bl, br) = run(&mut b, &events);
    assert_eq!(al, bl, "left channel must be byte-identical");
    assert_eq!(ar, br, "right channel must be byte-identical");
    assert!(rms(&al) > 1e-3, "expected audible output");
    assert!(al.iter().all(|x| x.is_finite()));
}

#[test]
fn note_on_renders_note_off_releases_to_silence() {
    let mut instance = configured(&empty_params());
    let (l, _) = run(
        &mut instance,
        &[Block {
            notes: vec![note_on(0, 60, 1.0)],
            params: vec![],
        }],
    );
    assert!(peak(&l) > 0.05, "attack transient should be audible");

    let mut sounding = silence(100);
    sounding[0].notes.push(note_off(0, 60));
    let (l, _) = run(&mut instance, &sounding);
    let tail = &l[90 * FRAMES..];
    assert!(
        rms(tail) < 1e-6,
        "release (0.2 s) must decay to silence, rms {}",
        rms(tail)
    );
    assert_eq!(instance.tail_frames(), 0);
}

#[test]
fn event_offsets_are_sample_accurate() {
    // Note-on at offset 64: the first 64 frames are exactly silent.
    let mut instance = configured(&empty_params());
    let (l, _) = run(
        &mut instance,
        &[Block {
            notes: vec![note_on(64, 60, 1.0)],
            params: vec![],
        }],
    );
    assert!(l[..64].iter().all(|&x| x == 0.0));
    assert!(peak(&l[64..]) > 0.0);
}

#[test]
fn polyphony_is_capped_at_64_and_steals() {
    let mut params = BTreeMap::new();
    params.insert("amp.release".to_string(), 8.0);
    let mut instance = configured(&params);
    let notes: Vec<_> = (0..70u8).map(|i| note_on(0, 20 + i, 0.5)).collect();
    let (l, _) = run(
        &mut instance,
        &[Block {
            notes,
            params: vec![],
        }],
    );
    assert_eq!(instance.active_voice_count(), 64);
    assert!(peak(&l) > 0.0, "stolen-in note still sounds");
}

#[test]
fn mono_and_legato_keep_a_single_voice() {
    for (mode, name) in [(1.0, "mono"), (2.0, "legato")] {
        let mut params = BTreeMap::new();
        params.insert("voiceMode".to_string(), mode);
        params.insert("glide".to_string(), 0.1);
        let mut instance = configured(&params);
        let (l, _) = run(
            &mut instance,
            &[Block {
                notes: vec![note_on(0, 60, 0.8), note_on(64, 67, 0.8)],
                params: vec![],
            }],
        );
        assert_eq!(instance.active_voice_count(), 1, "{name} keeps one voice");
        assert!(peak(&l) > 0.0, "{name} voice sounds");
    }
}

#[test]
fn legato_glide_bends_pitch_continuously() {
    // With glide, the transition between two notes must not restart the
    // waveform: the instantaneous frequency during the glide lies between
    // the two note frequencies (130.8 Hz → 261.6 Hz).
    let mut params = BTreeMap::new();
    params.insert("voiceMode".to_string(), 2.0);
    params.insert("glide".to_string(), 0.05);
    params.insert("oscA.wavetable".to_string(), 0.0); // sine
    let mut instance = configured(&params);
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 48, 1.0)],
        params: vec![],
    })
    .chain(silence(10))
    .chain(std::iter::once(Block {
        notes: vec![note_on(0, 60, 1.0)],
        params: vec![],
    }))
    .chain(silence(20))
    .collect();
    let (l, _) = run(&mut instance, &blocks);
    let glide_region = &l[11 * FRAMES..13 * FRAMES];
    let crossings = glide_region
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    let seconds = glide_region.len() as f64 / 48_000.0;
    let freq = crossings as f64 / seconds;
    assert!(
        (140.0..=270.0).contains(&freq),
        "glide frequency {freq} Hz must stay between the note frequencies"
    );
}

#[test]
fn unknown_runtime_parameter_event_is_ignored() {
    let mut instance = configured(&empty_params());
    let (l, _) = run(
        &mut instance,
        &[Block {
            notes: vec![note_on(0, 60, 1.0)],
            params: vec![ParameterEvent {
                frame_offset: 0,
                parameter_id: "ghost.param",
                value: 42.0,
            }],
        }],
    );
    assert!(peak(&l) > 0.0);
}

#[test]
fn filter_envelope_modulates_cutoff() {
    let base = empty_params();
    let mut with_env_params = BTreeMap::new();
    with_env_params.insert("filter.cutoff".to_string(), 400.0);
    with_env_params.insert("filterEnv.amount".to_string(), 36.0);
    with_env_params.insert("filterEnv.decay".to_string(), 0.05);
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 60, 1.0)],
        params: vec![],
    })
    .chain(silence(5))
    .collect();
    let mut a = configured(&base);
    let mut b = configured(&with_env_params);
    let (al, _) = run(&mut a, &blocks);
    let (bl, _) = run(&mut b, &blocks);
    assert_ne!(al, bl);
    assert!(bl.iter().all(|x| x.is_finite()) && rms(&bl) > 1e-3);
}

#[test]
fn reset_clears_voices() {
    let mut instance = configured(&empty_params());
    let _ = run(
        &mut instance,
        &[Block {
            notes: vec![note_on(0, 60, 1.0)],
            params: vec![],
        }],
    );
    assert!(instance.tail_frames() > 0);
    instance.reset();
    assert_eq!(instance.tail_frames(), 0);
    assert_eq!(instance.active_voice_count(), 0);
}

#[test]
fn specs_cover_the_full_descriptor() {
    let ids: Vec<&str> = parameter_specs().iter().map(|s| s.id.as_str()).collect();
    for expected in [
        "oscA.wavetable",
        "oscA.pitch",
        "oscA.unison",
        "oscA.detune",
        "oscA.spread",
        "oscB.wavetable",
        "osc.mix",
        "filter.type",
        "filter.cutoff",
        "filter.resonance",
        "filterEnv.amount",
        "amp.attack",
        "amp.decay",
        "amp.sustain",
        "amp.release",
        "voiceMode",
        "glide",
        "level",
        "pan",
    ] {
        assert!(ids.contains(&expected), "missing parameter {expected}");
    }
    let plugin: Arc<dyn Plugin> = Arc::new(WavetableSynthPlugin);
    assert_eq!(plugin.descriptor().parameters.len(), ids.len());
}
