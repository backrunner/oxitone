#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use oxitone_core::wire::{AllowPlugins, PluginManifest, RegisterPluginOptions};
use oxitone_graph::{HostContext, ParameterEvent, Plugin, PluginInstance, ProcessContext};
use oxitone_render::plugins::{load_plugin, CPlugin};

pub fn manifest() -> PluginManifest {
    serde_json::from_str(include_str!("../fixtures/gain.json")).unwrap()
}

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn build_c(name: &str, defines: &[&str], executable: bool) -> PathBuf {
    let dir = crate::common::out_dir(&format!("plugins-{}", std::process::id()));
    let path = dir.join(format!(
        "{name}{}",
        if executable {
            ""
        } else {
            std::env::consts::DLL_SUFFIX
        }
    ));
    let mut cmd = Command::new("cc");
    cmd.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
    if !executable {
        cmd.args(["-shared", "-fPIC"]);
    }
    for define in defines {
        cmd.arg(format!("-D{define}"));
    }
    let out = cmd
        .arg("-I")
        .arg(root().join("include"))
        .arg(root().join("crates/render/tests/fixtures/gain.c"))
        .arg("-o")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    path
}

pub fn c_library() -> &'static Path {
    static LIB: OnceLock<PathBuf> = OnceLock::new();
    LIB.get_or_init(|| build_c("gain", &[], false)).as_path()
}

pub fn options(path: &Path) -> RegisterPluginOptions {
    RegisterPluginOptions {
        library_path: path.display().to_string(),
        expected_hash: None,
        manifest: manifest(),
    }
}

pub fn load(options: &RegisterPluginOptions) -> std::sync::Arc<CPlugin> {
    // Fixtures obey the ABI and are compiled from this repository.
    unsafe { load_plugin(options, AllowPlugins::Any) }.unwrap()
}

pub fn instance(plugin: &CPlugin) -> Box<dyn PluginInstance> {
    let mut instance = plugin
        .try_create(&HostContext {
            sample_rate: 48000.0,
            max_block_size: 8,
        })
        .unwrap();
    instance.try_prepare(48000.0, 8).unwrap();
    instance
}

pub fn process(instance: &mut dyn PluginInstance, parameters: &[ParameterEvent<'_>]) -> Vec<f32> {
    let left = [1., 2., 3., 4., 5., 6., 7., 8.];
    let right = [8., 7., 6., 5., 4., 3., 2., 1.];
    let side = [0.25; 8];
    let inputs: [&[f32]; 2] = [&left, &right];
    let sidechain: [&[f32]; 2] = [&side, &side];
    let mut l = [0.; 8];
    let mut r = [0.; 8];
    instance.process(&mut ProcessContext {
        frames: 8,
        sample_rate: 48000.,
        inputs: &inputs,
        outputs: &mut [&mut l, &mut r],
        note_events: &[],
        parameter_events: parameters,
        sidechain: Some(&sidechain),
    });
    l.into_iter().chain(r).collect()
}
