#[path = "common/allocations.rs"]
mod allocations;
mod common;
#[path = "common/plugins.rs"]
mod fixture;

use oxitone_core::{codes, wire::AllowPlugins};
use oxitone_graph::{abi_c::OxiPluginEntryV1, ParameterEvent, ProcessContext};
use oxitone_render::plugins::{from_entry, load_plugin};

#[test]
fn entry_prefix_lifecycle_and_minor_compatibility() {
    let library = unsafe { libloading::Library::new(fixture::c_library()) }.unwrap();
    let symbol = unsafe {
        library
            .get::<unsafe extern "C" fn() -> *const OxiPluginEntryV1>(b"oxitone_plugin_entry_v1\0")
    }
    .unwrap();
    let original = unsafe { *symbol() };
    let manifest = fixture::manifest();
    let check = |entry: *const OxiPluginEntryV1, code| {
        assert_eq!(
            unsafe { from_entry(entry, &manifest, String::new(), None) }
                .err()
                .unwrap()
                .code,
            code
        );
    };
    check(std::ptr::null(), codes::PLUGIN_MANIFEST_MISMATCH);
    let mut entry = original;
    entry.struct_size = 12;
    check(&entry, codes::PLUGIN_ABI_MISMATCH);
    entry = original;
    entry.process = None;
    check(&entry, codes::PLUGIN_MANIFEST_MISMATCH);
    entry = original;
    entry.descriptor = std::ptr::null();
    check(&entry, codes::PLUGIN_MANIFEST_MISMATCH);
    entry = original;
    let mut descriptor = unsafe { *original.descriptor };
    descriptor.capabilities = 8;
    entry.descriptor = &descriptor;
    check(&entry, codes::PLUGIN_MANIFEST_MISMATCH);
    descriptor = unsafe { *original.descriptor };
    descriptor.abi_minor = 1;
    entry.abi_minor = 1;
    entry.descriptor = &descriptor;
    let mut future = manifest;
    future.abi_minor = 1;
    // Minor-compatible records preserve the prefix and use minHostVersion.
    assert!(unsafe { from_entry(&entry, &future, String::new(), None) }.is_ok());
}

#[test]
fn missing_symbol_is_an_abi_error() {
    let path = fixture::build_c(
        "missing-symbol",
        &["oxitone_plugin_entry_v1=different_entry"],
        false,
    );
    let error = unsafe { load_plugin(&fixture::options(&path), AllowPlugins::Any) }
        .err()
        .unwrap();
    assert_eq!(error.code, codes::PLUGIN_ABI_MISMATCH);
}

#[test]
fn adapter_is_allocation_free_and_guards_short_host_buffers() {
    let plugin = fixture::load(&fixture::options(fixture::c_library()));
    let mut instance = fixture::instance(&plugin);
    let input = [0.5; 8];
    let mut left = [0.; 8];
    let mut right = [0.; 8];
    let parameters = [ParameterEvent {
        frame_offset: 0,
        parameter_id: "gain",
        value: 0.5,
    }];
    let (allocated, freed) = allocations::count(|| {
        for _ in 0..1000 {
            instance.process(&mut ProcessContext {
                frames: 8,
                sample_rate: 48000.,
                inputs: &[&input, &input],
                outputs: &mut [&mut left, &mut right],
                note_events: &[],
                parameter_events: &parameters,
                sidechain: None,
            });
            instance.reset();
        }
    });
    assert_eq!((allocated, freed), (0, 0));
    assert_eq!(left, [0.25; 8]);
    instance.process(&mut ProcessContext {
        frames: 8,
        sample_rate: 48000.,
        inputs: &[&input, &input],
        outputs: &mut [&mut left[..4], &mut right],
        note_events: &[],
        parameter_events: &[],
        sidechain: None,
    });
    assert_eq!(left[..4], [0.; 4]);
    assert_eq!(right, [0.; 8]);
    assert_eq!(plugin.fault_count(), 1);
}
