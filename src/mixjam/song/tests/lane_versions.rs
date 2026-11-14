use super::*;

#[test]
fn versions_before_17_reject_a_lane_ducker() {
    let mut project = project();
    project.tracks.iter_mut().for_each(before_version_18);
    let mut document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    document["formatVersion"] = 16.into();
    let reopened: Document = serde_json::from_value(document.clone()).unwrap();
    assert_eq!(
        reopened.into_project().unwrap_err(),
        "Version 16 lanes cannot contain ducker."
    );
    document["lanes"][5]
        .as_object_mut()
        .unwrap()
        .remove("ducker");
    let reopened: Document = serde_json::from_value(document.clone()).unwrap();
    let mut expected = project;
    expected.tracks[5].ducker = Default::default();
    assert_eq!(reopened.into_project().unwrap(), expected);
    // The pedals' keys are new in version 17 too.
    document["master"]["limiter"]["key"] = 3.into();
    let reopened: Document = serde_json::from_value(document).unwrap();
    assert_eq!(
        reopened.into_project().unwrap_err(),
        "Version 16 songs cannot contain key or duckKey."
    );
}

#[test]
fn automation_records_reject_positional_arrays_in_every_supported_version() {
    // Serde can read a struct from a positional array. Such an array
    // must not hide version 20's pan lane, version 21's repeat lane,
    // version 23's repeat gate lane, or version 24's pitch lane from an
    // earlier reader.
    let document = serde_json::to_value(Document::new(
        &Project::default(),
        timestamp(0),
        timestamp(0),
    ))
    .unwrap();
    for version in 14..=FORMAT_VERSION {
        let mut document = document.clone();
        document["formatVersion"] = version.into();
        if version < 16 {
            with_earlier_booster_keys(&mut document);
        }
        document["lanes"][0]["automation"] = serde_json::json!({});
        assert!(parse(&serde_json::to_vec(&document).unwrap()).is_ok());
        for automation in [
            serde_json::json!([[], [[], [], [], []], []]),
            serde_json::json!([[], [[], [], [], []], [], []]),
            serde_json::json!([[], [[], [], [], []], [], [{"tick": 0, "value": 1.0}]]),
            serde_json::json!([[], [[], [], [], []], [], [], []]),
            serde_json::json!([[], [[], [], [], []], [], [], [{"tick": 0, "value": 16.0}]]),
            serde_json::json!([[], [[], [], [], []], [], [], [], []]),
            serde_json::json!([[], [[], [], [], []], [], [], [], [{"tick": 0, "value": 50.0}]]),
            serde_json::json!([[], [[], [], [], []], [], [], [], [], []]),
            serde_json::json!([
                [],
                [[], [], [], []],
                [],
                [],
                [],
                [],
                [{"tick": 0, "value": -1200.0}]
            ]),
        ] {
            document["lanes"][0]["automation"] = automation;
            assert_eq!(
                parse(&serde_json::to_vec(&document).unwrap()).unwrap_err(),
                "The automation must be a JSON object.",
                "version {version}"
            );
        }
    }
}

#[test]
fn song_automation_points_must_be_objects() {
    let document = serde_json::to_value(Document::new(
        &Project::default(),
        timestamp(0),
        timestamp(0),
    ))
    .unwrap();
    for (pointer, points) in [
        ("/fxBuses/0/returnAutomation", serde_json::json!([[0, 1.0]])),
        (
            "/lanes/0/automation",
            serde_json::json!({"repeatGate": [[0, 50.0]]}),
        ),
        (
            "/lanes/0/automation",
            serde_json::json!({"volume": [[0, -6.0]]}),
        ),
        (
            "/lanes/0/automation",
            serde_json::json!({"sends": [[[0, 0.5]], [], [], []]}),
        ),
    ] {
        let mut wrong = document.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        wrong.pointer_mut(parent).unwrap()[key] = points;
        let error = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
        assert!(
            error.contains("points must be JSON objects"),
            "{pointer}: {error}"
        );
    }
}

