use oxitone_graph::{HostContext, ParameterEvent, PluginInstance, ProcessContext};
pub const RATE: f64 = 48000.;
pub const NEW: [&str; 12] = [
    "oxitone.nonlinear-filter",
    "oxitone.compactor",
    "oxitone.multiband-dynamics",
    "oxitone.resonator",
    "oxitone.frequency-shifter",
    "oxitone.pitch-shifter",
    "oxitone.flanger",
    "oxitone.convolver",
    "oxitone.bitcrush",
    "oxitone.tape",
    "oxitone.spreader",
    "oxitone.limiter",
];
pub fn configured(id: &str, params: &[(&str, f64)]) -> Box<dyn PluginInstance> {
    let plugin = oxitone_mixer::builtin_effect_plugins()
        .into_iter()
        .find(|p| p.descriptor().plugin_id == id)
        .unwrap();
    let mut instance = plugin.create(&HostContext {
        sample_rate: RATE,
        max_block_size: 256,
    });
    set(instance.as_mut(), params);
    instance.reset();
    instance
}
pub fn set(instance: &mut dyn PluginInstance, params: &[(&str, f64)]) {
    let events: Vec<_> = params
        .iter()
        .map(|(id, v)| ParameterEvent {
            frame_offset: 0,
            parameter_id: id,
            value: *v,
        })
        .collect();
    // Zero-frame calls configure only, and must not advance DSP time.
    instance.process(&mut ProcessContext {
        frames: 0,
        sample_rate: RATE,
        inputs: &[&[], &[]],
        outputs: &mut [&mut [], &mut []],
        note_events: &[],
        parameter_events: &events,
        sidechain: None,
    });
}
pub fn render(
    instance: &mut dyn PluginInstance,
    left: &[f32],
    right: &[f32],
    block: usize,
) -> [Vec<f32>; 2] {
    let mut output = [vec![0.; left.len()], vec![0.; left.len()]];
    let [out_l, out_r] = &mut output;
    for start in (0..left.len()).step_by(block) {
        let end = (start + block).min(left.len());
        instance.process(&mut ProcessContext {
            frames: end - start,
            sample_rate: RATE,
            inputs: &[&left[start..end], &right[start..end]],
            outputs: &mut [&mut out_l[start..end], &mut out_r[start..end]],
            note_events: &[],
            parameter_events: &[],
            sidechain: None,
        });
    }
    output
}
pub fn sine(hz: f64, amp: f64, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| (amp * (std::f64::consts::TAU * hz * i as f64 / RATE).sin()) as f32)
        .collect()
}
pub fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}
pub fn amplitude(x: &[f32], hz: f64) -> f64 {
    let (mut real, mut imag) = (0., 0.);
    for (n, x) in x.iter().enumerate() {
        let (s, c) = (std::f64::consts::TAU * hz * n as f64 / RATE).sin_cos();
        real += *x as f64 * c;
        imag += *x as f64 * s;
    }
    2. * real.hypot(imag) / x.len() as f64
}
