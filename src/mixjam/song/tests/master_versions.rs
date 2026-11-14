use super::*;

#[test]
fn version_17_ducker_thresholds_move_by_the_master_gain() {
    // A version 17 key held the master gain. From version 18 it leaves
    // it out, 6.02 dB hotter at a master gain of 0.5, so the threshold
    // rises as much and the lane ducks as it did.
    let mut project = project();
    project.tracks.iter_mut().for_each(before_version_18);
    assert_eq!(project.master_gain, 0.5);
    let mut document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    let open =
        |document: &serde_json::Value| parse(&serde_json::to_vec(document).unwrap()).unwrap();
    // Version 18 keeps the saved threshold.
    assert_eq!(open(&document), project);
    document["formatVersion"] = 17.into();
    let opened = open(&document);
    let raised = opened.tracks[5].ducker.threshold - project.tracks[5].ducker.threshold;
    assert!((raised - 6.0206).abs() < 1e-3, "{raised} dB");
    // Nothing else changes, and lanes without a ducker keep the default.
    let mut expected = project.clone();
    expected.tracks[5].ducker.threshold = opened.tracks[5].ducker.threshold;
    assert_eq!(opened, expected);
    // A ducker that is off moves too. The range holds the threshold, and
    // a master gain of 0, which left the key silent, leaves it.
    document["lanes"][5]["ducker"]["on"] = false.into();
    for (master_gain, threshold, moved) in [
        (0.5, -3.0, 0.0),
        (2.0, -58.0, -60.0),
        (2.0, -18.0, -24.0206),
        (1.0, -18.0, -18.0),
        (0.0, -18.0, -18.0),
    ] {
        document["song"]["masterGain"] = master_gain.into();
        document["lanes"][5]["ducker"]["threshold"] = threshold.into();
        let ducker = open(&document).tracks[5].ducker;
        assert!(!ducker.on);
        assert!(
            (ducker.threshold - moved).abs() < 1e-3,
            "{master_gain} {threshold}: {}",
            ducker.threshold
        );
    }
    // A threshold outside its range stays, so the song is rejected, as
    // before the move existed, even where the move would bring it back.
    for (master_gain, threshold) in [(1.0, 5.0), (0.5, -61.0), (2.0, 3.0)] {
        document["song"]["masterGain"] = master_gain.into();
        document["lanes"][5]["ducker"]["threshold"] = threshold.into();
        let found = parse(&serde_json::to_vec(&document).unwrap()).unwrap_err();
        assert!(
            found.contains("ducker setting is out of range"),
            "{master_gain} {threshold}: {found}"
        );
    }
}