#[test]
fn only_version_23_songs_hold_return_lanes_repeat_gates_and_reversed_clips() {
    let project = thrown(project());
    let document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    assert_eq!(document["formatVersion"], 24);
    let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(reopened, project);
    // Version 22 and earlier reject each key, even when it holds nothing.
    let empty = [
        ("/fxBuses/1/returnAutomation", serde_json::json!([])),
        ("/lanes/5/automation/repeatGate", serde_json::json!([])),
        ("/lanes/63/placements/0/reverse", serde_json::json!(false)),
    ];
    let full = [
        (
            "/fxBuses/1/returnAutomation",
            document["fxBuses"][1]["returnAutomation"].clone(),
        ),
        (
            "/lanes/5/automation/repeatGate",
            document["lanes"][5]["automation"]["repeatGate"].clone(),
        ),
        ("/lanes/63/placements/0/reverse", serde_json::json!(true)),
    ];
    let without = |document: &serde_json::Value| {
        let mut earlier = document.clone();
        earlier["fxBuses"][1]
            .as_object_mut()
            .unwrap()
            .remove("returnAutomation");
        earlier["lanes"][5]["automation"]
            .as_object_mut()
            .unwrap()
            .remove("repeatGate");
        earlier["lanes"][63]["placements"][0]
            .as_object_mut()
            .unwrap()
            .remove("reverse");
        earlier
    };
    for version in [18, 21, 22] {
        for (index, (pointer, value)) in empty.iter().chain(&full).enumerate() {
            let mut earlier = without(&document);
            earlier["formatVersion"] = version.into();
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            earlier.pointer_mut(parent).unwrap()[key] = value.clone();
            let error = parse(&serde_json::to_vec(&earlier).unwrap()).unwrap_err();
            let expected = match index % 3 {
                0 => format!("Version {version} songs cannot contain FX return automation."),
                1 => format!("Version {version} lanes cannot contain repeat gate automation."),
                _ => format!("Version {version} placements cannot contain reverse."),
            };
            assert_eq!(error, expected, "{pointer} {value}");
        }
    }
    // Without the keys, version 22 opens with a static return, full-length
    // repeats, and the clip forward.
    let mut earlier = without(&document);
    earlier["formatVersion"] = 22.into();
    let mut expected = project.clone();
    expected.return_automation = Default::default();
    expected.tracks[5].automation.repeat_gate.clear();
    expected.clips[0].reverse = false;
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap()).unwrap(),
        expected
    );
    // Version 23 takes empty lanes as none and a false switch as forward.
    let mut quiet = document.clone();
    for (pointer, value) in &empty {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        quiet.pointer_mut(parent).unwrap()[key] = value.clone();
    }
    let mut forward = project.clone();
    forward.return_automation = Default::default();
    forward.tracks[5].automation.repeat_gate.clear();
    forward.clips[0].reverse = false;
    assert_eq!(
        parse(&serde_json::to_vec(&quiet).unwrap()).unwrap(),
        forward
    );
    // Each holds its range and form.
    for (pointer, value, error) in [
        (
            "/fxBuses/1/returnAutomation/2/value",
            serde_json::json!(2.5),
            "FX return automation values",
        ),
        (
            "/fxBuses/1/returnAutomation/0/value",
            serde_json::json!(-0.1),
            "FX return automation values",
        ),
        (
            "/fxBuses/1/returnAutomation/1/tick",
            serde_json::json!(0),
            "increasing tick order",
        ),
        (
            "/fxBuses/1/returnAutomation/1/level",
            serde_json::json!(1),
            "unknown field",
        ),
        (
            "/fxBuses/1/returnAutomation",
            serde_json::json!({}),
            "return automation is invalid",
        ),
        (
            "/fxBuses/1/returnAutomation",
            serde_json::Value::Null,
            "return automation is invalid",
        ),
        (
            "/lanes/5/automation/repeatGate/0/value",
            serde_json::json!(4),
            "Repeat gate automation values",
        ),
        (
            "/lanes/5/automation/repeatGate/1/value",
            serde_json::json!(101),
            "Repeat gate automation values",
        ),
        (
            "/lanes/5/automation/repeatGate/1/tick",
            serde_json::json!(256),
            "increasing tick order",
        ),
        (
            "/lanes/63/placements/0/reverse",
            serde_json::Value::Null,
            "invalid type",
        ),
        (
            "/lanes/63/placements/0/reverse",
            serde_json::json!(1),
            "invalid type",
        ),
    ] {
        let mut wrong = document.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        match wrong.pointer_mut(parent).unwrap() {
            serde_json::Value::Array(items) => items[key.parse::<usize>().unwrap()] = value,
            record => record[key] = value,
        }
        let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
        assert!(found.contains(error), "{pointer}: {found}");
    }
    // A duplicate return lane in one bus is rejected too.
    let text = serde_json::to_string(&document).unwrap();
    let duplicate = text.replacen(
        "\"returnAutomation\":[",
        "\"returnAutomation\":[],\"returnAutomation\":[",
        1,
    );
    assert_ne!(duplicate, text);
    assert!(
        parse(duplicate.as_bytes())
            .unwrap_err()
            .contains("duplicate field")
    );
}

