//! Plugin assignment lifecycle, including editing a newly added instance.
use super::*;

impl Smoke {
    pub(super) fn plugins(
        &mut self,
        view: &Entity<Preview>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        match self.stage {
            13 => {
                assert_eq!(
                    view.read(cx)
                        .project
                        .as_ref()
                        .unwrap()
                        .snapshot
                        .pattern_clips
                        .len(),
                    self.clips
                );
                view.update(cx, |s, cx| {
                    s.choose_plugin(
                        Slot {
                            owner: self.owner.clone(),
                            target: "channelInsert".into(),
                            instance: None,
                            label: "Keys · Add effect".into(),
                        },
                        window,
                    );
                    let handle = s
                        .document
                        .view
                        .as_ref()
                        .unwrap()
                        .plugins
                        .iter()
                        .find(|p| p.plugin_id == "oxitone.delay")
                        .unwrap()
                        .handle
                        .clone();
                    s.select_library_plugin(handle);
                    cx.notify();
                });
            }
            14 => {
                window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
            }
            15 => {
                let s = view.read(cx);
                let effects = &s.project.as_ref().unwrap().snapshot.channels[0].effect_chain;
                assert_eq!(effects.len(), 1);
                assert_eq!(effects[0].plugin_id, "oxitone.delay");
                self.instance = effects[0].instance_id.clone().unwrap();
                assert!(!s.document.manager.open);
                if !self.configured {
                    view.update(cx, |s, cx| {
                        s.edit_plugin_source(
                            &crate::plugin_details::DetailTarget::ChannelInsert(
                                self.owner.clone(),
                                0,
                            ),
                            window,
                            cx,
                        );
                        assert!(
                            s.configuration_site().is_some(),
                            "added plugin has an editable source boundary"
                        );
                        s.set_configuration_value("feedback".into(), false, 0.45);
                        cx.notify();
                    });
                    self.configured = true;
                    return false;
                }
                assert_eq!(effects[0].parameters.get("feedback"), Some(&0.45));
                view.update(cx, |s, cx| {
                    s.document.configuration.open = false;
                    s.choose_plugin(
                        Slot {
                            owner: self.owner.clone(),
                            target: "channelInsert".into(),
                            instance: Some(self.instance.clone()),
                            label: "Keys · Replace effect".into(),
                        },
                        window,
                    );
                    let handle = s
                        .document
                        .view
                        .as_ref()
                        .unwrap()
                        .plugins
                        .iter()
                        .find(|p| p.plugin_id == "oxitone.reverb")
                        .unwrap()
                        .handle
                        .clone();
                    s.select_library_plugin(handle);
                    cx.notify();
                });
            }
            16 => {
                window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
            }
            17 => {
                let effects =
                    &view.read(cx).project.as_ref().unwrap().snapshot.channels[0].effect_chain;
                assert_eq!(effects.len(), 1);
                assert_eq!(effects[0].plugin_id, "oxitone.reverb");
                assert_ne!(
                    effects[0].instance_id.as_deref(),
                    Some(self.instance.as_str())
                );
                self.instance = effects[0].instance_id.clone().unwrap();
                view.update(cx, |s, cx| {
                    s.remove_plugin_slot(&Slot {
                        owner: self.owner.clone(),
                        target: "channelInsert".into(),
                        instance: Some(self.instance.clone()),
                        label: "".into(),
                    });
                    cx.notify();
                });
            }
            18 => {
                assert!(view.read(cx).project.as_ref().unwrap().snapshot.channels[0]
                    .effect_chain
                    .is_empty());
                view.update(cx, |s, cx| {
                    s.document.windows.hidden = true;
                    cx.notify();
                });
            }
            _ => unreachable!(),
        }
        true
    }
}
