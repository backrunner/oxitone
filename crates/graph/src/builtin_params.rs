//! Fixed built-in parameter sets for engine-owned graph nodes
//! (04-api-contracts.md §内建图节点的 stable parameter ID 全集,
//! 02-domain-spec.md §Automation). Built-in nodes and plugin parameters share
//! one `ParameterSpec` schema, mapping, smoothing, and scheduling path, so
//! automation target validation reuses these tables.

use oxitone_core::wire::{
    ParameterMapping, ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit,
};

/// Engine-owned entities that expose automatable parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinEntityKind {
    Project,
    Channel,
    MixerChannel,
    SampleClip,
    EffectInsert,
}

/// Sample edit fields baked into PCM at prepare; binding automation to them
/// is `AutomationTargetInvalid` (02-domain-spec.md §Automation 边界规则).
pub const NON_AUTOMATABLE_SAMPLE_FIELDS: &[&str] = &[
    "startFrame",
    "endFrame",
    "normalize",
    "fadeIn",
    "fadeOut",
    "crossfade",
];

/// `send.<destinationId>.ratio` parameter ID parts (MixerChannel).
pub const SEND_RATIO_PREFIX: &str = "send.";
pub const SEND_RATIO_SUFFIX: &str = ".ratio";

/// `insert.<index>.<param>` parameter ID prefix (EffectInsert built-ins on a
/// Channel's `effectChain`, addressed on the owning Channel entity).
pub const INSERT_PREFIX: &str = "insert.";

/// Built-in per-insert parameters addressable through `insert.<index>.<id>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertParam {
    Mix,
    Bypass,
}

impl InsertParam {
    pub fn id(self) -> &'static str {
        match self {
            InsertParam::Mix => "mix",
            InsertParam::Bypass => "bypass",
        }
    }

    fn spec(self) -> ParameterSpec {
        match self {
            InsertParam::Mix => continuous("mix", "Mix", 0.0, 1.0, 1.0),
            InsertParam::Bypass => step("bypass", "Bypass"),
        }
    }
}

fn spec(
    id: &str,
    label: &str,
    min: f64,
    max: f64,
    default: f64,
    mapping: ParameterMapping,
) -> ParameterSpec {
    ParameterSpec {
        id: id.to_string(),
        label: label.to_string(),
        unit: ParameterUnit::Normalized,
        min,
        max,
        default,
        smoothing: ParameterSmoothing::Linear,
        rate: ParameterRate::Control,
        automation: Some(true),
        mapping: Some(mapping),
    }
}

fn continuous(id: &str, label: &str, min: f64, max: f64, default: f64) -> ParameterSpec {
    spec(id, label, min, max, default, ParameterMapping::Linear)
}

fn bipolar(id: &str, label: &str, default: f64) -> ParameterSpec {
    spec(id, label, -1.0, 1.0, default, ParameterMapping::Bipolar)
}

fn step(id: &str, label: &str) -> ParameterSpec {
    ParameterSpec {
        unit: ParameterUnit::Enum,
        smoothing: ParameterSmoothing::None,
        ..spec(id, label, 0.0, 1.0, 0.0, ParameterMapping::Enum)
    }
}

/// Project: `tempo` (20..=999 BPM, log mapping, control rate).
pub fn project_parameters() -> Vec<ParameterSpec> {
    vec![spec(
        "tempo",
        "Tempo",
        20.0,
        999.0,
        120.0,
        ParameterMapping::Log,
    )]
}

/// Channel: `level` (0..2), `pan` (-1..1), `mute` (0|1), `swing` (0..1).
pub fn channel_parameters() -> Vec<ParameterSpec> {
    vec![
        continuous("level", "Level", 0.0, 2.0, 1.0),
        bipolar("pan", "Pan", 0.0),
        step("mute", "Mute"),
        continuous("swing", "Swing", 0.0, 1.0, 0.0),
    ]
}

/// MixerChannel static parameters; per-send ratio parameters are dynamic —
/// see [`send_ratio_parameter_id`] and [`send_ratio_parameter`].
pub fn mixer_channel_parameters() -> Vec<ParameterSpec> {
    vec![
        continuous("level", "Level", 0.0, 2.0, 1.0),
        bipolar("balance", "Balance", 0.0),
        step("mute", "Mute"),
        continuous("masterSendRatio", "Master Send Ratio", 0.0, 1.0, 1.0),
    ]
}

