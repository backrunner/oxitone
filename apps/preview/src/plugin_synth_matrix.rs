use super::{choice, group, knob, Page};

pub fn page() -> Page {
    Page {
        id: "matrix".into(),
        title: "Matrix".into(),
        groups: std::iter::once(group(
            "macros",
            "Macros",
            4,
            (1..=4)
                .map(|i| knob(&format!("macro{i}"), &format!("Macro {i}")))
                .collect(),
        ))
        .chain((0..8).map(|i| {
            group(
                &format!("route{i}"),
                &format!("Route {}", i + 1),
                2,
                vec![
                    choice(
                        &format!("mod.{i}.source"),
                        "Source",
                        &[
                            "Off",
                            "LFO 1",
                            "LFO 2",
                            "Amp ENV",
                            "Filter ENV",
                            "Mod ENV",
                            "Velocity",
                            "Key",
                            "Random",
                            "Macro 1",
                            "Macro 2",
                            "Macro 3",
                            "Macro 4",
                        ],
                    ),
                    choice(
                        &format!("mod.{i}.target"),
                        "Target",
                        &[
                            "Pitch",
                            "Cutoff",
                            "A position",
                            "B position",
                            "A warp",
                            "B warp",
                            "FM",
                            "Ring",
                            "A/B mix",
                            "Pan",
                            "Level",
                        ],
                    ),
                    knob(&format!("mod.{i}.amount"), "Amount"),
                    knob(&format!("mod.{i}.curve"), "Curve"),
                ],
            )
        }))
        .collect(),
    }
}
