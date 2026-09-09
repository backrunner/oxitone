//! Two-level lane navigation: select a destination, then its musical parameter.
use crate::{automation_labels::TargetLabel, ui::Preview};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> Div {
    let t = this.theme;
    let Some(project) = &this.project else {
        return div();
    };
    let selected = this.automation_lane().map(|lane| lane.id.as_str());
    let lanes: Vec<_> = project
        .snapshot
        .automation
        .iter()
        .map(|lane| (lane.id.clone(), project.automation_target(lane)))
        .collect();
    let active = lanes.iter().find(|(id, _)| Some(id.as_str()) == selected);
    let mut targets: Vec<&TargetLabel> = Vec::new();
    for (_, label) in &lanes {
        if !targets.iter().any(|target| target.key == label.key) {
            targets.push(label);
        }
    }
    let mut owners = div()
        .id("automation-targets")
        .min_w_0()
        .overflow_x_scroll()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1();
    for target in &targets {
        let selected_target = active.is_some_and(|(_, label)| label.key == target.key);
        let id = lanes
            .iter()
            .find(|(_, label)| label.key == target.key)
            .unwrap()
            .0
            .clone();
        let duplicate = targets
            .iter()
            .filter(|other| other.context() == target.context())
            .count()
            > 1;
        let ordinal = targets
            .iter()
            .take_while(|other| other.key != target.key)
            .filter(|other| other.context() == target.context())
            .count()
            + 1;
        let owner = if duplicate {
            format!("{} ({ordinal})", target.owner)
        } else {
            target.owner.clone()
        };
        let mut label = div().flex().items_center().gap_2().child(owner);
        if let Some(device) = &target.device {
            label = label.child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .child(device.clone()),
            );
        }
        owners = owners.child(
            t.tool(
                format!("automation-target-{}", target.key),
                "",
                selected_target,
            )
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| select(this, &id, cx))),
        );
    }
    let mut parameters = div()
        .id("automation-lanes")
        .min_w_0()
        .overflow_x_scroll()
        .flex()
        .items_center()
        .gap_1();
    if let Some((_, active)) = active {
        let members: Vec<_> = lanes
            .iter()
            .filter(|(_, label)| label.key == active.key)
            .collect();
        for (index, (id, label)) in members.iter().enumerate() {
            let count = members
                .iter()
                .filter(|(_, other)| other.parameter == label.parameter)
                .count();
            let ordinal = members[..index]
                .iter()
                .filter(|(_, other)| other.parameter == label.parameter)
                .count()
                + 1;
            let name = if count > 1 {
                format!("{} {ordinal}", label.parameter)
            } else {
                label.parameter.clone()
            };
            let id = id.clone();
            parameters = parameters.child(
                t.tool(format!("lane-{id}"), name, Some(id.as_str()) == selected)
                    .on_click(cx.listener(move |this, _, _, cx| select(this, &id, cx))),
            );
        }
    }
    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .border_b_1()
        .border_color(rgb(t.border))
        .when(targets.len() > 1, |d| d.child(owners))
        .child(
            div()
                .h(px(34.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .when(targets.len() == 1, |d| {
                    d.child(
                        div()
                            .min_w_0()
                            .max_w(px(160.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(rgb(t.muted))
                            .child(targets[0].context()),
                    )
                })
                .child(div().flex_1().min_w_0().child(parameters))
                .child(
                    t.tool(
                        "automation-scope",
                        if this.document.automation.shared {
                            "Shared"
                        } else {
                            "Lane"
                        },
                        this.document.automation.shared,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.document.automation.shared = !this.document.automation.shared;
                        this.document.automation.gesture = None;
                        cx.notify();
                    })),
                ),
        )
}

fn select(this: &mut Preview, id: &str, cx: &mut Context<Preview>) {
    this.document.automation.selected = Some(id.into());
    this.document.automation.gesture = None;
    cx.notify();
}
