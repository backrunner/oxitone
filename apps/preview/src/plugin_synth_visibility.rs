//! Presentation rules for the bundled synth; hidden controls retain their source values.
use crate::{plugin_details::PluginDetails, plugin_layout::Control};

pub fn bundled(details: &PluginDetails) -> bool {
    details.info.library.is_none()
        && details.info.descriptor.plugin_id == "oxitone.wavetable"
        && details.info.descriptor.plugin_version == "1.0.0"
}
pub fn value(details: &PluginDetails, id: &str) -> f64 {
    details
        .parameters
        .iter()
        .find(|p| !p.host && p.spec.id == id)
        .map_or(0., |p| p.value)
}
pub fn advanced(id: &str) -> bool {
    matches!(
        id.split_once('.'),
        Some((
            "oscA" | "oscB",
            "octave" | "pitch" | "phase" | "spread" | "phaseSpread"
        ))
    ) || id.ends_with("Curve")
        || id.ends_with(".curve")
        || id.ends_with(".phase")
}
pub fn active(details: &PluginDetails, id: &str) -> bool {
    if let Some((osc @ ("oscA" | "oscB"), field)) = id.split_once('.') {
        return match field {
            "wavetable" | "morphTo" => value(details, &format!("{osc}.bank")) == 0.,
            "warp" => value(details, &format!("{osc}.warpMode")) != 0.,
            "detune" | "spread" | "phaseSpread" => value(details, &format!("{osc}.unison")) > 1.,
            _ => true,
        };
    }
    if let Some(rest) = id.strip_prefix("mod.") {
        if let Some((slot, field)) = rest.split_once('.') {
            return field == "source" || value(details, &format!("mod.{slot}.source")) != 0.;
        }
    }
    id != "glide" || value(details, "voiceMode") != 0.
}
pub fn group_visible(details: &PluginDetails, id: &str) -> bool {
    let Some(slot) = id
        .strip_prefix("route")
        .and_then(|s| s.parse::<usize>().ok())
    else {
        return true;
    };
    value(details, &format!("mod.{slot}.source")) != 0.
        || (0..slot).all(|i| value(details, &format!("mod.{i}.source")) != 0.)
}
pub fn control(control: &Control) -> Control {
    let id = control.bindings()[0];
    if id.ends_with(".position") || id.ends_with(".amount") || id == "osc.mix" {
        Control::Fader {
            parameter: id.into(),
            label: control.label().map(str::to_owned),
        }
    } else {
        control.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn details() -> PluginDetails {
        crate::plugin_plot_tests::details(oxitone_instruments::wavetable::descriptor().clone())
    }
    fn set(d: &mut PluginDetails, id: &str, v: f64) {
        d.parameters
            .iter_mut()
            .find(|p| p.spec.id == id)
            .unwrap()
            .value = v;
    }
    #[test]
    fn mode_changes_only_reveal_relevant_controls_without_changing_values() {
        let mut d = details();
        assert!(active(&d, "oscA.wavetable"));
        set(&mut d, "oscA.bank", 2.);
        assert!(!active(&d, "oscA.wavetable"));
        assert!(!active(&d, "oscA.morphTo"));
        assert!(active(&d, "oscA.position"));
        assert!(!active(&d, "oscA.warp"));
        set(&mut d, "oscA.warpMode", 1.);
        assert!(active(&d, "oscA.warp"));
        assert!(!active(&d, "oscA.detune"));
        set(&mut d, "oscA.unison", 4.);
        assert!(active(&d, "oscA.detune"));
        assert!(active(&d, "oscA.spread"));
        assert_eq!(value(&d, "oscA.detune"), 8.);
    }
    #[test]
    fn routing_keeps_active_slots_and_one_empty_slot_accessible() {
        let mut d = details();
        assert!(group_visible(&d, "route0"));
        assert!(!group_visible(&d, "route1"));
        assert!(!active(&d, "mod.0.target"));
        set(&mut d, "mod.0.source", 1.);
        set(&mut d, "mod.6.source", 3.);
        assert!(active(&d, "mod.0.target"));
        assert!(group_visible(&d, "route1"));
        assert!(!group_visible(&d, "route2"));
        assert!(group_visible(&d, "route6"));
    }
}
