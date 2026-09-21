use super::*;

fn catalog(input: usize, notes: bool) -> Catalog {
    Catalog {
        bundle_path: "/Fixture.vst3".into(),
        class_id: "1".repeat(32),
        input_channels: Some(input),
        output_channels: Some(2),
        note_input: Some(notes),
        parameters: vec![],
        render: None,
        preset_path: None,
    }
}

#[test]
fn form_requires_paths_and_bounded_finite_duration() {
    let mut draft = Draft::default();
    let effect = catalog(2, false);
    assert!(draft.options(&effect).is_err());
    draft.output = "/out.wav".into();
    assert!(draft.options(&effect).is_err());
    draft.input = "/in.wav".into();
    assert_eq!(draft.options(&effect).unwrap().frames, 48000);
    for seconds in ["NaN", "inf", "0", "-1", "601", ""] {
        draft.seconds = seconds.into();
        assert!(draft.options(&effect).is_err());
    }
    draft.seconds = "0.001".into();
    draft.tail = "60.1".into();
    assert!(draft.options(&effect).is_err());
    draft.tail = "0".into();
    draft.tempo = "19".into();
    assert!(draft.options(&effect).is_err());
    draft.tempo = "999".into();
    assert_eq!(draft.options(&effect).unwrap().frames, 48);
}

#[test]
fn test_note_is_bounded_and_wire_contains_no_configuration() {
    let mut draft = Draft {
        output: "/out.wav".into(),
        seconds: "0.001".into(),
        ..Default::default()
    };
    let instrument = catalog(0, true);
    for pitch in ["-1", "128", "60.5", "NaN"] {
        draft.pitch = pitch.into();
        assert!(draft.options(&instrument).is_err());
    }
    draft.pitch = "127".into();
    let options = draft.options(&instrument).unwrap();
    assert!(options.input_path.is_none());
    assert_eq!(options.events[0]["frame"], 0);
    assert_eq!(options.events[1]["frame"], 47);
    assert_eq!(options.events[1]["pitch"], 127);
    let wire = serde_json::to_value(Command::Render {
        plugin: "fixture".into(),
        options,
    })
    .unwrap();
    assert_eq!(wire["kind"], "render");
    assert_eq!(wire["options"]["sampleRate"], 48000);
    assert!(wire["options"].get("inputPath").is_none());
    assert!(wire["options"].get("configuration").is_none());
    assert!(draft.options(&catalog(0, false)).unwrap().events.is_empty());
}
