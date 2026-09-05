use oxitone_core::wire::PluginManifest;
use oxitone_core::{codes, OxitoneError};
use oxitone_graph::abi_c::{cstr, descriptor_from_c, OxiPluginEntryV1, ABI_MAJOR};
use oxitone_graph::{ChannelLayout, PluginDescriptor, PluginKind};

fn mismatch(message: &str) -> OxitoneError {
    OxitoneError::new(codes::PLUGIN_MANIFEST_MISMATCH, message)
}

pub(super) fn manifest_version(manifest: &PluginManifest) -> Result<(), OxitoneError> {
    let host = semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("crate semver");
    let minimum = semver::Version::parse(&manifest.min_host_version)
        .map_err(|_| mismatch("invalid minHostVersion"))?;
    semver::Version::parse(&manifest.plugin_version)
        .map_err(|_| mismatch("invalid pluginVersion"))?;
    if minimum > host || manifest.abi_major != ABI_MAJOR {
        return Err(OxitoneError::new(
            codes::PLUGIN_ABI_MISMATCH,
            "plugin requires an unsupported ABI or host version",
        ));
    }
    Ok(())
}

pub(super) unsafe fn entry(
    ptr: *const OxiPluginEntryV1,
    manifest: &PluginManifest,
) -> Result<(OxiPluginEntryV1, PluginDescriptor), OxitoneError> {
    manifest_version(manifest)?;
    if ptr.is_null() || !ptr.is_aligned() {
        return Err(mismatch("invalid plugin entry pointer"));
    }
    // Read only the common prefix until version and record size are accepted.
    let major = unsafe { std::ptr::addr_of!((*ptr).abi_major).read() };
    let minor = unsafe { std::ptr::addr_of!((*ptr).abi_minor).read() };
    if major != ABI_MAJOR {
        return Err(OxitoneError::new(
            codes::PLUGIN_ABI_MISMATCH,
            "unsupported plugin entry ABI",
        ));
    }
    let size = unsafe { std::ptr::addr_of!((*ptr).struct_size).read() };
    if size < std::mem::size_of::<OxiPluginEntryV1>() as u32 {
        return Err(OxitoneError::new(
            codes::PLUGIN_ABI_MISMATCH,
            "truncated plugin entry",
        ));
    }
    let entry = unsafe { ptr.read() };
    if entry.descriptor.is_null() || !entry.descriptor.is_aligned() {
        return Err(mismatch("invalid descriptor pointer"));
    }
    if entry.create.is_none()
        || entry.dispose.is_none()
        || entry.prepare.is_none()
        || entry.process.is_none()
        || entry.reset.is_none()
        || entry.tail_frames.is_none()
        || entry.latency_frames.is_none()
    {
        return Err(mismatch("missing lifecycle function"));
    }
    let raw = unsafe { &*entry.descriptor };
    let descriptor = unsafe { descriptor_from_c(raw)? };
    let mut expected_parameters = manifest.parameters.clone();
    for parameter in &mut expected_parameters {
        parameter.automation = Some(parameter.automation.unwrap_or(false));
    }
    let minimum = unsafe { cstr(entry.min_host_version, "minHostVersion")? };
    let kind = match descriptor.kind {
        PluginKind::Instrument => "instrument",
        PluginKind::Effect => "effect",
    };
    let layout = |v| match v {
        ChannelLayout::None => "none",
        ChannelLayout::Mono => "mono",
        ChannelLayout::Stereo => "stereo",
    };
    if minor != manifest.abi_minor
        || raw.abi_minor != minor
        || raw.abi_major != major
        || minimum != manifest.min_host_version
        || descriptor.plugin_id != manifest.plugin_id
        || descriptor.plugin_version != manifest.plugin_version
        || kind != manifest.kind
        || layout(descriptor.input_layout) != manifest.input_layout
        || layout(descriptor.output_layout) != manifest.output_layout
        || descriptor.capabilities.sidechain_input != manifest.sidechain_input
        || descriptor.capabilities.reports_tail != manifest.reports_tail
        || descriptor.max_polyphony != manifest.max_polyphony
        || descriptor.parameters != expected_parameters
    {
        return Err(mismatch("C descriptor differs from the supplied manifest"));
    }
    if descriptor.max_polyphony.is_some_and(|n| n > 65536) {
        return Err(mismatch("polyphony exceeds host budget"));
    }
    Ok((entry, descriptor))
}
