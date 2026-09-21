//! Plugin instance factories used by `build.rs` (control thread).

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{MixerChannelSpec, ParameterUnit};
use oxitone_graph::abi::HostContext;
use oxitone_graph::PluginRegistry;

use crate::assets::SampleStore;
use crate::channel::{BeatParam, InsertNode};
use crate::graph::MixerBeatParam;

fn invalid(message: impl Into<String>, path: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

/// Plain initial events plus beat-unit conversions of one effect.
type SplitParams = (Vec<(usize, f64)>, Vec<BeatParam>);

/// Split an effect's initial parameters into plain initial events and
/// beat-unit conversions (`<name>Beats` → `<name>Seconds`).
pub(super) fn split_effect_params(
    param_ids: &[String],
    units: &BTreeMap<String, ParameterUnit>,
    parameters: &BTreeMap<String, f64>,
    path: &str,
) -> Result<SplitParams, OxitoneError> {
    let mut initial = Vec::new();
    let mut beat_params = Vec::new();
    for (index, id) in param_ids.iter().enumerate() {
        let value = parameters.get(id).copied();
        if units.get(id) == Some(&ParameterUnit::Beats) {
            if let Some(seconds_id) = id
                .strip_suffix("Beats")
                .map(|stem| format!("{stem}Seconds"))
                .filter(|candidate| param_ids.iter().any(|p| p == candidate))
            {
                beat_params.push(BeatParam {
                    beats: value.unwrap_or(0.0),
                    parameter_index: index,
                    seconds_index: param_ids.iter().position(|p| p == &seconds_id).unwrap(),
                    active: value.is_some(),
                    last_sent: f64::NAN,
                });
                continue;
            }
        }
        if let Some(value) = value {
            initial.push((index, value));
        }
    }
    for id in parameters.keys() {
        if !param_ids.contains(id) {
            return Err(invalid(
                format!("unknown parameter {id:?}"),
                format!("{path}.{id}"),
            ));
        }
    }
    Ok((initial, beat_params))
}

/// Create one channel insert from an `EffectRef`.
pub(super) fn create_insert(
    midi_output: bool,
    channel_id: &str,
    index: usize,
    effect: &oxitone_core::wire::EffectRef,
    host: &HostContext,
    registry: &PluginRegistry,
    samples: &SampleStore,
    sample_rate: f64,
    max_block: u32,
) -> Result<InsertNode, OxitoneError> {
    let path = format!("$.channels[{channel_id}].effectChain[{index}]");
    let plugin = registry
        .lookup_instance(
            &effect.plugin_id,
            &effect.plugin_version,
            effect.instance_id.as_deref(),
        )
        .ok_or_else(|| {
            invalid(
                format!(
                    "unknown plugin {}@{}",
                    effect.plugin_id, effect.plugin_version
                ),
                path.clone(),
            )
        })?;
    let descriptor = plugin.descriptor();
    let param_ids: Arc<Vec<String>> =
        Arc::new(descriptor.parameters.iter().map(|p| p.id.clone()).collect());
    let units: BTreeMap<String, ParameterUnit> = descriptor
        .parameters
        .iter()
        .map(|p| (p.id.clone(), p.unit))
        .collect();
    let (mut initial, beats) = split_effect_params(&param_ids, &units, &effect.parameters, &path)?;
    let mut instance =
        oxitone_mixer::effects::create_effect(plugin.as_ref(), effect, host, Some(samples))?;
    instance.configure_input_buses(&[])?;
    instance.configure_output_buses(&[])?;
    instance.configure_midi_output(midi_output)?;
    instance.try_prepare(sample_rate, max_block)?;
    if instance.initial_parameters_applied() {
        initial.clear();
    }
    let mut mix = oxitone_dsp::gain_pan::OnePoleSmoother::new(sample_rate, 20.0);
    mix.snap(effect.mix.unwrap_or(1.0) as f32);
    let latency = instance.latency_frames() as usize;
    let mut dry_delay = oxitone_mixer::DelayLine::new(latency, max_block as usize);
    dry_delay.set_delay(latency);
    Ok(InsertNode {
        dry_delay,
        delayed_l: vec![0.0; max_block as usize],
        delayed_r: vec![0.0; max_block as usize],
        instance,
        param_ids: param_ids.clone(),
        specs: Arc::new(descriptor.parameters.clone()),
        initial,
        mix,
        bypass: effect.bypass.unwrap_or(false),
        beat_params: beats,
        staged: oxitone_mixer::parameter_queue::ParameterQueue::new(&descriptor.parameters),
        first_block: true,
    })
}

/// Preprocess a mixer channel spec: beat-unit insert parameters are moved
/// out of the initial table and their seconds counterparts computed at the
/// compile-time BPM; the per-block conversion state is collected.
pub(super) fn preprocess_mixer_channel(
    spec: &MixerChannelSpec,
    registry: &PluginRegistry,
    bpm: f64,
    beat_params: &mut Vec<MixerBeatParam>,
) -> Result<MixerChannelSpec, OxitoneError> {
    let mut out = spec.clone();
    for (index, effect) in out.inserts.iter_mut().enumerate() {
        let Some(descriptor) = registry.instance_descriptor(
            &effect.plugin_id,
            &effect.plugin_version,
            effect.instance_id.as_deref(),
        ) else {
            continue;
        };
        let param_ids: Vec<&str> = descriptor
            .parameters
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        let units: BTreeMap<&str, ParameterUnit> = descriptor
            .parameters
            .iter()
            .map(|p| (p.id.as_str(), p.unit))
            .collect();
        let mut removals = Vec::new();
        let mut additions = Vec::new();
        for (parameter_index, id) in param_ids.iter().enumerate() {
            if units.get(id) != Some(&ParameterUnit::Beats) {
                continue;
            }
            let Some(seconds_id) = id
                .strip_suffix("Beats")
                .map(|stem| format!("{stem}Seconds"))
                .filter(|candidate| param_ids.iter().any(|p| p == candidate))
            else {
                continue;
            };
            let value = effect.parameters.get(*id).copied();
            let seconds = value.unwrap_or(0.0) * 60.0 / bpm.max(1e-9);
            if value.is_some() {
                removals.push(id.to_string());
                additions.push((seconds_id.clone(), seconds));
            }
            beat_params.push(MixerBeatParam {
                bus: spec.id.clone(),
                bus_index: 0, // Resolved after the mixer's topological sort.
                insert: index,
                state: BeatParam {
                    beats: value.unwrap_or(0.0),
                    parameter_index,
                    active: value.is_some(),
                    seconds_index: param_ids.iter().position(|p| *p == seconds_id).unwrap(),
                    last_sent: seconds,
                },
            });
        }
        for id in removals {
            effect.parameters.remove(&id);
        }
        for (id, value) in additions {
            effect.parameters.insert(id, value);
        }
    }
    Ok(out)
}
