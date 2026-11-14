use super::*;

#[test]
fn divisions_follow_note_lengths() {
    let at = |division: Division| division.seconds(120.0);
    assert_eq!(at(Division::Quarter), 0.5);
    assert_eq!(at(Division::EighthDotted), 0.375);
    assert!((at(Division::QuarterTriplet) - 1.0 / 3.0).abs() < 1e-12);
    assert_eq!(at(Division::Whole), 2.0);
    assert_eq!(at(Division::SixteenthDotted), 0.1875);
    let labels: Vec<_> = Division::ALL.iter().map(|d| d.label()).collect();
    assert_eq!(labels[0], "1/1");
    assert_eq!(labels[14], "1/16T");
    for division in Division::ALL {
        let json = serde_json::to_string(&division).unwrap();
        assert_eq!(json, format!("\"{}\"", division.label()));
    }
}

#[test]
fn every_preset_is_valid_and_the_defaults_are_the_first() {
    assert_eq!(
        Echoform::preset("Wide Tape Echo"),
        Some(Echoform::default())
    );
    assert_eq!(
        Aetherform::preset("Warm Chamber"),
        Some(Aetherform::default())
    );
    for (name, mix) in Echoform::PRESETS {
        Echoform::preset(name).unwrap().validate().unwrap();
        assert!((0.0..=100.0).contains(&mix));
    }
    for (name, mix) in Aetherform::PRESETS {
        Aetherform::preset(name).unwrap().validate().unwrap();
        assert!((0.0..=100.0).contains(&mix));
    }
    assert_eq!(Echoform::preset("Nope"), None);
    assert_eq!(Emberform::preset("Warm Tube"), Some(Emberform::default()));
    let mut names = Vec::new();
    for (name, mix) in Emberform::PRESETS {
        let drive = Emberform::preset(name).unwrap();
        drive.validate().unwrap();
        assert!((0.0..=100.0).contains(&mix));
        assert!(!names.contains(&name), "{name} repeats");
        names.push(name);
    }
    // Every curve has a preset.
    for shape in DistortionShape::ALL {
        assert!(
            Emberform::PRESETS
                .iter()
                .any(|(name, _)| Emberform::preset(name).unwrap().shape == shape),
            "{shape:?}"
        );
    }
}

#[test]
fn a_drive_saves_its_shape_and_holds_its_ranges() {
    let mut buses = FxBuses::default();
    buses.buses[3].module = Module::EmberformDrive(Emberform::preset("Industrial Clip").unwrap());
    let json = serde_json::to_value(buses).unwrap();
    assert_eq!(json[3]["module"]["type"], "emberform-drive");
    assert_eq!(json[3]["module"]["shape"], "hard-clip");
    assert!(json[3]["module"].get("duckKey").is_none());
    let back: FxBuses = serde_json::from_value(json).unwrap();
    assert_eq!(back, buses);
    let mut drive = Emberform::default();
    DistortionParam::Bits.set(&mut drive, 40.0);
    assert_eq!(drive.bits, 16);
    DistortionParam::Drive.set(&mut drive, f32::NAN);
    assert_eq!(drive.drive_db, 12.0);
    drive.bits = 0;
    assert!(drive.validate().is_err());
}

#[test]
fn buses_save_with_the_v7_module_names() {
    let mut buses = FxBuses::default();
    buses.buses[1].module = Module::EchoformDelay(Echoform::default());
    buses.buses[2].module = Module::AetherformReverb(Aetherform::default());
    let json = serde_json::to_value(buses).unwrap();
    assert_eq!(json[0]["module"], serde_json::json!({"type": "empty"}));
    assert_eq!(json[1]["module"]["type"], "echoform-delay");
    assert_eq!(json[1]["module"]["divisionR"], "1/8.");
    assert_eq!(json[2]["module"]["type"], "aetherform-reverb");
    assert_eq!(json[2]["module"]["shimmerIntervalSemitones"], 12);
    assert_eq!(json[0]["returnLevel"], 1.0);
    let back: FxBuses = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back, buses);
    let mut extra = json;
    extra[1]["module"]["mix"] = serde_json::json!(1);
    assert!(serde_json::from_value::<FxBuses>(extra).is_err());
}

#[test]
fn controls_hold_their_ranges() {
    let mut delay = Echoform::default();
    DelayParam::Feedback.set(&mut delay, 500.0);
    assert_eq!(delay.feedback, 110.0);
    DelayParam::TimeL.set(&mut delay, f32::NAN);
    assert_eq!(delay.time_ms_l, 420.0);
    let mut reverb = Aetherform::default();
    ReverbParam::Decay.set(&mut reverb, 0.0);
    assert_eq!(reverb.decay_seconds, 0.2);
    reverb.shimmer_interval_semitones = 5;
    assert!(reverb.validate().is_err());
    let mut bus = FxBus {
        return_level: 1.5,
        ..FxBus::default()
    };
    assert!(bus.validate().is_err());
    bus.return_level = 0.5;
    bus.validate().unwrap();
}
