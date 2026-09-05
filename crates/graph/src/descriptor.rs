//! Static plugin metadata (01-architecture.md §Plugin model, 08-plugin-abi.md
//! §ABI v1). A descriptor is the single source of truth for a plugin's ID,
//! version, channel layout, parameter specs, capabilities, and optional
//! structured-state schema. Built-in and third-party plugins share this model.

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterSpec, ParameterUnit};

/// Instrument or effect (08-plugin-abi.md `OxiPluginKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    Instrument,
    Effect,
}

/// Non-interleaved channel layout (08-plugin-abi.md `OxiChannelLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    None,
    Mono,
    Stereo,
}

impl ChannelLayout {
    /// Number of non-interleaved f32 buses for this layout.
    pub fn bus_count(self) -> usize {
        match self {
            ChannelLayout::None => 0,
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
        }
    }
}

/// Capability flags advertised by the descriptor (08-plugin-abi.md
/// `capabilities`). Extend with new fields; never repurpose existing ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginCapabilities {
    /// Accepts a sidechain detector input (e.g. Compressor).
    pub sidechain_input: bool,
    /// Reports a meaningful tail via `tail_frames`; plugins without this flag
    /// are silent immediately after reset/switch.
    pub reports_tail: bool,
}

/// Versioned identifier of a plugin's structured-state schema
/// (04-api-contracts.md §InstrumentRef.state). New schema versions use new IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateSchemaId(pub &'static str);

/// The only Phase 1 plugin using the structured-state channel.
pub const SLICER_PLUGIN_ID: &str = "oxitone.slicer";
/// State schema of the built-in Slicer slice table (04-api-contracts.md).
pub const SLICER_STATE_SCHEMA_ID: StateSchemaId = StateSchemaId("oxitone.slicer.slices@1");

/// Immutable metadata owned by a plugin factory and borrowed by the registry.
#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub plugin_id: String,
    pub plugin_version: String,
    pub kind: PluginKind,
    /// Effect input layout; must be `None` for instruments.
    pub input_layout: ChannelLayout,
    pub output_layout: ChannelLayout,
    pub parameters: Vec<ParameterSpec>,
    pub capabilities: PluginCapabilities,
    /// Declared when the plugin accepts `InstrumentRef.state`; Phase 1 only
    /// `oxitone.slicer` uses this channel.
    pub state_schema: Option<StateSchemaId>,
    /// Instruments only; `None` for effects.
    pub max_polyphony: Option<u32>,
}

impl PluginDescriptor {
    /// Structural validation applied at registration time. Failures use
    /// `PluginManifestMismatch`: an invalid descriptor is an invalid manifest.
    pub fn validate(&self) -> Result<(), OxitoneError> {
        if self.plugin_id.is_empty() || self.plugin_version.is_empty() {
            return Err(manifest_err(
                "plugin_id and plugin_version must be non-empty",
            ));
        }
        match self.kind {
            PluginKind::Instrument if self.input_layout != ChannelLayout::None => {
                return Err(manifest_err("instruments must declare input_layout none"));
            }
            PluginKind::Effect if self.input_layout == ChannelLayout::None => {
                return Err(manifest_err("effects must declare an input layout"));
            }
            _ => {}
        }
        if self.output_layout == ChannelLayout::None {
            return Err(manifest_err("output_layout must not be none"));
        }
        if self.kind == PluginKind::Effect && self.max_polyphony.is_some() {
            return Err(manifest_err("max_polyphony only applies to instruments"));
        }
        validate_parameter_specs(&self.parameters)?;
        Ok(())
    }
}

fn manifest_err(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::PLUGIN_MANIFEST_MISMATCH, message)
}

/// Parameter table rules shared by plugin descriptors and the built-in
/// parameter sets (04-api-contracts.md §ParameterSpec).
pub fn validate_parameter_specs(specs: &[ParameterSpec]) -> Result<(), OxitoneError> {
    for (index, spec) in specs.iter().enumerate() {
        if spec.id.is_empty() {
            return Err(manifest_err(format!("parameter #{index} has an empty id")));
        }
        if specs[..index].iter().any(|other| other.id == spec.id) {
            return Err(manifest_err(format!(
                "duplicate parameter id {:?}",
                spec.id
            )));
        }
        validate_parameter_spec(spec)?;
    }
    Ok(())
}

/// Range/default and unit/mapping combination rules for one spec.
pub fn validate_parameter_spec(spec: &ParameterSpec) -> Result<(), OxitoneError> {
    let id = &spec.id;
    let finite = [spec.min, spec.max, spec.default]
        .iter()
        .all(|v| v.is_finite());
    if !finite {
        return Err(manifest_err(format!(
            "parameter {id:?} range must be finite"
        )));
    }
    if spec.min > spec.max {
        return Err(manifest_err(format!("parameter {id:?} has min > max")));
    }
    if spec.default < spec.min || spec.default > spec.max {
        return Err(manifest_err(format!(
            "parameter {id:?} default is outside min..=max"
        )));
    }
    let is_enum = spec.unit == ParameterUnit::Enum || spec.mapping == Some(ParameterMapping::Enum);
    if is_enum {
        if spec.unit != ParameterUnit::Enum || spec.mapping != Some(ParameterMapping::Enum) {
            return Err(manifest_err(format!(
                "parameter {id:?} must pair unit enum with mapping enum"
            )));
        }
        if spec.min.fract() != 0.0 || spec.max.fract() != 0.0 {
            return Err(manifest_err(format!(
                "parameter {id:?} enum range must use integer steps"
            )));
        }
        if spec.smoothing != ParameterSmoothing::None {
            return Err(manifest_err(format!(
                "parameter {id:?} enum parameters must not smooth"
            )));
        }
    }
    match spec.mapping {
        Some(ParameterMapping::Log) if spec.min <= 0.0 => {
            return Err(manifest_err(format!(
                "parameter {id:?} log mapping requires min > 0"
            )));
        }
        Some(ParameterMapping::Bipolar) if spec.min >= 0.0 || spec.max <= 0.0 => {
            return Err(manifest_err(format!(
                "parameter {id:?} bipolar mapping requires min < 0 < max"
            )));
        }
        _ => {}
    }
    Ok(())
}
