//! Native editor actions are document requests; the UI never obtains a DSP control handle.
use crate::{
    plugin_window::PluginWindow,
    vst3_model::{Command, LiveAction},
};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> Div {
    let enabled = this.owner.upgrade().is_some_and(|owner| {
        let owner = owner.read(cx);
        owner.document_ready() && owner.plugin_configuration_target(&this.target).is_some()
    });
    let mut row = div().flex().flex_wrap().gap_2();
    for (id, label, action) in [
        (
            "vst3-instance-open",
            "Open native editor",
            Some(LiveAction::OpenEditor),
        ),
        (
            "vst3-instance-close",
            "Close native editor",
            Some(LiveAction::CloseEditor),
        ),
        ("vst3-instance-capture", "Use current state", None),
    ] {
        row = row.child(
            this.theme
                .ghost(id, label)
                .opacity(if enabled { 1. } else { 0.4 })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !enabled {
                        return;
                    }
                    let target = this.target.clone();
                    let identity = this.identity.clone();
                    let _ = this.owner.update(cx, |owner, cx| {
                        if owner.document_ready()
                            && owner.project.as_ref().is_some_and(|p| {
                                crate::plugin_identity::key(p, &target) == identity
                            })
                        {
                            if let Some((site, usage)) = owner.plugin_configuration_target(&target)
                            {
                                let command = match action {
                                    Some(action) => Command::ControlInstance {
                                        site,
                                        usage,
                                        action,
                                    },
                                    None => Command::CaptureInstance { site, usage },
                                };
                                owner.document_request(
                                    crate::document_wire::DocumentOperation::Vst3 { command },
                                );
                            }
                        }
                        cx.notify();
                    });
                })),
        );
    }
    row
}