#[test]
fn only_version_20_lanes_hold_pan_automation() {
    let project = panned(project());
    let document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(
        reopened.tracks[5].automation.pan,
        project.tracks[5].automation.pan
    );
    // Version 19 rejects the key, even as an empty list, and opens the
    // same lane without it.
    for pan in [
        document["lanes"][5]["automation"]["pan"].clone(),
        serde_json::json!([]),
    ] {
        let mut earlier = document.clone();
        earlier["formatVersion"] = 19.into();
        earlier["lanes"][5]["automation"]["pan"] = pan;
        assert_eq!(
            parse(&serde_json::to_vec(&earlier).unwrap()).unwrap_err(),
            "Version 19 lanes cannot contain pan automation."
        );
    }
    let mut earlier = document.clone();
    earlier["formatVersion"] = 19.into();
    earlier["lanes"][5]["automation"]
        .as_object_mut()
        .unwrap()
        .remove("pan");
    let mut expected = project.clone();
    expected.tracks[5].automation.pan.clear();
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap())
            .unwrap()
            .tracks,
        expected.tracks
    );
    // Version 20 takes an empty list as no lane, and checks the values.
    let mut empty = document.clone();
    empty["lanes"][5]["automation"]["pan"] = serde_json::json!([]);
    assert!(
        parse(&serde_json::to_vec(&empty).unwrap()).unwrap().tracks[5]
            .automation
            .pan
            .is_empty()
    );
    for (value, error) in [
        (serde_json::json!(-1.5), "Pan automation values"),
        (serde_json::json!(null), "The automation is invalid"),
    ] {
        let mut wrong = document.clone();
        wrong["lanes"][5]["automation"]["pan"][1]["value"] = value;
        let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
        assert!(found.contains(error), "{found}");
    }
    let mut wrong = document;
    wrong["lanes"][5]["automation"]["pan"][1]["tick"] = 0.into();
    let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
    assert!(found.contains("increasing tick order"), "{found}");
}

#[test]
fn only_version_24_lanes_hold_pitch_automation() {
    let project = stopped(project());
    let document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    assert_eq!(document["formatVersion"], 24);
    let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(reopened.tracks, project.tracks);
    // Version 23 rejects the key, even as an empty list, and opens the
    // same lane without it.
    for pitch in [
        document["lanes"][5]["automation"]["pitch"].clone(),
        serde_json::json!([]),
    ] {
        let mut earlier = document.clone();
        earlier["formatVersion"] = 23.into();
        earlier["lanes"][5]["automation"]["pitch"] = pitch;
        assert_eq!(
            parse(&serde_json::to_vec(&earlier).unwrap()).unwrap_err(),
            "Version 23 lanes cannot contain pitch automation."
        );
    }
    let mut earlier = document.clone();
    earlier["formatVersion"] = 23.into();
    earlier["lanes"][5]["automation"]
        .as_object_mut()
        .unwrap()
        .remove("pitch");
    let mut expected = project.clone();
    expected.tracks[5].automation.pitch.clear();
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap())
            .unwrap()
            .tracks,
        expected.tracks
    );
    // Version 24 takes an empty list as no lane, and any value in the
    // range, but none outside it.
    let mut empty = document.clone();
    empty["lanes"][5]["automation"]["pitch"] = serde_json::json!([]);
    assert!(
        parse(&serde_json::to_vec(&empty).unwrap()).unwrap().tracks[5]
            .automation
            .pitch
            .is_empty()
    );
    for value in [-4800.0, -1234.5, 0.0, 2400.0] {
        let mut each = document.clone();
        each["lanes"][5]["automation"]["pitch"][1]["value"] = value.into();
        assert!(
            parse(&serde_json::to_vec(&each).unwrap()).is_ok(),
            "{value}"
        );
    }
    for (value, error) in [
        (serde_json::json!(-4801.0), "Pitch automation values"),
        (serde_json::json!(2401.0), "Pitch automation values"),
        (serde_json::json!(null), "The automation is invalid"),
    ] {
        let mut wrong = document.clone();
        wrong["lanes"][5]["automation"]["pitch"][1]["value"] = value;
        let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
        assert!(found.contains(error), "{found}");
    }
    let mut wrong = document;
    wrong["lanes"][5]["automation"]["pitch"][1] = serde_json::json!([832, -1200.0]);
    assert_eq!(
        parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err(),
        "The automation points must be JSON objects."
    );
}

