//! Navigate from a displayed plugin slot to that exact use's editable source configuration.
use crate::{plugin_details::DetailTarget, ui::Preview, window_manager::WindowId};
use gpui::*;

impl Preview {
    pub fn plugin_configuration_target(
        &self,
        target: &DetailTarget,
    ) -> Option<(String, Option<String>)> {
        let view = self.document.view.as_ref()?;
        let usage = view
            .plugins
            .iter()
            .flat_map(|p| &p.usages)
            .find(|usage| match target {
                DetailTarget::Instrument(owner) => {
                    usage.kind == "instrument" && &usage.owner == owner
                }
                DetailTarget::ChannelInsert(owner, index) => {
                    usage.kind == "channelInsert" && &usage.owner == owner && usage.index == *index
                }
                DetailTarget::BusInsert(owner, index) => {
                    usage.kind == "busInsert" && &usage.owner == owner && usage.index == *index
                }
            })?
            .handle();
        let site = view
            .configuration_sites
            .iter()
            .find(|s| s.scope == "reference" && s.usages.len() == 1 && s.usages[0].handle == usage)
            .or_else(|| {
                view.configuration_sites
                    .iter()
                    .find(|s| s.usages.len() == 1 && s.usages[0].handle == usage)
            })?;
        Some((
            site.handle.clone(),
            (site.scope == "reference").then_some(usage),
        ))
    }
    pub fn edit_plugin_source(
        &mut self,
        target: &DetailTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selection = self.document.view.as_ref().and_then(|view| {
            view.plugins.iter().find_map(|entry| {
                entry
                    .usages
                    .iter()
                    .find(|usage| match target {
                        DetailTarget::Instrument(owner) => {
                            usage.kind == "instrument" && &usage.owner == owner
                        }
                        DetailTarget::ChannelInsert(owner, index) => {
                            usage.kind == "channelInsert"
                                && &usage.owner == owner
                                && usage.index == *index
                        }
                        DetailTarget::BusInsert(owner, index) => {
                            usage.kind == "busInsert"
                                && &usage.owner == owner
                                && usage.index == *index
                        }
                    })
                    .map(|usage| {
                        (
                            entry.handle.clone(),
                            usage.handle(),
                            entry.parameters.len(),
                            entry.kind == "effect",
                        )
                    })
            })
        });
        let Some((plugin, usage, parameters, effect)) = selection else {
            return;
        };
        if self.document.configuration.plugin.as_ref() != Some(&plugin) {
            let mut bounds = self.document.windows.state(WindowId::Configuration).bounds;
            bounds.height =
                (132. + parameters as f32 * 34. + if effect { 68. } else { 0. }).clamp(210., 480.);
            self.document
                .windows
                .set_bounds(WindowId::Configuration, bounds);
        }
        let editor = &mut self.document.configuration;
        editor.open = true;
        editor.plugin = Some(plugin);
        editor.scroll.set_offset(point(px(0.), px(0.)));
        editor.selected = Some(usage);
        editor.shared = false;
        editor.input = None;
        self.document.windows.focus(WindowId::Configuration);
        self.workspace_focus.focus(window);
        cx.notify();
    }
}
