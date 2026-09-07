use crate::plugin_layout::{Control, Layout};
use oxitone_core::wire::ParameterUnit;
use oxitone_graph::descriptor::PluginDescriptor;
use std::collections::HashSet;

pub fn validate(layout: &Layout, descriptor: &PluginDescriptor) -> Result<(), String> {
    let invalid = |message: &str| Err(message.to_owned());
    if layout.ui_version != "1.0" {
        return invalid("Unsupported UI version");
    }
    if layout.plugin_id != descriptor.plugin_id
        || layout.plugin_version != descriptor.plugin_version
    {
        return invalid("Plugin identity does not match descriptor");
    }
    if !text(&layout.plugin_id, 128)
        || !text(&layout.plugin_version, 128)
        || !text(&layout.title, 64)
        || !(440..=1200).contains(&layout.size.width)
        || !(280..=900).contains(&layout.size.height)
    {
        return invalid("Invalid panel title, identity or size");
    }
    if !(1..=8).contains(&layout.pages.len()) {
        return invalid("Expected 1–8 pages");
    }
    let mut pages = HashSet::new();
    let mut count = 0;
    for page in &layout.pages {
        if !text(&page.id, 128)
            || !text(&page.title, 64)
            || !pages.insert(&page.id)
            || !(1..=16).contains(&page.groups.len())
        {
            return invalid("Invalid page or duplicate page ID");
        }
        let mut groups = HashSet::new();
        for group in &page.groups {
            if !text(&group.id, 128)
                || !text(&group.title, 64)
                || !groups.insert(&group.id)
                || !(1..=6).contains(&group.columns)
                || !(1..=32).contains(&group.controls.len())
            {
                return invalid("Invalid group, columns or duplicate group ID");
            }
            count += group.controls.len();
            if count > 256 {
                return invalid("Panel exceeds 256 controls");
            }
            for control in &group.controls {
                if control.label().is_some_and(|label| !text(label, 64)) {
                    return invalid("Invalid control label");
                }
                let mut specs = Vec::new();
                for id in control.bindings() {
                    let spec = descriptor
                        .parameters
                        .iter()
                        .find(|p| p.id == id)
                        .filter(|_| text(id, 128))
                        .ok_or_else(|| format!("Unknown plugin parameter: {id}"))?;
                    specs.push(spec);
                }
                match control {
                    Control::Toggle { .. }
                        if specs[0].unit != ParameterUnit::Enum
                            || specs[0].min != 0.
                            || specs[0].max != 1. =>
                    {
                        return invalid("Toggle requires a boolean enum parameter")
                    }
                    Control::Choice { options, .. } => {
                        let spec = specs[0];
                        if spec.unit != ParameterUnit::Enum || !(2..=16).contains(&options.len()) {
                            return invalid("Choice requires an enum and 2–16 options");
                        }
                        for (i, option) in options.iter().enumerate() {
                            if !text(&option.label, 64)
                                || !option.value.is_finite()
                                || option.value.fract() != 0.
                                || option.value < spec.min
                                || option.value > spec.max
                                || options[..i].iter().any(|p| p.value == option.value)
                            {
                                return invalid("Invalid or duplicate choice option");
                            }
                        }
                    }
                    Control::Envelope { .. } => {
                        if [0, 1, 3]
                            .iter()
                            .any(|&i| specs[i].unit != ParameterUnit::Seconds || specs[i].min < 0.)
                            || specs[2].unit != ParameterUnit::Normalized
                            || specs[2].min < 0.
                            || specs[2].max > 1.
                        {
                            return invalid(
                                "Envelope requires nonnegative seconds and normalized sustain",
                            );
                        }
                    }
                    Control::Oscillator {
                        bank,
                        warp_mode,
                        warp,
                        octave,
                        ..
                    } => {
                        let within = |i: usize, unit, min, max| {
                            specs[i].unit == unit && specs[i].min >= min && specs[i].max <= max
                        };
                        if ![0, 1]
                            .iter()
                            .all(|&i| within(i, ParameterUnit::Enum, 0., 5.))
                            || ![2, 3, 6]
                                .iter()
                                .all(|&i| within(i, ParameterUnit::Normalized, 0., 1.))
                            || !within(4, ParameterUnit::Enum, 1., 16.)
                            || !within(5, ParameterUnit::Normalized, 0., 100.)
                        {
                            return invalid("Oscillator requires basic-cycle enums, normalized phase/position/spread and unison/detune");
                        }
                        let mut i = 7;
                        for (id, unit, min, max) in [
                            (bank, ParameterUnit::Enum, 0., 3.),
                            (warp_mode, ParameterUnit::Enum, 0., 3.),
                            (warp, ParameterUnit::Normalized, 0., 1.),
                            (octave, ParameterUnit::Enum, -4., 4.),
                        ] {
                            if id.is_some() {
                                if !within(i, unit, min, max) {
                                    return invalid("Invalid oscillator bank/warp/octave binding");
                                }
                                i += 1;
                            }
                        }
                    }
                    Control::SubOscillator { .. } => {
                        if specs[0].unit != ParameterUnit::Enum
                            || specs[0].min < 0.
                            || specs[0].max > 5.
                            || specs[1].unit != ParameterUnit::Enum
                            || specs[1].min < -4.
                            || specs[1].max > 4.
                            || specs[2].unit != ParameterUnit::Normalized
                            || specs[2].min < 0.
                            || specs[2].max > 1.
                        {
                            return invalid(
                                "Sub requires six-wave enum, octave enum and normalized level",
                            );
                        }
                    }
                    Control::FilterResponse { .. } => {
                        if specs[0].unit != ParameterUnit::Enum
                            || specs[0].min < 0.
                            || specs[0].max > 2.
                            || specs[1].unit != ParameterUnit::Hz
                            || specs[1].min <= 0.
                            || specs[2].unit != ParameterUnit::Normalized
                            || specs[2].min < 0.
                            || specs[2].max > 1.
                        {
                            return invalid(
                                "Filter response requires LP/HP/BP, Hz and normalized resonance",
                            );
                        }
                    }
                    Control::LfoCurve { .. } => {
                        if specs[0].unit != ParameterUnit::Enum
                            || specs[0].min < 0.
                            || specs[0].max > 3.
                            || specs[1].unit != ParameterUnit::Hz
                            || specs[1].min <= 0.
                            || specs[2].unit != ParameterUnit::Normalized
                            || specs[2].min < 0.
                            || specs[2].max > 1.
                        {
                            return invalid(
                                "LFO curve requires four-shape enum, Hz rate and normalized phase",
                            );
                        }
                    }
                    Control::Modulation { routes, .. } => {
                        if !(1..=8).contains(&routes.len())
                            || routes.iter().any(|r| !text(&r.label, 64))
                        {
                            return invalid("Modulation display requires 1–8 labelled routes");
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.encode_utf16().count() <= max && !value.chars().any(char::is_control)
}
