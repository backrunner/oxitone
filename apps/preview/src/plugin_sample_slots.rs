//! Explicit resource drop targets also work for multi-resource instruments.
use crate::plugin_window::PluginWindow;
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow) -> Div {
    this.sample_slots.borrow_mut().clear();
    let Some(details) = &this.details else {
        return div();
    };
    let id = details.info.descriptor.plugin_id.as_str();
    let keys: Vec<String> = match id {
        "oxitone.sampler" => vec!["sample".into()],
        "oxitone.slicer" => vec!["state.sampleId".into()],
        "oxitone.convolver" => vec!["impulse".into()],
        _ if id.starts_with("vst3.") => vec![],
        _ => details
            .resources
            .iter()
            .map(|(key, _)| key.clone())
            .collect(),
    };
    let t = this.theme;
    let mut row = div().flex().flex_wrap().gap_1().px_3();
    for key in keys {
        let bounds = this.sample_slots.clone();
        let label = format!(
            "↓ Drop audio · {}",
            if key == "state.sampleId" {
                "sample"
            } else {
                &key
            }
        );
        row = row.child(
            div()
                .relative()
                .my_1()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(t.panel))
                .border_1()
                .border_color(rgb(t.border))
                .text_size(px(10.))
                .text_color(rgb(t.muted))
                .child(label)
                .child(
                    canvas(
                        move |area, _, _| {
                            bounds.borrow_mut().insert(key.clone(), area);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                ),
        );
    }
    row
}
