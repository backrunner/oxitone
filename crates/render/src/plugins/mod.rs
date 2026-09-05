//! Control-thread dynamic loading and owned C ABI instances.
mod instance;
mod validate;

use libloading::Library;
use oxitone_core::wire::{AllowPlugins, RegisterPluginOptions, RegisteredPlugin};
use oxitone_core::{codes, OxitoneError};
use oxitone_graph::abi_c::OxiPluginEntryV1;
use oxitone_graph::{HostContext, Plugin, PluginDescriptor, PluginInstance};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{atomic::AtomicU64, Arc};

pub struct CPlugin {
    shared: Arc<LoadedPlugin>,
    pub registration: RegisteredPlugin,
}

pub(super) struct LoadedPlugin {
    descriptor: PluginDescriptor,
    entry: OxiPluginEntryV1,
    faults: AtomicU64,
    // Every instance retains this owner until after dispose has run.
    _library: Option<Library>,
}

// SAFETY: ABI entry metadata is immutable, factories support independent
// instances on control threads, and each instance is used exclusively.
unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}

pub(super) fn fault(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::REALTIME_FAULT, message)
}

pub fn library_hash(path: &Path) -> Result<String, OxitoneError> {
    let mut file =
        File::open(path).map_err(|e| OxitoneError::new(codes::ASSET_UNAVAILABLE, e.to_string()))?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = file
            .read(&mut bytes)
            .map_err(|e| OxitoneError::new(codes::ASSET_UNAVAILABLE, e.to_string()))?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn verify_signature(path: &Path) -> Result<(), OxitoneError> {
    #[cfg(target_os = "macos")]
    {
        let result = std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict", "-R", "anchor apple generic"])
            .arg(path)
            .output();
        if result.is_ok_and(|r| r.status.success()) {
            return Ok(());
        }
    }
    let _ = path;
    Err(OxitoneError::new(
        codes::PLUGIN_MANIFEST_MISMATCH,
        "signed-only requires a valid Apple-issued code signature",
    ))
}

/// Load explicitly trusted native code. All I/O and validation run here.
///
/// # Safety
/// The library and its dependencies must obey the C ABI, including valid
/// pointers, exclusive instance access, realtime-safe process/reset and no
/// unwinding across C calls. Loading arbitrary native code cannot be sandboxed.
pub unsafe fn load_plugin(
    options: &RegisterPluginOptions,
    policy: AllowPlugins,
) -> Result<Arc<CPlugin>, OxitoneError> {
    validate::manifest_version(&options.manifest)?;
    let path = std::fs::canonicalize(&options.library_path)
        .map_err(|e| OxitoneError::new(codes::ASSET_UNAVAILABLE, e.to_string()))?;
    let sha256 = library_hash(&path)?;
    if let Some(expected) = &options.expected_hash {
        if expected.len() != 64 || !expected.eq_ignore_ascii_case(&sha256) {
            return Err(OxitoneError::new(
                codes::PLUGIN_MANIFEST_MISMATCH,
                "plugin SHA-256 mismatch",
            ));
        }
    }
    if policy == AllowPlugins::SignedOnly {
        verify_signature(&path)?;
    }
    // SAFETY: caller explicitly trusts this native module. Library lifetime
    // is transferred to LoadedPlugin, shared by all of its instances.
    let library = unsafe { Library::new(&path) }
        .map_err(|e| OxitoneError::new(codes::ASSET_UNAVAILABLE, e.to_string()))?;
    let entry = unsafe {
        let symbol = library
            .get::<unsafe extern "C" fn() -> *const OxiPluginEntryV1>(b"oxitone_plugin_entry_v1\0")
            .map_err(|e| OxitoneError::new(codes::PLUGIN_ABI_MISMATCH, e.to_string()))?;
        symbol()
    };
    unsafe { from_entry(entry, &options.manifest, sha256, Some(library)) }
}

/// Adapt a statically linked C entry with the same validation as dlopen.
/// This is intended for conformance fixtures and hosts that embed a C plugin.
///
/// # Safety
/// Entry and its function pointers must obey the ABI and remain valid for
/// every resulting instance. `library` must retain any dynamically loaded code.
pub unsafe fn from_entry(
    entry: *const OxiPluginEntryV1,
    manifest: &oxitone_core::wire::PluginManifest,
    sha256: String,
    library: Option<Library>,
) -> Result<Arc<CPlugin>, OxitoneError> {
    let (entry, descriptor) = unsafe { validate::entry(entry, manifest)? };
    Ok(Arc::new(CPlugin {
        registration: RegisteredPlugin {
            plugin_id: descriptor.plugin_id.clone(),
            plugin_version: descriptor.plugin_version.clone(),
            sha256,
        },
        shared: Arc::new(LoadedPlugin {
            descriptor,
            entry,
            faults: AtomicU64::new(0),
            _library: library,
        }),
    }))
}

impl CPlugin {
    pub fn fault_count(&self) -> u64 {
        self.shared
            .faults
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Plugin for CPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.shared.descriptor
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        self.try_create(host)
            .expect("use try_create to handle native plugin allocation failures")
    }
    fn try_create(&self, host: &HostContext) -> Result<Box<dyn PluginInstance>, OxitoneError> {
        Ok(Box::new(instance::CInstance::new(
            self.shared.clone(),
            host,
        )?))
    }
}
