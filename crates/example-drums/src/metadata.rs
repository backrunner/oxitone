use oxitone_graph::abi_c::*;

pub(crate) struct Shared<T>(pub T);
// SAFETY: only immutable static ABI tables are shared; all pointees are static.
unsafe impl<T> Sync for Shared<T> {}

static PARAMETERS: Shared<[OxiParamSpecV1; 2]> = Shared([
    OxiParamSpecV1 {
        id: c"volume".as_ptr(),
        label: c"Volume".as_ptr(),
        unit: 0,
        min: 0.0,
        max: 1.0,
        default_value: 0.8,
        smoothing: 0,
        rate: 1,
        automation: 1,
        mapping: 1,
    },
    OxiParamSpecV1 {
        id: c"decay".as_ptr(),
        label: c"Decay".as_ptr(),
        unit: 0,
        min: 0.5,
        max: 2.0,
        default_value: 1.0,
        smoothing: 0,
        rate: 0,
        automation: 1,
        mapping: 1,
    },
]);

static DESCRIPTOR: Shared<OxiPluginDescriptorV1> = Shared(OxiPluginDescriptorV1 {
    abi_major: 1,
    abi_minor: 0,
    plugin_id: c"example.drums".as_ptr(),
    plugin_version: c"1.0.0".as_ptr(),
    kind: 0,
    input_layout: 0,
    output_layout: 2,
    capabilities: 2,
    params: PARAMETERS.0.as_ptr(),
    param_count: 2,
    max_polyphony: 4,
});

pub(crate) static ENTRY: Shared<OxiPluginEntryV1> = Shared(OxiPluginEntryV1 {
    abi_major: 1,
    abi_minor: 0,
    struct_size: std::mem::size_of::<OxiPluginEntryV1>() as u32,
    descriptor: &DESCRIPTOR.0,
    min_host_version: c"0.1.0".as_ptr(),
    create: Some(super::create),
    dispose: Some(super::dispose),
    prepare: Some(super::prepare),
    process: Some(super::process),
    reset: Some(super::reset),
    tail_frames: Some(super::tail),
    latency_frames: Some(super::latency),
});
