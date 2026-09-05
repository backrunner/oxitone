mod common;
#[path = "common/plugins.rs"]
mod fixture;

use oxitone_core::{codes, wire::AllowPlugins};
use oxitone_graph::{
    HostContext, NoteEvent, NoteEventKind, ParameterEvent, Plugin, ProcessContext,
};
use oxitone_render::plugins::{library_hash, load_plugin};
use std::process::Command;

#[test]
fn c_static_and_dynamic_samples_match_and_instances_retain_the_library() {
    let path = fixture::c_library();
    let mut opts = fixture::options(path);
    opts.expected_hash = Some(library_hash(path).unwrap());
    let plugin = fixture::load(&opts);
    let mut instance = fixture::instance(&plugin);
    drop(plugin);
    let events = [
        ParameterEvent {
            frame_offset: 0,
            parameter_id: "gain",
            value: 0.5,
        },
        ParameterEvent {
            frame_offset: 3,
            parameter_id: "gain",
            value: 0.25,
        },
        ParameterEvent {
            frame_offset: 3,
            parameter_id: "gain",
            value: 0.75,
        },
    ];
    let dynamic = fixture::process(&mut *instance, &events);
    let exe = fixture::build_c("static-gain", &["FIXTURE_RUNNER"], true);
    let output = Command::new(exe).output().unwrap();
    assert!(output.status.success());
    let expected: Vec<_> = output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_ne_bytes(bytes.try_into().unwrap()))
        .collect();
    assert_eq!(dynamic, expected);
    assert!(dynamic.iter().any(|&v| v != 0.0));
    instance.reset();
    instance.try_prepare(44100., 8).unwrap();
    assert_eq!(fixture::process(&mut *instance, &events), dynamic);
}

#[test]
fn mono_adapter_and_faults_are_contained() {
    let path = fixture::build_c("mono", &["FIXTURE_LAYOUT=1"], false);
    let mut opts = fixture::options(&path);
    opts.manifest.input_layout = "mono".into();
    opts.manifest.output_layout = "mono".into();
    let plugin = fixture::load(&opts);
    let samples = fixture::process(&mut *fixture::instance(&plugin), &[]);
    assert_eq!(samples, vec![4.75; 16]);
    for fault in 1..=3 {
        let path = fixture::build_c(
            &format!("fault-{fault}"),
            &[&format!("FIXTURE_FAULT={fault}")],
            false,
        );
        let plugin = fixture::load(&fixture::options(&path));
        let mut instance = fixture::instance(&plugin);
        assert_eq!(fixture::process(&mut *instance, &[]), vec![0.; 16]);
        assert_eq!(fixture::process(&mut *instance, &[]), vec![0.; 16]);
        assert_eq!(plugin.fault_count(), 1, "fault latches until reset");
        instance.reset();
        fixture::process(&mut *instance, &[]);
        assert_eq!(plugin.fault_count(), 2);
    }
}

#[test]
fn loader_rejects_incompatible_metadata_and_failed_lifecycle() {
    let path = fixture::c_library();
    let check = |options: &_, code| {
        let result = unsafe { load_plugin(options, AllowPlugins::Any) };
        assert_eq!(result.err().unwrap().code, code);
    };
    let mut opts = fixture::options(path);
    opts.expected_hash = Some("0".repeat(64));
    check(&opts, codes::PLUGIN_MANIFEST_MISMATCH);
    opts = fixture::options(path);
    opts.manifest.parameters[0].max = 3.;
    check(&opts, codes::PLUGIN_MANIFEST_MISMATCH);
    opts = fixture::options(path);
    opts.manifest.min_host_version = "99.0.0".into();
    check(&opts, codes::PLUGIN_ABI_MISMATCH);
    opts.manifest.min_host_version = "invalid".into();
    check(&opts, codes::PLUGIN_MANIFEST_MISMATCH);
    let wrong_abi = fixture::build_c("abi", &["FIXTURE_ABI=2"], false);
    check(&fixture::options(&wrong_abi), codes::PLUGIN_ABI_MISMATCH);
    let plugin = fixture::load(&fixture::options(path));
    assert!(plugin
        .try_create(&HostContext {
            sample_rate: 48000.,
            max_block_size: 13
        })
        .is_err());
    let mut instance = fixture::instance(&plugin);
    assert_eq!(
        instance.try_prepare(48000., 13).unwrap_err().code,
        codes::REALTIME_FAULT
    );
    assert!(unsafe { load_plugin(&fixture::options(path), AllowPlugins::SignedOnly) }.is_err());
}

#[test]
fn rust_cdylib_notes_tail_and_reset() {
    let path = common::out_dir(&format!("plugins-{}", std::process::id()))
        .join(format!("rust{}", std::env::consts::DLL_SUFFIX));
    let output = Command::new("rustc")
        .args(["--edition=2021", "--crate-type=cdylib", "-O"])
        .arg(fixture::root().join("crates/render/tests/fixtures/instrument.rs"))
        .arg("-o")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut opts = fixture::options(&path);
    opts.manifest.plugin_id = "fixture.rust".into();
    opts.manifest.kind = "instrument".into();
    opts.manifest.input_layout = "none".into();
    opts.manifest.parameters.clear();
    opts.manifest.sidechain_input = false;
    opts.manifest.max_polyphony = Some(1);
    let plugin = fixture::load(&opts);
    let mut instance = fixture::instance(&plugin);
    let mut l = [0.; 8];
    let mut r = [0.; 8];
    let notes = [NoteEvent {
        frame_offset: 2,
        kind: NoteEventKind::NoteOn,
        pitch: 64,
        velocity: 0.5,
    }];
    instance.process(&mut ProcessContext {
        frames: 8,
        sample_rate: 48000.,
        inputs: &[],
        outputs: &mut [&mut l, &mut r],
        note_events: &notes,
        parameter_events: &[],
        sidechain: None,
    });
    assert_eq!(l, [0., 0., 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
    assert_eq!(r, l);
    assert_eq!(instance.tail_frames(), 17);
    instance.reset();
    assert_eq!(instance.tail_frames(), 0);
    instance.process(&mut ProcessContext {
        frames: 8,
        sample_rate: 48000.,
        inputs: &[],
        outputs: &mut [&mut l, &mut r],
        note_events: &[],
        parameter_events: &[],
        sidechain: None,
    });
    assert_eq!(l, [0.; 8]);
}
