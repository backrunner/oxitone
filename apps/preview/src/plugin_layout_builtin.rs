//! Compact descriptor-based fallback, with a purpose-built Wavetable front panel.
use crate::{
    plugin_details::PluginDetails,
    plugin_layout::{Control, Group, Layout, Page, PanelSize},
};
use oxitone_core::wire::ParameterUnit;

pub fn panel(details: &PluginDetails) -> Layout {
    let descriptor = &details.info.descriptor;
    let pages = if descriptor.plugin_id == "oxitone.wavetable"
        && descriptor.plugin_version == "1.0.0"
    {
        crate::plugin_synth_layout::pages()
    } else if let Some(pages) = crate::plugin_effect_layout::pages(descriptor) {
        pages
    } else {
        let mut groups: Vec<Group> = Vec::new();
        for spec in &descriptor.parameters {
            let (group, label) = spec
                .id
                .rsplit_once('.')
                .unwrap_or(("Controls", &spec.label));
            if !groups.iter().any(|g| g.id == group) {
                groups.push(Group {
                    id: group.into(),
                    title: group.into(),
                    columns: 4,
                    controls: vec![],
                });
            }
            let control = if spec.unit == ParameterUnit::Enum && spec.min == 0. && spec.max == 1. {
                Control::Toggle {
                    parameter: spec.id.clone(),
                    label: Some(label.into()),
                }
            } else if spec.unit == ParameterUnit::Enum {
                Control::Readout {
                    parameter: spec.id.clone(),
                    label: Some(label.into()),
                }
            } else {
                knob(&spec.id, label)
            };
            groups
                .iter_mut()
                .find(|g| g.id == group)
                .unwrap()
                .controls
                .push(control);
        }
        for group in &mut groups {
            group.columns = (group.controls.len() as u32).clamp(1, 4);
        }
        vec![Page {
            id: "main".into(),
            title: "Controls".into(),
            groups,
        }]
    };
    let count = descriptor.parameters.len();
    Layout {
        ui_version: "1.0".into(),
        plugin_id: descriptor.plugin_id.clone(),
        plugin_version: descriptor.plugin_version.clone(),
        title: details.name.clone(),
        size: if descriptor.plugin_id == "oxitone.wavetable" {
            PanelSize {
                width: 1120,
                height: 680,
            }
        } else {
            PanelSize {
                width: if count > 8 { 720 } else { 520 },
                height: (230 + count.div_ceil(4) as u32 * 94).clamp(280, 640),
            }
        },
        pages,
    }
}
fn knob(id: &str, label: &str) -> Control {
    Control::Knob {
        parameter: id.into(),
        label: Some(label.into()),
    }
}
