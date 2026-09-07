use oxitone_graph::abi_c::*;

pub(crate) struct Shared<T>(pub T);
// SAFETY: only immutable static ABI tables are shared; all pointees are static.
unsafe impl<T> Sync for Shared<T> {}

const fn tone(
    id: &'static std::ffi::CStr,
    label: &'static std::ffi::CStr,
    min: f64,
    max: f64,
    default: f64,
) -> OxiParamSpecV1 {
    OxiParamSpecV1 {
        id: id.as_ptr(),
        label: label.as_ptr(),
        unit: 0,
        min,
        max,
        default_value: default,
        smoothing: 0,
        rate: 0,
        automation: 1,
        mapping: 1,
    }
}
static PARAMETERS: Shared<[OxiParamSpecV1; 12]> = Shared([
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
    tone(c"kickTune", c"Kick Tune (Hz)", 32., 80., 48.),
    tone(c"kickSweep", c"Kick Sweep (Hz)", 0., 400., 130.),
    tone(c"kickClick", c"Kick Click", 0., 1., 0.),
    tone(c"snareTune", c"Snare Body (Hz)", 100., 320., 185.),
    tone(c"snareSnap", c"Snare Snap", 0., 1., 0.),
    tone(c"kickDecay", c"Kick Decay", 0.25, 2., 1.),
    tone(c"snareDecay", c"Snare Decay", 0.25, 2., 1.),
    tone(c"closedDecay", c"Closed Hat Decay", 0.25, 2., 1.),
    tone(c"openDecay", c"Open Hat Decay", 0.25, 2., 1.),
    tone(c"hatTone", c"Hat Color", 0., 1., 0.),
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
    param_count: 12,
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