/// SampleClip: `level` (0..2), `tone` (-1..1), `gain` (0..2), `pan` (-1..1),
/// `rate` (0.25..4, log).
pub fn sample_clip_parameters() -> Vec<ParameterSpec> {
    vec![
        continuous("level", "Level", 0.0, 2.0, 1.0),
        bipolar("tone", "Tone", 0.0),
        continuous("gain", "Gain", 0.0, 2.0, 1.0),
        bipolar("pan", "Pan", 0.0),
        spec("rate", "Rate", 0.25, 4.0, 1.0, ParameterMapping::Log),
    ]
}

/// EffectInsert built-ins, stacked on top of the plugin's own parameters:
/// `mix` (dry/wet 0..1, default 1) and `bypass` (0|1).
pub fn effect_insert_parameters() -> Vec<ParameterSpec> {
    vec![
        continuous("mix", "Mix", 0.0, 1.0, 1.0),
        step("bypass", "Bypass"),
    ]
}

/// All static parameters for a built-in entity kind.
pub fn parameters_for(kind: BuiltinEntityKind) -> Vec<ParameterSpec> {
    match kind {
        BuiltinEntityKind::Project => project_parameters(),
        BuiltinEntityKind::Channel => channel_parameters(),
        BuiltinEntityKind::MixerChannel => mixer_channel_parameters(),
        BuiltinEntityKind::SampleClip => sample_clip_parameters(),
        BuiltinEntityKind::EffectInsert => effect_insert_parameters(),
    }
}

/// Parameter ID of the send-ratio parameter for `destination_id`.
pub fn send_ratio_parameter_id(destination_id: &str) -> String {
    format!("{SEND_RATIO_PREFIX}{destination_id}{SEND_RATIO_SUFFIX}")
}

/// Spec of one send-ratio parameter (0..1, default 1), exposed per send.
pub fn send_ratio_parameter(destination_id: &str) -> ParameterSpec {
    continuous(
        &send_ratio_parameter_id(destination_id),
        "Send Ratio",
        0.0,
        1.0,
        1.0,
    )
}

/// Parse `send.<destinationId>.ratio`, returning the destination ID.
pub fn parse_send_ratio_parameter(parameter_id: &str) -> Option<&str> {
    parameter_id
        .strip_prefix(SEND_RATIO_PREFIX)?
        .strip_suffix(SEND_RATIO_SUFFIX)
        .filter(|destination| !destination.is_empty())
}

/// Parameter ID of the built-in `param` of insert number `index`.
pub fn insert_parameter_id(index: usize, param: InsertParam) -> String {
    format!("{INSERT_PREFIX}{index}.{}", param.id())
}

/// Parse `insert.<index>.<param>`, returning the insert index and parameter.
pub fn parse_insert_parameter(parameter_id: &str) -> Option<(usize, InsertParam)> {
    let (index, parameter) = crate::insert_params::parse(parameter_id)?;
    let param = match parameter {
        crate::insert_params::InsertParameter::Mix => InsertParam::Mix,
        crate::insert_params::InsertParameter::Bypass => InsertParam::Bypass,
        crate::insert_params::InsertParameter::Plugin(_) => return None,
    };
    Some((index, param))
}

/// Spec of one built-in insert parameter (same definitions as
/// [`effect_insert_parameters`]).
pub fn insert_parameter(param: InsertParam) -> ParameterSpec {
    param.spec()
}

/// Static lookup over the fixed sets plus dynamic `send.<id>.ratio` on mixer
/// channels. Does not check that the send destination exists — that is the
/// validator's job.
pub fn find_parameter(kind: BuiltinEntityKind, parameter_id: &str) -> Option<ParameterSpec> {
    if let Some(spec) = parameters_for(kind)
        .into_iter()
        .find(|s| s.id == parameter_id)
    {
        return Some(spec);
    }
    if kind == BuiltinEntityKind::MixerChannel {
        return parse_send_ratio_parameter(parameter_id).map(send_ratio_parameter);
    }
    None
}