#[test]
fn only_version_21_lanes_hold_repeat_automation() {
    let project = rolled(panned(project()));
    let document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    assert_eq!(document["formatVersion"], 24);
    let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(reopened.tracks, project.tracks);
    // Version 20 rejects the key, even as an empty list, and opens the
    // same lane without it.
    for repeat in [
        document["lanes"][5]["automation"]["repeat"].clone(),
        serde_json::json!([]),
    ] {
        let mut earlier = document.clone();
        earlier["formatVersion"] = 20.into();
        earlier["lanes"][5]["automation"]["repeat"] = repeat;
        assert_eq!(
            parse(&serde_json::to_vec(&earlier).unwrap()).unwrap_err(),
            "Version 20 lanes cannot contain repeat automation."
        );
    }
    let mut earlier = document.clone();
    earlier["formatVersion"] = 20.into();
    earlier["lanes"][5]["automation"]
        .as_object_mut()
        .unwrap()
        .remove("repeat");
    let mut expected = project.clone();
    expected.tracks[5].automation.repeat.clear();
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap())
            .unwrap()
            .tracks,
        expected.tracks
    );
    // Version 21 takes an empty list as no lane, and every division,
    // 1/4 included, but no value between them.
    let mut empty = document.clone();
    empty["lanes"][5]["automation"]["repeat"] = serde_json::json!([]);
    assert!(
        parse(&serde_json::to_vec(&empty).unwrap()).unwrap().tracks[5]
            .automation
            .repeat
            .is_empty()
    );
    for value in [0.0, 4.0, 8.0, 16.0, 32.0, 64.0] {
        let mut each = document.clone();
        each["lanes"][5]["automation"]["repeat"][1]["value"] = value.into();
        assert!(
            parse(&serde_json::to_vec(&each).unwrap()).is_ok(),
            "{value}"
        );
    }
    for (value, error) in [
        (serde_json::json!(12.0), "Repeat automation values"),
        (serde_json::json!(128.0), "Repeat automation values"),
        (serde_json::json!(-8.0), "Repeat automation values"),
        (serde_json::json!(2.0), "Repeat automation values"),
        (serde_json::json!(null), "The automation is invalid"),
    ] {
        let mut wrong = document.clone();
        wrong["lanes"][5]["automation"]["repeat"][1]["value"] = value;
        let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
        assert!(found.contains(error), "{found}");
    }
    let mut wrong = document;
    wrong["lanes"][5]["automation"]["repeat"][1]["tick"] = 256.into();
    let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
    assert!(found.contains("increasing tick order"), "{found}");
}

#[test]
fn drive_null_is_not_an_absent_setting_in_any_song_version() {
    let document = serde_json::to_value(Document::new(
        &Project::default(),
        timestamp(0),
        timestamp(0),
    ))
    .unwrap();
    for version in 8..=FORMAT_VERSION {
        let mut document = document.clone();
        document["formatVersion"] = version.into();
        if version < 11 {
            document.as_object_mut().unwrap().remove("master");
        } else if version < 16 {
            with_earlier_booster_keys(&mut document);
            if version == 11 {
                let board = document["master"].as_object_mut().unwrap();
                board.retain(|key, _| {
                    ["order", "ejayEq", "ejayCompressor", "ejayWide"].contains(&key.as_str())
                });
                board["order"].as_array_mut().unwrap().truncate(3);
            }
        }
        if version < 13 {
            document.as_object_mut().unwrap().remove("fxBuses");
        }
        for lane in document["lanes"].as_array_mut().unwrap() {
            let lane = lane.as_object_mut().unwrap();
            if version < 9 {
                lane.remove("polarityInverted");
            }
            if version < 10 {
                lane.remove("muted");
                lane.remove("solo");
            }
            if version < 13 {
                lane.remove("sends");
            }
        }
        assert!(parse(&serde_json::to_vec(&document).unwrap()).is_ok());
        document["lanes"][0]["drive"] = serde_json::Value::Null;
        assert!(
            parse(&serde_json::to_vec(&document).unwrap()).is_err(),
            "version {version} accepted a null drive"
        );
    }
}

