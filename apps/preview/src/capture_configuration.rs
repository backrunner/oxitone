//! The explicit smoke fixture is a temporary project with a real npm/C plugin.
use crate::{
    configuration_edit::ConfigurationInput, document_wire::DocumentOperation, ui::Preview,
};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    revision: u64,
    review_frame: usize,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        self.stage == 8 || std::env::var_os("OXITONE_PREVIEW_CAPTURE_CONFIGURATION").is_none()
    }
    pub fn step(
        &mut self,
        frame: usize,
        view: &Entity<Preview>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        match self.stage {
            0 => {
                view.update(cx, |state, cx| {
                    let document = state.document.view.as_ref().unwrap();
                    self.revision = document.revision;
                    let plugin = document
                        .plugins
                        .iter()
                        .find(|entry| entry.plugin_id == "fixture.gain")
                        .unwrap();
                    let owner = state.project.as_ref().unwrap().snapshot.channels[0]
                        .id
                        .clone();
                    let rack = document
                        .rack_sites
                        .iter()
                        .find(|site| {
                            site.scope == "reference"
                                && site.usages.len() == 1
                                && site.usages[0].owner == owner
                        })
                        .unwrap();
                    let operation = DocumentOperation::PlanMaterializeRack {
                        site: rack.handle.clone(),
                        owner: Some(owner.clone()),
                    };
                    state.document.manager.selected = Some(plugin.handle.clone());
                    state.edit_plugin_source(
                        &crate::plugin_details::DetailTarget::ChannelInsert(owner, 1),
                        window,
                        cx,
                    );
                    state.document.automation.open = false;
                    state.document.show_code = false;
                    state.workspace_focus.focus(window);
                    state.document_request(operation);
                    cx.notify();
                });
                self.stage = 1;
            }
            1 if view
                .read(cx)
                .document
                .view
                .as_ref()
                .unwrap()
                .rack_materialization
                .is_some() =>
            {
                assert_eq!(
                    view.read(cx)
                        .document
                        .view
                        .as_ref()
                        .unwrap()
                        .rack_materialization
                        .as_ref()
                        .unwrap()
                        .effects,
                    2
                );
                self.review_frame = frame;
                self.stage = 2;
            }
            2 if frame > self.review_frame + 3 => {
                view.update(cx, |state, _| {
                    state.document_request(DocumentOperation::ConfirmMaterialize {
                        plan_id: state
                            .document
                            .view
                            .as_ref()
                            .unwrap()
                            .rack_materialization
                            .as_ref()
                            .unwrap()
                            .plan_id
                            .clone(),
                    });
                });
                self.stage = 3;
            }
            3 if view.read(cx).document.view.as_ref().unwrap().revision == self.revision + 1 => {
                view.update(cx, |state, cx| {
                    let site = state.configuration_site().unwrap();
                    state.document.configuration.input = Some(ConfigurationInput {
                        site: site.handle.clone(),
                        usage: Some(site.usages[0].handle.clone()),
                        parameter: "gain".into(),
                        host: false,
                        text: "0.9".into(),
                        select_all: false,
                    });
                    state.workspace_focus.focus(window);
                    cx.notify();
                });
                self.review_frame = frame;
                self.stage = 4;
            }
            4 if frame > self.review_frame + 2 => {
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                assert!(view.read(cx).document.pending.is_none());
                assert!(view.read(cx).document.configuration.input.is_some());
                window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
                assert!(view.read(cx).document.pending.is_some());
                self.stage = 5;
            }
            5 if view.read(cx).document.view.as_ref().unwrap().revision == self.revision + 2 => {
                let snapshot = &view.read(cx).project.as_ref().unwrap().snapshot;
                assert_eq!(snapshot.channels[0].effect_chain[1].parameters["gain"], 0.9);
                assert_eq!(snapshot.channels[1].effect_chain[1].parameters["gain"], 0.4);
                let owner = snapshot.channels[0].id.clone();
                let instance = snapshot.channels[0].effect_chain[1]
                    .instance_id
                    .clone()
                    .unwrap();
                view.update(cx, |state, _| {
                    state.document_request(
                        state.effect_order_operation(&owner, &instance, -1).unwrap(),
                    );
                });
                self.stage = 6;
            }
            6 if view.read(cx).document.view.as_ref().unwrap().revision == self.revision + 3 => {
                let snapshot = &view.read(cx).project.as_ref().unwrap().snapshot;
                assert_eq!(snapshot.channels[0].effect_chain[0].parameters["gain"], 0.9);
                assert_eq!(snapshot.channels[0].effect_chain[1].parameters["gain"], 0.7);
                assert_eq!(
                    snapshot.automation.last().unwrap().target.entity_id,
                    snapshot.channels[0].effect_chain[1]
                        .instance_id
                        .as_ref()
                        .unwrap()
                        .as_str()
                );
                assert_eq!(
                    view.read(cx)
                        .configuration_site()
                        .unwrap()
                        .config
                        .parameters["gain"],
                    0.9
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
                self.stage = 7;
                self.review_frame = frame;
            }
            7 if !view.read(cx).document.view.as_ref().unwrap().modified
                && frame > self.review_frame + 3 =>
            {
                eprintln!("Configuration smoke passed: real C plugin, rack review, confirmation, numeric input, isolated parameter edit, instance reorder, typed binding, native projection, Save TS");
                self.stage = 8;
            }
            _ => {}
        }
    }
}
