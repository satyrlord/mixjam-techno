use super::*;

#[test]
fn version_29_channel_strip_keys_round_trip_and_reject_in_earlier_versions() {
    let mut proj = project();
    proj.tracks[2].eq.on = true;
    proj.tracks[2].eq.cut_slope = crate::eq::CutSlope::Slope24;
    proj.tracks[2].compressor.on = true;
    proj.tracks[2].compressor.sc_hpf_hz = 115.0;
    proj.tracks[2].width = 140.0;
    proj.tracks[2].mono_below_hz = 120.0;
    proj.tracks[2].filter_resonance = 42.0;
    proj.tracks[2].filter_slope = crate::automation::FilterSlope::Slope24;

    let doc = serde_json::to_value(Document::new(
        &proj,
        "2026-11-23T20:00:00Z".into(),
        timestamp(0),
    ))
    .unwrap();
    assert_eq!(doc["formatVersion"], FORMAT_VERSION);
    let lane = &doc["lanes"][2];
    assert_eq!(lane["eq"]["cutSlope"], "24db");
    assert_eq!(lane["compressor"]["scHpfHz"], 115.0);
    assert_eq!(lane["monoBelowHz"], 120.0);
    assert_eq!(lane["filterResonance"], 42.0);
    assert_eq!(lane["filterSlope"], "24db");
    // Defaults are omitted on untouched tracks.
    assert!(doc["lanes"][5].get("monoBelowHz").is_none());
    assert!(doc["lanes"][5].get("filterResonance").is_none());
    assert!(doc["lanes"][5].get("filterSlope").is_none());
    assert!(doc["lanes"][5]["eq"].get("cutSlope").is_none());
    assert!(doc["lanes"][5]["compressor"].get("scHpfHz").is_none());

    let reopened = parse(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(reopened, proj);

    // Version 28 rejects every v29 key (even at explicit default values).
    let base = serde_json::to_value(Document::new(
        &project(),
        "2026-11-23T20:00:00Z".into(),
        timestamp(0),
    ))
    .unwrap();
    for (patch, needle) in [
        (
            serde_json::json!({"eq": {"on": false, "lowCut": 20.0, "lowHz": 100.0, "lowDb": 0.0,
                "midHz": 1000.0, "midDb": 0.0, "midQ": 1.0, "highHz": 8000.0, "highDb": 0.0,
                "highCut": 20000.0, "cutSlope": "12db"}}),
            "eq cutSlope",
        ),
        (
            serde_json::json!({"compressor": {"on": false, "threshold": -20.0, "ratio": 4.0,
                "attack": 10.0, "release": 100.0, "makeup": 0.0, "scHpfHz": 20.0}}),
            "compressor scHpfHz",
        ),
        (
            serde_json::json!({"monoBelowHz": 20.0}),
            "v29 channel strip",
        ),
        (
            serde_json::json!({"filterResonance": 0.0}),
            "v29 channel strip",
        ),
        (
            serde_json::json!({"filterSlope": "12db"}),
            "v29 channel strip",
        ),
    ] {
        let mut old = base.clone();
        old["formatVersion"] = 28.into();
        let target = old["lanes"][0].as_object_mut().unwrap();
        for (k, v) in patch.as_object().unwrap() {
            target.insert(k.clone(), v.clone());
        }
        let err = parse(&serde_json::to_vec(&old).unwrap()).unwrap_err();
        assert!(err.contains(needle), "{needle}: {err}");
    }

    // Version 28 without v29 keys opens cleanly with lossless defaults.
    let mut valid_v28 = base;
    valid_v28["formatVersion"] = 28.into();
    let migrated = parse(&serde_json::to_vec(&valid_v28).unwrap()).unwrap();
    assert_eq!(migrated, project());
}

#[test]
fn channel_strip_keys_reject_nulls_positional_records_and_invalid_values() {
    let base = serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
    for version in [28, 29] {
        for key in ["monoBelowHz", "filterResonance", "filterSlope"] {
            let mut closed = base.clone();
            closed["formatVersion"] = version.into();
            closed["lanes"][0][key] = serde_json::Value::Null;
            assert!(
                parse(&serde_json::to_vec(&closed).unwrap()).is_err(),
                "{key} in {version}"
            );
        }
        for (key, array) in [
            (
                "eq",
                serde_json::json!([
                    true, 20.0, 100.0, 0.0, 1000.0, 0.0, 1.0, 250.0, 0.0, 1.0, 8000.0, 0.0,
                    20000.0, "24db"
                ]),
            ),
            (
                "compressor",
                serde_json::json!([true, -20.0, 4.0, 10.0, 100.0, 0.0, 140.0]),
            ),
        ] {
            let mut closed = base.clone();
            closed["formatVersion"] = version.into();
            closed["lanes"][0][key] = array;
            let error = parse(&serde_json::to_vec(&closed).unwrap()).unwrap_err();
            assert!(
                error.contains("must be a JSON object"),
                "{key} in {version}: {error}"
            );
        }
    }
    for (key, value) in [
        ("monoBelowHz", serde_json::json!(301)),
        ("filterResonance", serde_json::json!(101)),
        ("filterSlope", serde_json::json!("48db")),
    ] {
        let mut closed = base.clone();
        closed["lanes"][0][key] = value;
        assert!(
            parse(&serde_json::to_vec(&closed).unwrap()).is_err(),
            "{key}"
        );
    }
}
