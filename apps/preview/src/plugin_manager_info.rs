use crate::{plugin_manager::CatalogEntry, theme::Theme};
use gpui::{prelude::*, *};

pub fn information(theme: Theme, entry: &CatalogEntry) -> Div {
    let mut info = div().flex().flex_col().gap_2();
    let package = entry.package_name.as_ref().map(|name| {
        entry
            .package_version
            .as_ref()
            .map_or_else(|| name.clone(), |version| format!("{name}@{version}"))
    });
    for (label, value) in [
        ("Version", Some(entry.plugin_version.clone())),
        ("Vendor", Some(entry.vendor.clone())),
        ("Identifier", Some(entry.plugin_id.clone())),
        ("Validation", Some(entry.validation.clone())),
        ("Package", package),
        ("License", entry.license.clone()),
        ("Library", entry.library_path.clone()),
        ("SHA-256", entry.sha256.clone()),
    ] {
        let Some(value) = value.filter(|v| !v.is_empty()) else {
            continue;
        };
        info = info.child(
            div()
                .flex()
                .gap_3()
                .text_size(px(11.))
                .child(
                    div()
                        .w(px(80.))
                        .flex_shrink_0()
                        .text_color(rgb(theme.muted))
                        .child(label),
                )
                .child(div().flex_1().min_w_0().child(value)),
        );
    }
    info
}