#[test]
fn versions_before_18_reject_a_compressor_a_width_and_a_second_peak() {
    let project = project();
    let document = serde_json::to_value(Document::new(
        &project,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    let mut expected = project.clone();
    expected.tracks.iter_mut().for_each(before_version_18);
    let mut earlier = serde_json::to_value(Document::new(
        &expected,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    earlier["formatVersion"] = 17.into();
    // Lane 5's ducker threshold moves by the master gain, as
    // `version_17_ducker_thresholds_move_by_the_master_gain` checks.
    let mut opened = expected.clone();
    opened.tracks[5].ducker.threshold =
        threshold_without_master_gain(opened.tracks[5].ducker.threshold, opened.master_gain);
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap()).unwrap(),
        opened
    );
    for (lane, key, value, error) in [
        (
            5,
            "compressor",
            document["lanes"][5]["compressor"].clone(),
            "compressor or width",
        ),
        (5, "width", 150.into(), "compressor or width"),
        (4, "width", 100.into(), "compressor or width"),
        (
            7,
            "eq",
            document["lanes"][7]["eq"].clone(),
            "eq peak2Hz, peak2Db, or peak2Q",
        ),
    ] {
        let mut changed = earlier.clone();
        changed["lanes"][lane][key] = value;
        let found = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
        assert_eq!(
            found,
            format!("Version 17 lanes cannot contain {error}."),
            "{key}"
        );
    }
    // An explicit flat second peak is new in version 18 too.
    let mut flat = earlier;
    flat["lanes"][5]["eq"]["peak2Db"] = 0.into();
    assert_eq!(
        parse(&serde_json::to_vec(&flat).unwrap()).unwrap_err(),
        "Version 17 lanes cannot contain eq peak2Hz, peak2Db, or peak2Q."
    );
    let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(reopened, project);
    for (pointer, value, error) in [
        ("/lanes/5/width", serde_json::json!(201), "width"),
        ("/lanes/5/width", serde_json::json!(-1), "width"),
        (
            "/lanes/5/compressor/ratio",
            serde_json::json!(0.5),
            "compressor setting",
        ),
        (
            "/lanes/5/compressor/knee",
            serde_json::json!(6),
            "unknown field",
        ),
        (
            "/lanes/5/eq/peak2Q",
            serde_json::json!(9),
            "channel EQ setting",
        ),
        (
            "/lanes/5/eq/peak2Hz",
            serde_json::json!(30),
            "channel EQ setting",
        ),
    ] {
        let mut changed = document.clone();
        match changed.pointer_mut(pointer) {
            Some(target) => *target = value,
            None => {
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                changed.pointer_mut(parent).unwrap()[key] = value;
            }
        }
        let found = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
        assert!(found.contains(error), "{pointer}: {found}");
    }
}

#[test]
fn legacy_master_settings_reject_duplicate_fields_before_migration() {
    for version in 11..=15 {
        let mut document = serde_json::to_value(Document::new(
            &Project::default(),
            timestamp(0),
            timestamp(0),
        ))
        .unwrap();
        document["formatVersion"] = version.into();
        with_earlier_booster_keys(&mut document);
        if version == 11 {
            document["master"]
                .as_object_mut()
                .unwrap()
                .retain(|key, _| {
                    key == "order" || BOOSTER_KEYS.iter().any(|(earlier, _)| key == *earlier)
                });
            document["master"]["order"]
                .as_array_mut()
                .unwrap()
                .truncate(3);
        }
        if version < 13 {
            document.as_object_mut().unwrap().remove("fxBuses");
            for lane in document["lanes"].as_array_mut().unwrap() {
                lane.as_object_mut().unwrap().remove("sends");
            }
        }
        if version < 15 {
            document["master"].as_object_mut().unwrap().remove("bypass");
        }
        let text = serde_json::to_string(&document).unwrap();
        parse(text.as_bytes()).unwrap();
        let eq = serde_json::to_string(&document["master"]["ejayEq"]).unwrap();
        let field = format!("\"ejayEq\":{eq}");
        for replacement in [
            format!("{field},{field}"),
            format!(
                "\"ejayEq\":{}",
                eq.replacen("\"on\":false", "\"on\":false,\"on\":true", 1)
            ),
            format!(
                "\"ejayEq\":{}",
                eq.replacen("\"on\":false", "\"on\":false,\"\\u006fn\":true", 1)
            ),
        ] {
            let changed = text.replacen(&field, &replacement, 1);
            assert_ne!(changed, text);
            let error = match parse(changed.as_bytes()) {
                Ok(_) => panic!("Version {version} accepted a duplicate master field."),
                Err(error) => error,
            };
            assert!(
                error.contains("duplicate field"),
                "version {version}: {error}"
            );
        }
    }
}

#[test]
fn fx_settings_reject_duplicate_fields_before_adoption() {
    let document = Document::new(&project(), timestamp(0), timestamp(0));
    let text = serde_json::to_string(&document).unwrap();
    let duplicate = text.replacen("\"powered\":true", "\"powered\":true,\"powered\":false", 1);
    assert_ne!(duplicate, text);
    assert!(
        parse(duplicate.as_bytes())
            .unwrap_err()
            .contains("duplicate field")
    );
}

#[test]
fn earlier_versions_reject_explicit_own_input_keys() {
    let mut expected = project();
    for track in &mut expected.tracks {
        track.ducker = Default::default();
        before_version_18(track);
    }
    let base = serde_json::to_value(Document::new(&expected, timestamp(0), timestamp(0))).unwrap();
    for effect in [false, true] {
        let mut document = base.clone();
        document["formatVersion"] = 16.into();
        if effect {
            let module = crate::fx::Module::EchoformDelay(Default::default());
            document["fxBuses"][0]["module"] = serde_json::to_value(module).unwrap();
            document["fxBuses"][0]["module"]["duckKey"] = 0.into();
        } else {
            document["master"]["limiter"]["key"] = 0.into();
        }
        let reopened: Document = serde_json::from_value(document).unwrap();
        assert_eq!(
            reopened.into_project().unwrap_err(),
            "Version 16 songs cannot contain key or duckKey."
        );
    }
}

#[test]
fn pedal_keys_save_only_when_they_name_a_track() {
    let mut project = project();
    let saved = |project: &Project| {
        serde_json::to_value(Document::new(
            project,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap()
    };
    let document = saved(&project);
    assert!(document["master"]["busCompressor"].get("key").is_none());
    project.master.bus_compressor.key = 2;
    project.master.bus_compressor.on = true;
    let document = saved(&project);
    assert_eq!(document["master"]["busCompressor"]["key"], 2);
    let reopened: Document = serde_json::from_value(document).unwrap();
    assert_eq!(reopened.into_project().unwrap().master, project.master);
}

#[test]
fn version_8_signed_lanes_migrate_and_save_as_the_current_version_without_losing_polarity() {
    let root = std::env::temp_dir().join(format!("mixjam-polarity-v8-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("song.mixjam");
    let mut expected = project();
    for track in &mut expected.tracks {
        (track.muted, track.solo) = (false, false);
    }
    expected.tracks[0].gain = 4.0;
    expected.tracks[1].gain = 0.0;
    expected.tracks[1].polarity_inverted = true;
    expected.tracks[2].set_legacy_gain(-2.0 / 65536.0);
    let mut legacy = earlier_document(&mut expected, 8);
    for (lane, track) in legacy["lanes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&expected.tracks)
    {
        let lane = lane.as_object_mut().unwrap();
        for key in ["polarityInverted", "muted", "solo"] {
            lane.remove(key);
        }
        lane.insert("gain".into(), track.effective_gain().into());
    }
    fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let (migrated, replaceable) = crate::mixjam::load_song(&path, &Library::empty()).unwrap();
    assert!(replaceable);
    assert_eq!(migrated, expected);
    assert!(migrated.tracks[1].polarity_inverted);
    save(&migrated, &path).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["formatVersion"], FORMAT_VERSION);
    assert_eq!(saved["createdAt"], "2020-01-02T03:04:05Z");
    assert_eq!(saved["lanes"][1]["gain"], 0.0);
    assert_eq!(saved["lanes"][1]["polarityInverted"], true);
    assert_eq!(
        crate::mixjam::load_song(&path, &Library::empty())
            .unwrap()
            .0,
        expected
    );
    legacy["lanes"][0]["gain"] = (-2.01).into();
    assert!(parse(&serde_json::to_vec(&legacy).unwrap()).is_err());
    legacy["lanes"][0]["gain"] = 1.0.into();
    legacy["lanes"][0]["polarityInverted"] = false.into();
    assert!(parse(&serde_json::to_vec(&legacy).unwrap()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn version_9_songs_open_unmuted_with_their_outputs_and_save_as_the_current_version() {
    let root = std::env::temp_dir().join(format!("mixjam-switches-v9-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("song.mixjam");
    let mut expected = project();
    for track in &mut expected.tracks {
        (track.muted, track.solo) = (false, false);
    }
    let mut earlier = earlier_document(&mut expected, 9);
    for lane in earlier["lanes"].as_array_mut().unwrap() {
        let lane = lane.as_object_mut().unwrap();
        lane.remove("muted");
        lane.remove("solo");
    }
    fs::write(&path, serde_json::to_vec(&earlier).unwrap()).unwrap();
    let (opened, replaceable) = crate::mixjam::load_song(&path, &Library::empty()).unwrap();
    assert!(replaceable);
    // L/R states stay as saved, including outputs an earlier import disabled.
    assert_eq!(opened, expected);
    assert!(!opened.tracks[5].left_enabled);
    save(&opened, &path).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["formatVersion"], FORMAT_VERSION);
    assert_eq!(saved["createdAt"], "2020-01-02T03:04:05Z");
    assert_eq!(saved["lanes"][5]["muted"], false);
    for key in ["muted", "solo"] {
        let mut changed = earlier.clone();
        changed["lanes"][0][key] = false.into();
        assert!(
            parse(&serde_json::to_vec(&changed).unwrap())
                .unwrap_err()
                .contains("cannot contain muted or solo")
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn version_10_songs_disable_pedals_and_current_songs_require_master() {
    let mut expected = project();
    let earlier = earlier_document(&mut expected, 10);
    let opened = parse(&serde_json::to_vec(&earlier).unwrap()).unwrap();
    assert_eq!(opened, expected);
    assert!(!opened.master.is_active());

    let mut with_master = earlier.clone();
    with_master["master"] = serde_json::to_value(MasterChain::default()).unwrap();
    assert!(
        parse(&serde_json::to_vec(&with_master).unwrap())
            .unwrap_err()
            .contains("Version 10 songs cannot contain master")
    );
    let current =
        serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
    let mut without = current.clone();
    without.as_object_mut().unwrap().remove("master");
    assert!(
        parse(&serde_json::to_vec(&without).unwrap())
            .unwrap_err()
            .contains("require master")
    );
    for (pointer, value) in [
        ("/master/boosterEq/bands/0", serde_json::Value::from(101)),
        ("/master/boosterCompressor/speed", 1001.into()),
        ("/master/boosterWide/spread", (-1).into()),
        ("/master/boosterWide/on", serde_json::Value::Null),
        ("/master/order/0", "boosterEq".into()),
        (
            "/master/order",
            serde_json::json!(["boosterEq", "boosterWide"]),
        ),
        ("/master/order/2", "limiter".into()),
        ("/master/order/12", "tape".into()),
        ("/master/order/5", "reverb".into()),
        ("/master/limiter/ceiling", 0.5.into()),
        ("/master/tape/speed", 20.into()),
        ("/master/trim", "loud".into()),
        ("/master/busCompressor/ratio", 1.into()),
    ] {
        let mut changed = current.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let mut unknown = current;
    unknown["master"]["boosterWide"]["width"] = 3.into();
    assert!(
        parse(&serde_json::to_vec(&unknown).unwrap())
            .unwrap_err()
            .contains("unknown field `width`")
    );
}

#[test]
fn version_11_songs_open_with_their_booster_pedals_first_and_the_rest_off() {
    let mut expected = project();
    let mut document = earlier_document(&mut expected, 11);
    let [eq, compressor, wide] = BOOSTER_KEYS.map(|(earlier, _)| earlier);
    document["master"] = serde_json::json!({"order": [wide, compressor, eq]});
    document["master"][eq] =
        serde_json::json!({"on": false, "bands": [50, 50, 50, 50, 50, 50, 50, 50, 50, 0]});
    document["master"][compressor] =
        serde_json::json!({"on": true, "drive": 368, "speed": 500, "gain": 500});
    document["master"][wide] = serde_json::json!({"on": true, "spread": 700});
    let opened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
    let master = opened.master;
    assert_eq!(
        master.order[..3],
        [
            PedalKind::BoosterWide,
            PedalKind::BoosterCompressor,
            PedalKind::BoosterEq
        ]
    );
    assert_eq!(master.order[3..], PedalKind::ALL[3..]);
    assert!(master.booster_compressor.on && master.booster_wide.on && !master.booster_eq.on);
    assert_eq!(
        (master.booster_wide.spread, master.booster_eq.bands[9]),
        (700, 0)
    );
    assert!(
        PedalKind::ALL
            .iter()
            .filter(|kind| !kind.is_booster())
            .all(|kind| !master.is_on(*kind))
    );
    assert_eq!(master.trim, 0.0);
    assert_eq!(opened, Project { master, ..expected });

    // A version 11 song keeps the Booster record's strict shape and its
    // earlier keys.
    for (pointer, value) in [
        ("/master/order".to_string(), serde_json::json!([eq, wide])),
        ("/master/order/0".into(), compressor.into()),
        ("/master/order/0".into(), "boosterWide".into()),
        (format!("/master/{wide}/spread"), 1001.into()),
    ] {
        let mut changed = document.clone();
        *changed.pointer_mut(&pointer).unwrap() = value;
        assert!(
            parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let mut later = document.clone();
    later["master"]["trim"] = 0.into();
    assert!(parse(&serde_json::to_vec(&later).unwrap()).is_err());
    let mut without = document;
    without.as_object_mut().unwrap().remove("master");
    assert!(
        parse(&serde_json::to_vec(&without).unwrap())
            .unwrap_err()
            .contains("Version 11 songs require master")
    );
}

#[test]
fn version_10_requires_explicit_mute_and_solo() {
    let document = earlier_document(&mut project(), 10);
    for key in ["muted", "solo"] {
        let mut changed = document.clone();
        changed["lanes"][0].as_object_mut().unwrap().remove(key);
        assert!(
            parse(&serde_json::to_vec(&changed).unwrap())
                .unwrap_err()
                .contains("require muted and solo")
        );
        for value in [serde_json::Value::Null, 0.into(), "false".into()] {
            changed = document.clone();
            changed["lanes"][0][key] = value;
            assert!(parse(&serde_json::to_vec(&changed).unwrap()).is_err());
        }
    }
}

#[test]
fn version_14_songs_open_with_every_eq_off_and_the_board_in_use() {
    let mut expected = project();
    expected.master.bypass = false;
    for track in &mut expected.tracks {
        track.eq = Default::default();
        track.ducker = Default::default();
        before_version_18(track);
    }
    let mut document =
        serde_json::to_value(Document::new(&expected, timestamp(0), timestamp(0))).unwrap();
    document["formatVersion"] = 14.into();
    with_earlier_booster_keys(&mut document);
    assert_eq!(
        parse(&serde_json::to_vec(&document).unwrap()).unwrap(),
        expected
    );
    let mut with_eq = document.clone();
    with_eq["lanes"][2]["eq"] = serde_json::to_value(crate::eq::ChannelEq::default()).unwrap();
    let error = parse(&serde_json::to_vec(&with_eq).unwrap()).unwrap_err();
    assert_eq!(error, "Version 14 lanes cannot contain eq.");
    let mut bypassed = document;
    bypassed["master"]["bypass"] = true.into();
    let error = parse(&serde_json::to_vec(&bypassed).unwrap()).unwrap_err();
    assert_eq!(error, "Version 14 songs cannot contain master bypass.");
}

#[test]
fn version_12_to_15_boards_keep_their_earlier_booster_keys() {
    let mut expected = project();
    for track in &mut expected.tracks {
        track.ducker = Default::default();
        before_version_18(track);
    }
    let current =
        serde_json::to_value(Document::new(&expected, timestamp(0), timestamp(0))).unwrap();
    let mut earlier = current.clone();
    earlier["formatVersion"] = 15.into();
    with_earlier_booster_keys(&mut earlier);
    // The same board opens from either key set, each at its own version.
    assert_eq!(
        parse(&serde_json::to_vec(&earlier).unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        parse(&serde_json::to_vec(&current).unwrap()).unwrap(),
        expected
    );
    let (eq, booster_eq) = BOOSTER_KEYS[0];
    for version in 12..=15 {
        let mut document = earlier.clone();
        document["formatVersion"] = version.into();
        document["master"].as_object_mut().unwrap().remove("bypass");
        let mut current_key = document.clone();
        let pedal = current_key["master"]
            .as_object_mut()
            .unwrap()
            .remove(eq)
            .unwrap();
        current_key["master"][booster_eq] = pedal;
        assert_eq!(
            parse(&serde_json::to_vec(&current_key).unwrap()).unwrap_err(),
            format!("Version {version} songs cannot contain master {booster_eq}.")
        );
        let mut current_order = document.clone();
        current_order["master"]["order"][0] = "boosterWide".into();
        assert_eq!(
            parse(&serde_json::to_vec(&current_order).unwrap()).unwrap_err(),
            format!("Version {version} songs cannot contain master boosterWide.")
        );
    }
    // Versions 16 and later accept only the new keys; a version after the
    // current one does not open.
    let mut earlier_key = current.clone();
    let pedal = earlier_key["master"]
        .as_object_mut()
        .unwrap()
        .remove(booster_eq)
        .unwrap();
    earlier_key["master"][eq] = pedal;
    let error = parse(&serde_json::to_vec(&earlier_key).unwrap()).unwrap_err();
    assert!(error.contains(&format!("unknown field `{eq}`")), "{error}");
    let mut earlier_order = current.clone();
    earlier_order["master"]["order"][0] = earlier["master"]["order"][0].clone();
    let error = parse(&serde_json::to_vec(&earlier_order).unwrap()).unwrap_err();
    assert!(error.contains("unknown variant"), "{error}");
    let mut later = current;
    later["formatVersion"] = (FORMAT_VERSION + 1).into();
    assert_eq!(
        parse(&serde_json::to_vec(&later).unwrap()).unwrap_err(),
        "This MixJam format version is not supported."
    );
}
