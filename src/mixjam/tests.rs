use super::v7::{Document, PROCESSORS, convert, validate};
use super::*;
use crate::master::{MasterChain, PedalKind};
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn unsupported_song_version_lists_all_supported_versions() {
    let path = std::env::temp_dir().join(format!(
        "mixjam-unsupported-version-{}.mixjam",
        std::process::id()
    ));
    let mut document = fixture();
    document["formatVersion"] = json!(25);
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let error = load_song(&path, &Library::empty()).unwrap_err();
    fs::remove_file(path).unwrap();
    assert_eq!(error, "Only MixJam format versions 7 to 24 are supported.");
}

#[test]
fn the_master_bus_plays_through_the_mastering_pedals() {
    let document = |bus: Value| {
        let mut value = fixture();
        value["masterBus"] = bus;
        serde_json::from_value::<Document>(value).unwrap()
    };
    let plain = convert(
        document(fixture()["masterBus"].clone()),
        Path::new("song.mixjam"),
        &Library::empty(),
    )
    .unwrap();
    assert_eq!(
        plain.master,
        MasterChain::default(),
        "A bypassed bus imports with every pedal off."
    );

    let mut order: Vec<&str> = PROCESSORS.to_vec();
    order.reverse();
    // The limiter leads this bus, and imports last all the same.
    let power: HashMap<String, bool> = PROCESSORS
        .iter()
        .map(|id| (id.to_string(), ["lim", "tape", "width"].contains(id)))
        .collect();
    let bus = json!({"order": order, "power": power, "preset": null, "params": {
        "gain.trim": -6.5, "lim.ceil": -0.3, "lim.gain": 7, "tape.ips": 0, "width.mono": 95,
        "comp.att": 0.25, "subeq.mud": -2.5,
    }});
    let project = convert(
        document(bus.clone()),
        Path::new("song.mixjam"),
        &Library::empty(),
    )
    .unwrap();
    let master = project.master;
    assert_eq!(master.order[..3], PedalKind::ALL[..3]);
    assert_eq!(master.order[3], PedalKind::Multiband);
    assert_eq!(master.order[11], PedalKind::SoftClip);
    assert_eq!(master.order[12], PedalKind::Limiter);
    assert!(master.validate().is_ok());
    assert!(master.limiter.on && master.tape.on && master.imaging.on && !master.tube.on);
    assert!(!master.booster_eq.on && !master.booster_compressor.on && !master.booster_wide.on);
    assert_eq!(
        (master.trim, master.limiter.ceiling, master.limiter.gain),
        (-6.5, -0.3, 7.0)
    );
    assert_eq!((master.tape.speed, master.imaging.mono_below), (15, 95.0));
    assert_eq!(
        (master.bus_compressor.attack, master.trim_eq.mud),
        (0.25, -2.5)
    );
    // Parameters the file leaves out keep their defaults.
    assert_eq!(master.soft_clip, crate::master::SoftClip::default());
    assert!(project.source_notices[0].contains("mastering pedals"));

    for (key, value) in [
        ("lim.ceil", json!(0.5)),
        ("tape.ips", json!(0.5)),
        ("reverb.size", json!(1)),
        ("mbc.lo", json!(-1)),
    ] {
        let mut changed = bus.clone();
        changed["params"][key] = value;
        assert!(
            convert(
                document(changed),
                Path::new("song.mixjam"),
                &Library::empty()
            )
            .unwrap_err()
            .contains("master bus settings"),
            "{key}"
        );
    }
}

fn fixture() -> Value {
    json!({"formatVersion":7,"appVersion":"1.0","createdAt":"2026-01-01T00:00:00Z","modifiedAt":"2026-01-01T00:00:00Z",
        "song":{"bpm":127,"masterGain":0.8,"clipEdgeMicroFades":{"enabled":false,"fadeInMs":2,"fadeOutMs":4}},
        "lanes":[{"id":"lane-1","name":"Drums","gain":0.6,"muted":true,"solo":true,"pan":-0.25,"sends":[0,0,0,0],"placements":[
            {"id":"a","sampleRef":"kick.wav","sampleName":"Kick","nativeBPM":140,"startTick":4,"durationTicks":32,"durationSeconds":2,"slot":null},
            {"id":"b","sampleRef":"kick.wav","sampleName":"Kick","nativeBPM":140,"startTick":20,"durationTicks":32,"durationSeconds":2,"slot":null}]}],
        "fxBuses":(0..4).map(|i| json!({"id":format!("fx-{}",i+1),"index":i,"name":format!("FX{}",i+1),"module":{"type":"empty"},"powered":true,"returnLevel":1,"limiterEnabled":true})).collect::<Vec<_>>(),
        "masterBus":{"order":PROCESSORS,"power":PROCESSORS.iter().map(|id| (id.to_string(),false)).collect::<HashMap<_,_>>(),"params":{"gain.trim":0},"preset":"Bypass All"}})
}

