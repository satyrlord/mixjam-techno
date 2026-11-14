use super::*;

#[test]
fn timestamps_follow_the_gregorian_calendar() {
    assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
    assert_eq!(timestamp(951_782_400 + 86_399), "2000-02-29T23:59:59Z");
    assert_eq!(timestamp(1_791_676_800), "2026-10-11T00:00:00Z");
}

#[test]
fn zero_length_fades_keep_their_selected_curves_on_roundtrip() {
    let mut project = project();
    project.clips[0].fades = Fades {
        fade_in: crate::engine::Fade {
            ticks: 0,
            curve: crate::engine::FadeCurve::EqualPower,
        },
        fade_out: crate::engine::Fade {
            ticks: 0,
            curve: crate::engine::FadeCurve::Exponential,
        },
    };
    let document = Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        "2020-01-02T03:04:05Z".into(),
    );
    let saved = serde_json::to_vec(&document).unwrap();
    let reopened: Document = serde_json::from_slice(&saved).unwrap();
    assert_eq!(reopened.into_project().unwrap(), project);
}

#[test]
fn saved_song_is_a_version_24_document_that_reopens_unchanged() {
    let root = std::env::temp_dir().join(format!("mixjam-song-format-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("song.mixjam");
    let project = stopped(thrown(driven(rolled(panned(sectioned(project()))))));
    save(&project, &path).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    let mut document: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(document["formatVersion"], 24);
    assert_eq!(document["appVersion"], crate::APP_VERSION);
    assert_eq!(
        document["song"]["sections"],
        serde_json::json!([
            {"startTick": 0, "name": "Intro"},
            {"startTick": 2048, "name": "Drift, part I"}
        ])
    );
    assert_eq!(
        document["lanes"][5]["eq"],
        serde_json::json!({
            "on": true, "lowCut": 80.0, "lowHz": 100.0, "lowDb": 0.0, "midHz": 1000.0,
            "midDb": 0.0, "midQ": 1.0, "highHz": 8000.0, "highDb": -7.5, "highCut": 20000.0
        })
    );
    assert_eq!(document["lanes"][7]["eq"]["on"], false);
    assert_eq!(document["lanes"][7]["eq"]["midDb"], 3.0);
    // The second peak saves only what differs from a new track's.
    assert_eq!(document["lanes"][7]["eq"]["peak2Db"], -4.5);
    assert!(document["lanes"][7]["eq"].get("peak2Hz").is_none());
    assert_eq!(
        document["lanes"][5]["compressor"],
        serde_json::json!({
            "on": true, "threshold": -24.0, "ratio": 4.0, "attack": 10.0, "release": 100.0,
            "makeup": 3.0
        })
    );
    assert_eq!(document["lanes"][5]["width"], 150.0);
    assert_eq!(document["lanes"][9]["width"], 0.0);
    assert_eq!(
        document["lanes"][5]["drive"],
        serde_json::json!({
            "on": true, "amount": 18.0, "character": 60.0, "mix": 40.0, "focus": 98.0,
            "output": -1.5
        })
    );
    // A drive that is off keeps its settings; one as a new track has it
    // saves none.
    assert_eq!(document["lanes"][8]["drive"]["on"], false);
    assert_eq!(document["lanes"][8]["drive"]["amount"], 6.0);
    assert!(document["lanes"][4].get("drive").is_none());
    assert!(document["lanes"][4].get("compressor").is_none());
    assert!(document["lanes"][4].get("width").is_none());
    // A lane whose EQ is as a new track has it saves none.
    assert!(document["lanes"][4].get("eq").is_none());
    assert_eq!(
        document["lanes"][5]["ducker"],
        serde_json::json!({
            "on": true, "key": 1, "threshold": -30.0, "depth": 9.5, "attack": 2.0, "release": 120.0
        })
    );
    assert!(document["lanes"][4].get("ducker").is_none());
    assert_eq!(document["master"]["bypass"], true);
    assert_eq!(
        document["lanes"][63]["placements"][0]["fades"],
        serde_json::json!({
            "in": {"ticks": 64, "curve": "equal-power"},
            "out": {"ticks": 128, "curve": "exponential"}
        })
    );
    assert_eq!(document["lanes"][63]["placements"][0]["pitchCents"], -150);
    assert_eq!(
        document["lanes"][5]["automation"],
        serde_json::json!({
            "volume": [{"tick": 0, "value": -60.0}, {"tick": 512, "value": 0.0}],
            "sends": [[], [{"tick": 256, "value": 0.75}], [], []],
            "filter": [{"tick": 1024, "value": -0.5}],
            "pan": [
                {"tick": 0, "value": -1.0},
                {"tick": 64, "value": 1.0},
                {"tick": 128, "value": 0.0}
            ],
            "repeat": [
                {"tick": 256, "value": 8.0},
                {"tick": 320, "value": 16.0},
                {"tick": 384, "value": 32.0},
                {"tick": 512, "value": 0.0}
            ],
            "repeatGate": [
                {"tick": 256, "value": 25.0},
                {"tick": 384, "value": 60.0}
            ],
            "pitch": [
                {"tick": 768, "value": 0.0},
                {"tick": 832, "value": -4800.0}
            ]
        })
    );
    assert!(document["lanes"][4].get("automation").is_none());
    // A placement without fades, pitch, or reverse saves none of them.
    let plain = document["lanes"][0]["placements"][0].as_object().unwrap();
    assert!(!plain.contains_key("fades") && !plain.contains_key("pitchCents"));
    assert!(!plain.contains_key("reverse"));
    assert_eq!(document["lanes"][63]["placements"][0]["reverse"], true);
    // A bus with a return lane saves it after its settings; one without
    // saves none.
    assert_eq!(
        document["fxBuses"][1]["returnAutomation"],
        serde_json::json!([
            {"tick": 0, "value": 0.25},
            {"tick": 448, "value": 0.25},
            {"tick": 480, "value": 2.0},
            {"tick": 512, "value": 0.25}
        ])
    );
    assert!(document["fxBuses"][0].get("returnAutomation").is_none());
    assert_eq!(
        document["lanes"][5]["sends"],
        serde_json::json!([0.25, 0.0, 0.5, 1.0])
    );
    assert_eq!(
        document["fxBuses"][0]["module"]["type"],
        "aetherform-reverb"
    );
    assert_eq!(document["fxBuses"][0]["module"]["spaceModel"], "hall");
    assert_eq!(document["fxBuses"][2]["module"]["divisionL"], "1/4.");
    assert_eq!(document["fxBuses"][2]["powered"], false);
    assert_eq!(
        document["fxBuses"][1],
        serde_json::json!({
            "module": {"type": "empty"}, "powered": true, "returnLevel": 1.0, "limiter": true,
            "returnAutomation": [
                {"tick": 0, "value": 0.25},
                {"tick": 448, "value": 0.25},
                {"tick": 480, "value": 2.0},
                {"tick": 512, "value": 0.25}
            ]
        })
    );
    // The return lane follows the bus's settings.
    let bus = text.find(r#""returnLevel": 1.0,"#).unwrap();
    assert!(text[bus..].find(r#""returnAutomation""#).is_some());
    assert_eq!(document["master"]["boosterCompressor"]["on"], true);
    assert_eq!(document["master"]["boosterCompressor"]["drive"], 368);
    assert_eq!(document["master"]["boosterEq"]["bands"][9], 0);
    assert_eq!(document["master"]["boosterWide"]["on"], false);
    assert_eq!(
        document["master"]["order"],
        serde_json::json!([
            "boosterWide",
            "boosterEq",
            "boosterCompressor",
            "softClip",
            "tube",
            "trimEq",
            "busCompressor",
            "maximizer",
            "liftEq",
            "tape",
            "imaging",
            "multiband",
            "limiter"
        ])
    );
    // Settings keep their written form: decibels, percent, Hz, and ms.
    assert!(text.contains(r#""ceiling": -0.3"#), "{text}");
    assert!(text.contains(r#""attack": 0.1,"#));
    assert_eq!(document["master"]["trim"], -2.5);
    assert_eq!(document["master"]["limiter"]["on"], true);
    assert_eq!(document["master"]["tape"]["speed"], 15);
    assert_eq!(document["master"]["imaging"]["monoBelow"], 120.0);
    assert_eq!(document["song"]["masterGain"], 0.5);
    assert_eq!(document["lanes"].as_array().unwrap().len(), 64);
    assert_eq!(document["lanes"][63]["placements"][0]["durationTicks"], 512);
    assert_eq!(document["lanes"][5]["leftEnabled"], false);
    assert_eq!(document["lanes"][5]["polarityInverted"], true);
    assert_eq!(document["lanes"][5]["muted"], true);
    assert_eq!(document["lanes"][5]["solo"], false);
    assert_eq!(document["lanes"][6]["solo"], true);
    assert_eq!(document["missingSamples"][1]["reference"]["nativeId"], 7);
    assert_eq!(
        document.as_object().unwrap().keys().collect::<Vec<_>>(),
        [
            "appVersion",
            "createdAt",
            "formatVersion",
            "fxBuses",
            "importNotices",
            "lanes",
            "libraries",
            "master",
            "missingSamples",
            "modifiedAt",
            "sampleConflicts",
            "samples",
            "song",
            "sourceNotices",
            "sourceSettings"
        ]
    );
    assert_eq!(document["sourceSettings"]["BoostCompressorLED"], "1");
    assert!(text.ends_with(
        "}
"
    ));
    assert_eq!(
        crate::mixjam::load_song(&path, &Library::empty())
            .unwrap()
            .0,
        project
    );

    // The creation time belongs to the file; later saves change only modifiedAt.
    document["createdAt"] = "2020-01-02T03:04:05Z".into();
    document["modifiedAt"] = "2020-01-02T03:04:05Z".into();
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    save(&project, &path).unwrap();
    let resaved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(resaved["createdAt"], "2020-01-02T03:04:05Z");
    assert_ne!(resaved["modifiedAt"], "2020-01-02T03:04:05Z");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn current_songs_require_explicit_polarity_and_nonnegative_amplitude() {
    let document =
        serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
    let mut changed = document.clone();
    changed["lanes"][0]
        .as_object_mut()
        .unwrap()
        .remove("polarityInverted");
    assert!(
        parse(&serde_json::to_vec(&changed).unwrap())
            .unwrap_err()
            .contains("polarityInverted")
    );
    for value in [serde_json::Value::Null, 0.into(), "false".into()] {
        changed = document.clone();
        changed["lanes"][0]["polarityInverted"] = value;
        assert!(parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
    for amplitude in [-0.5, 4.01] {
        changed = document.clone();
        changed["lanes"][0]["gain"] = amplitude.into();
        assert!(parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
}

#[test]
fn rejects_unknown_keys_missing_lanes_and_invalid_arrangement_data() {
    let document =
        serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
    for (pointer, value, expected) in [
        ("/song/pitchRatio", serde_json::json!(1.0), "unknown field"),
        (
            "/lanes/0/sends",
            serde_json::json!([0, 0, 0, 1.5]),
            "sends must be",
        ),
        (
            "/fxBuses/0/module/mix",
            serde_json::json!(1),
            "unknown field",
        ),
        (
            "/fxBuses/0/returnLevel",
            serde_json::json!(2),
            "return level",
        ),
        (
            "/lanes/63/placements/0/fades/in/ticks",
            serde_json::json!(512),
            "fades longer than the clip",
        ),
        (
            "/lanes/63/placements/0/fades/in/curve",
            serde_json::json!("s-curve"),
            "unknown variant",
        ),
        (
            "/lanes/63/placements/0/pitchCents",
            serde_json::json!(2401),
            "pitch must be",
        ),
        ("/formatVersion", serde_json::json!(13), "cannot contain"),
        ("/formatVersion", serde_json::json!(14), "cannot contain"),
        (
            "/lanes/5/eq/highCut",
            serde_json::json!(20_001),
            "channel EQ setting",
        ),
        (
            "/lanes/5/eq/midQ",
            serde_json::json!(0.2),
            "channel EQ setting",
        ),
        ("/lanes/5/eq/gain", serde_json::json!(1), "unknown field"),
        (
            "/lanes/5/automation/volume/1/tick",
            serde_json::json!(0),
            "increasing tick order",
        ),
        (
            "/lanes/5/automation/filter/0/value",
            serde_json::json!(1.5),
            "Filter automation values",
        ),
        (
            "/lanes/5/automation/balance",
            serde_json::json!([]),
            "unknown field",
        ),
        ("/lanes", serde_json::json!([]), "64 lanes"),
        ("/song/bpm", serde_json::json!(127), "Tempo"),
        ("/lanes/0/placements/0/id", serde_json::json!(1), "unique"),
        ("/createdAt", serde_json::json!(""), "metadata"),
    ] {
        let mut changed = document.clone();
        match changed.pointer_mut(pointer) {
            Some(target) => *target = value,
            None => {
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                changed.pointer_mut(parent).unwrap()[key] = value;
            }
        }
        let error = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
        assert!(error.contains(expected), "{pointer}: {error}");
    }
}

#[test]
fn nullable_fields_are_required_but_accept_null() {
    let document =
        serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
    for (parent, key) in [
        ("/song", "loopRegion"),
        ("/samples/0", "sourceSampleRate"),
        ("/samples/0", "origin"),
        ("/missingSamples/0/reference", "origin"),
    ] {
        let mut changed = document.clone();
        changed.pointer_mut(parent).unwrap()[key] = serde_json::Value::Null;
        parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
        changed
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        let error = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
        assert!(error.contains("missing field"), "{parent}/{key}: {error}");
    }
}

#[test]
fn saving_over_an_oversized_destination_does_not_read_its_contents() {
    let path = std::env::temp_dir().join(format!(
        "mixjam-oversized-save-{}.mixjam",
        std::process::id()
    ));
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_SONG_BYTES + 1).unwrap();
    drop(file);
    assert!(existing_creation_time(&path).is_none());
    save(&Project::default(), &path).unwrap();
    assert_eq!(
        crate::mixjam::load_song(&path, &Library::empty())
            .unwrap()
            .0,
        Project {
            // An untitled song opens under its file name.
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            ..Project::default()
        }
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn serialized_song_limit_includes_formatting_and_preserves_the_destination() {
    let path =
        std::env::temp_dir().join(format!("mixjam-save-limit-{}.mixjam", std::process::id()));
    let project = Project::default();
    save(&project, &path).unwrap();
    let saved_length = fs::metadata(&path).unwrap().len();
    let compact = serde_json::to_vec(&Document::new(&project, timestamp(0), timestamp(0))).unwrap();
    assert!((compact.len() as u64) < saved_length);

    fs::write(&path, b"previous song").unwrap();
    for limit in [compact.len() as u64, saved_length - 1] {
        assert!(
            save_with_limit(&project, &path, limit)
                .unwrap_err()
                .contains("limit")
        );
        assert_eq!(fs::read(&path).unwrap(), b"previous song");
    }
    save_with_limit(&project, &path, saved_length).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), saved_length);
    assert_eq!(parse(&fs::read(&path).unwrap()).unwrap(), project);
    fs::remove_file(&path).unwrap();
    assert!(save_with_limit(&project, &path, saved_length - 1).is_err());
    assert!(!path.exists());
}
