//! Shared control-thread resolution of effect insert paths.

use oxitone_core::wire::{EffectRef, ParameterSpec};

use crate::builtin_params::{insert_parameter, InsertParam};
use crate::PluginRegistry;

#[derive(Debug, Clone, Copy)]
pub enum InsertParameter<'a> {
    Mix,
    Bypass,
    Plugin(&'a str),
}

pub fn parse(parameter_id: &str) -> Option<(usize, InsertParameter<'_>)> {
    let (index, parameter) = parameter_id.strip_prefix("insert.")?.split_once('.')?;
    if index.is_empty()
        || !index.bytes().all(|c| c.is_ascii_digit())
        || (index.len() > 1 && index.starts_with('0'))
    {
        return None;
    }
    let parameter = match parameter {
        "mix" => InsertParameter::Mix,
        "bypass" => InsertParameter::Bypass,
        id => InsertParameter::Plugin(id.strip_prefix("parameter.").filter(|s| !s.is_empty())?),
    };
    Some((index.parse().ok()?, parameter))
}

pub fn resolve(
    effects: &[EffectRef],
    registry: &PluginRegistry,
    parameter_id: &str,
) -> Option<ParameterSpec> {
    let (index, parameter) = parse(parameter_id)?;
    let effect = effects.get(index)?;
    match parameter {
        InsertParameter::Mix => Some(insert_parameter(InsertParam::Mix)),
        InsertParameter::Bypass => Some(insert_parameter(InsertParam::Bypass)),
        InsertParameter::Plugin(id) => registry
            .lookup_descriptor(&effect.plugin_id, &effect.plugin_version)?
            .parameters
            .iter()
            .find(|spec| spec.id == id)
            .cloned(),
    }
}