#[test]
fn v7_exact_source_spans_survive_octave_playback_and_save_reopen() {
    let root = std::env::temp_dir().join(format!(
        "mixjam-exact-span-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let wave = root.join("kick.wav");
    crate::audio::Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 44_100],
    }
    .write_wav(&wave)
    .unwrap();
    let mut library = Library::empty();
    library
        .import_wav_with_tempo(&wave, None, "Drum", 140.0)
        .unwrap();
    let mut document = fixture();
    document["song"]["bpm"] = json!(140);
    document["lanes"][0]["placements"] = json!([{
        "id":"a", "sampleRef":"kick.wav", "sampleName":"Kick",
        "nativeBPM":140, "startTick":0, "durationTicks":32,
        "durationSeconds":1, "slot":null
    }]);
    let input = root.join("original.mixjam");
    fs::write(&input, serde_json::to_vec(&document).unwrap()).unwrap();
    let (project, ready) = crate::project_io::open(&input, &library).unwrap();
    assert_eq!(project.imported_samples[0].source_bpm, 240.0);
    let rendered = crate::audio::render_project(&project, &ready).unwrap();
    assert_eq!(rendered.frames(), 75_600);
    assert!(
        rendered.samples.iter().all(|sample| *sample > 0.0),
        "The one-second source must fill its saved one-bar span."
    );
    let saved = root.join("saved.mixjam");
    crate::project_io::save(&project, &saved).unwrap();
    let (reopened, ready) = crate::project_io::open(&saved, &Library::empty()).unwrap();
    let rerendered = crate::audio::render_project(&reopened, &ready).unwrap();
    assert_eq!(rerendered.samples, rendered.samples);
    for file in [saved, input, wave] {
        fs::remove_file(file).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn v7_compressor_attack_accepts_exact_decimal_endpoints() {
    for attack in [0.1_f64, 30.0] {
        let mut value = fixture();
        value["masterBus"]["params"]["comp.att"] = json!(attack);
        let project = convert(
            serde_json::from_value(value).unwrap(),
            Path::new("song.mixjam"),
            &Library::empty(),
        )
        .unwrap();
        assert_eq!(project.master.bus_compressor.attack, attack as f32);
    }
    // Reject outside values before f32 conversion rounds them to an endpoint.
    for attack in [0.1_f64.next_down(), 30.0_f64.next_up()] {
        let mut value = fixture();
        value["masterBus"]["params"]["comp.att"] = json!(attack);
        let document = serde_json::from_value(value).unwrap();
        assert!(
            validate(&document)
                .unwrap_err()
                .contains("master bus settings")
        );
    }
}

#[test]
fn rejects_invalid_settings_and_unsafe_paths_before_audio_resolution() {
    for (pointer, value, expected) in [
        (
            "/song/clipEdgeMicroFades/fadeInMs",
            json!(21),
            "fade settings",
        ),
        ("/masterBus/params/gain.trim", json!(25), "gain trim"),
        (
            "/lanes/0/placements/0/sampleRef",
            json!("../kick.wav"),
            "invalid identity",
        ),
        (
            "/lanes/0/placements/1/durationTicks",
            json!(33),
            "same source span",
        ),
        ("/formatVersion", json!(8), "version 7"),
    ] {
        let mut value_doc = fixture();
        *value_doc.pointer_mut(pointer).unwrap() = value;
        let document = serde_json::from_value(value_doc).unwrap();
        assert!(
            convert(document, Path::new("song.mixjam"), &Library::empty())
                .unwrap_err()
                .contains(expected),
            "{pointer}"
        );
    }
}

#[test]
fn wav_only_import_converts_clock_solo_and_monophonic_spans() {
    let root = std::env::temp_dir().join(format!("mixjam-import-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let wave = root.join("kick.wav");
    let mut writer = hound::WavWriter::create(
        &wave,
        hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..16000 {
        writer.write_sample(1000i16).unwrap();
    }
    writer.finalize().unwrap();
    let mut library = Library::empty();
    library
        .import_wav_with_tempo(&wave, None, "Drum", 140.0)
        .unwrap();
    let project = convert(
        serde_json::from_value(fixture()).unwrap(),
        Path::new("song.mixjam"),
        &library,
    )
    .unwrap();
    assert_eq!(project.bpm, 125.0);
    assert_eq!(project.import_notices[0].source_bpm, 127.0);
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.clips[0].start_tick, 32);
    assert_eq!(project.clips[0].length_ticks, 128);
    assert_eq!(project.clips[1].length_ticks, 256);
    assert_eq!(project.imported_samples.len(), 1);
    assert_eq!(project.imported_samples[0].source_bpm, 120.0);
    assert!(project.tracks[0].left_enabled && project.tracks[0].right_enabled);
    // MixJam plays a muted solo lane, so it imports soloed and unmuted.
    assert!(project.tracks[0].solo && !project.tracks[0].muted);
    assert!(!project.tracks[1].audible(project.any_solo()));
    assert_eq!(project.tracks[0].pan, -0.25);
    assert_eq!(project.tracks[0].gain, 0.6);
    assert!(project.libraries.is_empty());
    let mut active = fixture();
    active["song"]["clipEdgeMicroFades"]["enabled"] = json!(true);
    active["masterBus"]["power"]["lim"] = json!(true);
    active["fxBuses"][0]["module"]["type"] = json!("reverb");
    active["lanes"][0]["placements"][1]["sampleRef"] = json!("relocated/kick.wav");
    active["lanes"][0]["placements"][1]["durationTicks"] = json!(64);
    let projected = convert(
        serde_json::from_value(active).unwrap(),
        Path::new("song.mixjam"),
        &library,
    )
    .unwrap();
    assert_eq!(projected.imported_samples.len(), 2);
    assert_ne!(projected.clips[0].sample_id, projected.clips[1].sample_id);
    assert_eq!(projected.imported_samples[1].source_bpm, 240.0);
    assert!(projected.source_notices[0].contains("not rendered"));
    assert!(projected.master.limiter.on);
    assert!(projected.source_notices[1].contains("\"lim\":true"));
    assert!(projected.source_notices[1].contains("\"type\":\"reverb\""));
    assert!(projected.fx.buses[0].module.is_empty());
    fs::remove_file(wave).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn return_buses_and_sends_play_through_the_application_effects() {
    let module = |value: serde_json::Value| {
        let mut value = value;
        value["id"] = json!("fx-module-1");
        value
    };
    let delay = crate::fx::Echoform::preset("Dotted Motion").unwrap();
    let reverb = crate::fx::Aetherform::preset("Vocal Plate").unwrap();
    let mut document = fixture();
    document["fxBuses"][1]["module"] =
        module(serde_json::to_value(crate::fx::Module::EchoformDelay(delay)).unwrap());
    document["fxBuses"][1]["returnLevel"] = json!(0.78);
    document["fxBuses"][2]["module"] =
        module(serde_json::to_value(crate::fx::Module::AetherformReverb(reverb)).unwrap());
    document["fxBuses"][2]["powered"] = json!(false);
    document["fxBuses"][2]["limiterEnabled"] = json!(false);
    document["lanes"][0]["sends"] = json!([0, 0.5, 0.25, 0]);
    let project = convert(
        serde_json::from_value(document.clone()).unwrap(),
        Path::new("song.mixjam"),
        &Library::empty(),
    )
    .unwrap();
    let buses = &project.fx.buses;
    assert!(buses[0].module.is_empty());
    assert_eq!(buses[1].module, crate::fx::Module::EchoformDelay(delay));
    assert_eq!(buses[1].return_level, 0.78);
    assert_eq!(buses[2].module, crate::fx::Module::AetherformReverb(reverb));
    assert!(!buses[2].powered && !buses[2].limiter);
    assert_eq!(project.tracks[0].sends, [0.0, 0.5, 0.25, 0.0]);
    // A module of a known type with a setting out of range is refused.
    document["fxBuses"][1]["module"]["feedback"] = json!(500);
    let error = convert(
        serde_json::from_value(document).unwrap(),
        Path::new("song.mixjam"),
        &Library::empty(),
    )
    .unwrap_err();
    assert!(error.contains("Echoform"), "{error}");
}

#[test]
fn legacy_folder_recovery_survives_save_reopen_and_resolves_saved_placeholders() {
    let root = std::env::temp_dir().join(format!("mixjam-legacy-layout-{}", std::process::id()));
    let selected = root.join("_original");
    let legacy = root.join("House/Drum/kick.wav");
    fs::create_dir_all(&selected).unwrap();
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    let write = |path: &Path, frames| {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..frames {
            writer.write_sample(1000_i16).unwrap();
        }
        writer.finalize().unwrap();
    };
    write(&legacy, 16000);
    // A same-basename palette sound must not replace the full saved reference.
    write(&selected.join("kick.wav"), 8000);
    let library = Library::open_wav_folder(&selected, 140.0).unwrap();
    let mut source = fixture();
    for placement in source["lanes"][0]["placements"].as_array_mut().unwrap() {
        placement["sampleRef"] = json!("House/Drum/kick.wav");
    }
    let input = root.join("old.mixjam");
    fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    let (opened, _) = crate::project_io::open(&input, &library).unwrap();
    assert!(opened.missing_samples.is_empty());
    let id = opened.clips[0].sample_id;
    let audio = opened
        .imported_samples
        .iter()
        .find(|sample| sample.id == id)
        .unwrap();
    assert_eq!(audio.path, fs::canonicalize(&legacy).unwrap());
    assert_eq!(audio.source_bpm, 120.0);
    assert_eq!(opened.clips[1].sample_id, id);

    let saved = root.join("saved.mixjam");
    crate::project_io::save(&opened, &saved).unwrap();
    let (reopened, _) = crate::project_io::open(&saved, &library).unwrap();
    assert_eq!(reopened.clips, opened.clips);
    assert_eq!(
        reopened
            .imported_samples
            .iter()
            .find(|sample| sample.id == id)
            .unwrap()
            .path,
        audio.path
    );

    let (missing, _) = crate::project_io::open(&input, &Library::empty()).unwrap();
    assert_eq!(missing.missing_samples.len(), 1);
    crate::project_io::save(&missing, &saved).unwrap();
    let (restored, _) = crate::project_io::open(&saved, &library).unwrap();
    assert!(restored.missing_samples.is_empty());
    assert_eq!(restored.clips, missing.clips);
    let restored_audio = restored
        .imported_samples
        .iter()
        .find(|sample| sample.id == restored.clips[0].sample_id)
        .unwrap();
    assert_eq!(restored_audio.path, audio.path);
    assert_eq!(restored_audio.source_bpm, 120.0);

    // A moved reference with two matches opens audibly with a conflict choice.
    for placement in source["lanes"][0]["placements"].as_array_mut().unwrap() {
        placement["sampleRef"] = json!("Old/kick.wav");
    }
    fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    let (conflicted, conflict_library) = crate::project_io::open(&input, &library).unwrap();
    assert!(conflicted.missing_samples.is_empty());
    assert_eq!(conflicted.sample_conflicts.len(), 1);
    let conflict = &conflicted.sample_conflicts[0];
    assert_eq!(conflict.candidates.len(), 2);
    assert_eq!(conflict.source_spans[&conflicted.clips[0].sample_id], 256);
    assert!(!conflict.confirmed);
    let before_clips = conflicted.clips.clone();
    let mut session =
        crate::application::Session::headless(std::sync::Arc::new(conflict_library), conflicted)
            .unwrap();
    session.resolve_sample_conflict(0, 1).unwrap();
    assert_eq!(session.project().clips, before_clips);
    let selected_id = session.project().clips[0].sample_id;
    let selected_audio = session
        .project()
        .imported_samples
        .iter()
        .find(|sample| sample.id == selected_id)
        .unwrap();
    assert_eq!(selected_audio.source_bpm, 240.0);
    assert!(session.sample_conflicts()[0].confirmed);
    crate::project_io::save(session.project(), &saved).unwrap();
    let (reopened, _) = crate::project_io::open(&saved, &library).unwrap();
    assert_eq!(
        reopened.sample_conflicts,
        session.project().sample_conflicts
    );
    assert_eq!(
        reopened
            .imported_samples
            .iter()
            .find(|sample| sample.id == selected_id)
            .unwrap()
            .path,
        selected_audio.path
    );
    fs::remove_dir_all(root).unwrap();
}
