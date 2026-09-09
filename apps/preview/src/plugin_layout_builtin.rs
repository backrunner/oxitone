//! Bundled front panels and compact descriptor fallback for external plugins.
use crate::{
    plugin_details::PluginDetails,
    plugin_layout::{Control, Group, Layout, Page, PanelSize},
};
use oxitone_core::wire::ParameterUnit;

pub fn panel(details: &PluginDetails) -> Layout {
    let descriptor = &details.info.descriptor;
    let bundled = details.info.library.is_none();
    let pages = if descriptor.plugin_id == "oxitone.wavetable"
        && descriptor.plugin_version == "1.0.0"
        && bundled
    {
        crate::plugin_synth_layout::pages()
    } else if let Some(pages) = bundled
        .then(|| crate::plugin_effect_layout::pages(descriptor))
        .flatten()
    {
        pages
    } else if let Some(pages) = bundled
        .then(|| crate::plugin_builtin_controls::sampler_pages(descriptor))
        .flatten()
    {
        pages
    } else {
        let mut groups: Vec<Group> = Vec::new();
        for spec in &descriptor.parameters {
            let group = spec
                .id
                .rsplit_once('.')
                .map_or("Controls", |(group, _)| group);
            // A descriptor's label is authoritative, including external dotted IDs.
            let label = spec.label.as_str();
            if !groups.iter().any(|g| g.id == group) {
                groups.push(Group {
                    id: group.into(),
                    title: crate::plugin_group_names::title(group),
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
    let wide = pages.iter().any(|page| page.groups.len() > 1)
        || matches!(
            descriptor.plugin_id.as_str(),
            "oxitone.limit" | "oxitone.limiter"
        );
    Layout {
        ui_version: "1.0".into(),
        plugin_id: descriptor.plugin_id.clone(),
        plugin_version: descriptor.plugin_version.clone(),
        title: details.name.clone(),
        size: if bundled
            && descriptor.plugin_version == "1.0.0"
            && descriptor.plugin_id == "oxitone.wavetable"
        {
            PanelSize {
                width: 1120,
                height: 680,
            }
        } else {
            PanelSize {
                width: if !bundled {
                    700
                } else if count > 8 {
                    840
                } else if count > 4 || wide {
                    760
                } else {
                    640
                },
                height: if bundled {
                    if pages.iter().any(|page| page.groups.len() > 2) {
                        660
                    } else {
                        520
                    }
                } else {
                    500
                },
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
