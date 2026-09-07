use crate::{plugin_window::PluginWindow, plugin_window_view::field};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow) -> Div {
    let theme = this.theme;
    let details = this.details.as_ref().unwrap();
    let mut root = div().w_full().p_4().flex().flex_col().gap_3();
    if details.resources.is_empty() {
        root = root.child(
            div()
                .text_sm()
                .text_color(rgb(theme.muted))
                .child("No sample resources"),
        );
    }
    for (key, id) in &details.resources {
        let mut block = div()
            .p_3()
            .rounded_md()
            .bg(rgb(theme.panel))
            .border_1()
            .border_color(rgb(theme.border))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(key.clone()),
            )
            .child(field(theme, "Reference", id.clone()));
        if let Some(sample) = this.project.snapshot.samples.iter().find(|s| s.id == *id) {
            block = block
                .child(field(theme, "Asset", sample.asset_uri.clone()))
                .child(field(
                    theme,
                    "Audio",
                    format!(
                        "{:?} · {} Hz · {} ch · {} frames",
                        sample.format, sample.sample_rate, sample.channels, sample.frames
                    ),
                ))
                .child(field(
                    theme,
                    "Duration",
                    format!(
                        "{:.3} s",
                        sample.frames as f64 / f64::from(sample.sample_rate)
                    ),
                ))
                .child(field(theme, "SHA-256", sample.sha256.clone()));
        }
        root = root.child(block);
    }
    root = root.child(
        div()
            .mt_2()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .child("Structured state"),
    );
    if let Some(state) = &details.state {
        // Inspect source state, including Slicer configuration; never decode assets on the UI thread.
        let json = serde_json::to_string_pretty(state).unwrap();
        root = root.child(
            div()
                .p_3()
                .rounded_md()
                .bg(rgb(theme.panel))
                .text_xs()
                .font_family("Menlo")
                .children(json.lines().map(|line| div().child(line.to_owned()))),
        );
    } else {
        root = root.child(div().text_sm().text_color(rgb(theme.muted)).child("None"));
    }
    root
}