#[test]
fn only_version_22_lanes_hold_a_drive() {
    let project = driven(project());
    let document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    assert_eq!(document["formatVersion"], 24);
    let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(reopened.tracks, project.tracks);
    // Version 21 and earlier reject the key, on or off, and open the
    // same lanes without it with every drive off.
    for version in [18, 20, 21] {
        for lane in [5, 8] {
            let mut earlier = document.clone();
            earlier["formatVersion"] = version.into();
            earlier["lanes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .enumerate()
                .filter(|(index, _)| *index != lane)
                .for_each(|(_, other)| {
                    other.as_object_mut().unwrap().remove("drive");
                });
            assert_eq!(
                parse(&serde_json::to_vec(&earlier).unwrap()).unwrap_err(),
                format!("Version {version} lanes cannot contain drive.")
            );
        }
    }
    let mut earlier = document.clone();
    earlier["formatVersion"] = 21.into();
    for lane in earlier["lanes"].as_array_mut().unwrap() {
        lane.as_object_mut().unwrap().remove("drive");
    }
    let mut expected = project.clone();
    for track in &mut expected.tracks {
        track.drive = Default::default();
    }
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap())
            .unwrap()
            .tracks,
        expected.tracks
    );
    // Version 22 needs every key, knows no other, and holds each to its
    // range.
    for (pointer, value, error) in [
        (
            "/lanes/5/drive/amount",
            serde_json::json!(25),
            "drive setting",
        ),
        ("/lanes/5/drive/mix", serde_json::json!(-1), "drive setting"),
        (
            "/lanes/5/drive/focus",
            serde_json::json!(10),
            "drive setting",
        ),
        (
            "/lanes/5/drive/output",
            serde_json::json!(13),
            "drive setting",
        ),
        (
            "/lanes/5/drive/character",
            serde_json::json!(101),
            "drive setting",
        ),
        ("/lanes/5/drive/on", serde_json::json!(1), "invalid type"),
    ] {
        let mut wrong = document.clone();
        *wrong.pointer_mut(pointer).unwrap() = value;
        let found = parse(&serde_json::to_vec(&wrong).unwrap()).unwrap_err();
        assert!(found.contains(error), "{pointer}: {found}");
    }
    let mut missing = document.clone();
    missing["lanes"][5]["drive"]
        .as_object_mut()
        .unwrap()
        .remove("focus");
    let found = parse(&serde_json::to_vec(&missing).unwrap()).unwrap_err();
    assert!(found.contains("missing field `focus`"), "{found}");
    let mut unknown = document;
    unknown["lanes"][5]["drive"]["tone"] = 1.into();
    let found = parse(&serde_json::to_vec(&unknown).unwrap()).unwrap_err();
    assert!(found.contains("unknown field `tone`"), "{found}");
}

#[test]
fn only_version_19_songs_hold_sections() {
    let project = sectioned(project());
    let mut document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    document["formatVersion"] = 18.into();
    let reopened: Document = serde_json::from_value(document.clone()).unwrap();
    assert_eq!(
        reopened.into_project().unwrap_err(),
        "Version 18 songs cannot contain sections."
    );
    // A song without sections saves no key.
    let plain = Project {
        sections: Vec::new(),
        ..project
    };
    let document = serde_json::to_value(Document::new(&plain, timestamp(0), timestamp(0))).unwrap();
    assert!(document["song"].get("sections").is_none());
    for version in [18, 19] {
        let mut explicit_empty = document.clone();
        explicit_empty["formatVersion"] = version.into();
        explicit_empty["song"]["sections"] = serde_json::json!([]);
        let result = parse(&serde_json::to_vec(&explicit_empty).unwrap());
        if version == 18 {
            assert_eq!(
                result.unwrap_err(),
                "Version 18 songs cannot contain sections."
            );
        } else {
            assert!(result.unwrap().sections.is_empty());
        }
        explicit_empty["song"]["sections"] = serde_json::Value::Null;
        assert!(parse(&serde_json::to_vec(&explicit_empty).unwrap()).is_err());
    }
    // Sections out of order, or with an unusable name, are not a song.
    for (sections, error) in [
        (
            serde_json::json!([{"startTick": 512, "name": "B"}, {"startTick": 256, "name": "A"}]),
            "each after the one before",
        ),
        (
            serde_json::json!([{"startTick": 0, "name": " Intro"}]),
            "1 to 48 characters",
        ),
        (
            serde_json::json!([{"startTick": 0, "name": ""}]),
            "1 to 48 characters",
        ),
        (
            serde_json::json!([{"startTick": 0, "name": "a
b"}]),
            "1 to 48 characters",
        ),
    ] {
        let mut document = document.clone();
        document["song"]["sections"] = sections;
        let reopened: Document = serde_json::from_value(document).unwrap();
        let error_text = reopened
            .into_project()
            .and_then(|p| p.validate().map(|()| p));
        assert!(
            error_text.as_ref().is_err_and(|text| text.contains(error)),
            "{error_text:?}"
        );
    }
}
