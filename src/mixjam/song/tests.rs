mod lane_versions;
mod master_versions;
mod roundtrip;

use super::*;
use super::{
    migration::{BOOSTER_KEYS, threshold_without_master_gain},
    model::Document,
    reading::parse,
    writing::{existing_creation_time, save, save_with_limit, timestamp},
};
use crate::library::Library;
use crate::{
    engine::{
        Clip, Fades, ImportNotice, ImportedSample, MissingSample, Project, SampleConflict,
        TRACK_WIDTH, Track,
    },
    master::{MasterChain, PedalKind},
    mixjam::MAX_SONG_BYTES,
    products::{LibraryBinding, Product},
};
use std::{collections::BTreeMap, fs, path::PathBuf};

fn project() -> Project {
    let imported = ImportedSample {
        id: crate::products::IMPORTED_ID_START,
        name: "Lead".into(),
        group_name: "Wave".into(),
        path: PathBuf::from("waves/lead.wav"),
        source_bpm: 140.0,
        source_sample_rate: Some(22_050),
        origin: Some(Product::T2),
    };
    let mut lost = imported.clone();
    lost.id += 1;
    let mut missing = MissingSample::external(&lost, 512, "Missing file.".into());
    missing
        .metadata
        .insert("Playback timing policy".into(), "mixjam-span".into());
    let clip = |id, sample_id, track| Clip {
        id,
        sample_id,
        track,
        start_tick: 256,
        length_ticks: 512,
        offset_frames: 4_410,
        gain: 0.75,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    };
    let mut project = Project {
        title: "Night drive".into(),
        author: "Owner".into(),
        bpm: 160.0,
        master_gain: 0.5,
        loop_region: Some([256, 1_024]),
        imported_samples: vec![imported.clone()].into(),
        missing_samples: vec![
            missing,
            MissingSample::native(
                Product::T2,
                7,
                "Kick".into(),
                140.0,
                256,
                false,
                "No library.".into(),
            )
            .unwrap(),
        ],
        // Arrangement order differs from lane order.
        clips: vec![
            clip(1, imported.id, 63),
            clip(2, lost.id, 0),
            clip(3, Product::T2.global_sample_id(7).unwrap(), 5),
        ],
        sample_conflicts: vec![SampleConflict {
            reference: "lead.wav".into(),
            sample_ids: vec![imported.id],
            candidates: vec!["a/lead.wav".into(), "b/lead.wav".into()],
            selected: 1,
            confirmed: true,
            source_spans: BTreeMap::from([(imported.id, 512)]),
        }],
        import_notices: vec![ImportNotice {
            source_bpm: 155.0,
            target_bpm: 160.0,
        }],
        source_notices: vec!["Imported.".into()],
        source_settings: BTreeMap::from([
            ("BoostCompressorLED".into(), "1".into()),
            ("DrumU1_0".into(), String::new()),
        ]),
        libraries: vec![LibraryBinding {
            product: Product::T2,
            root: "library".into(),
        }],
        ..Project::default()
    };
    project.tracks[5] = Track {
        gain: 0.25,
        polarity_inverted: true,
        pan: -0.5,
        left_enabled: false,
        right_enabled: true,
        muted: true,
        solo: false,
        sends: [0.25, 0.0, 0.5, 1.0],
        automation: Default::default(),
        eq: crate::eq::ChannelEq {
            on: true,
            low_cut: 80.0,
            high_db: -7.5,
            ..Default::default()
        },
        drive: Default::default(),
        compressor: crate::compressor::Compressor {
            on: true,
            threshold: -24.0,
            makeup: 3.0,
            ..Default::default()
        },
        width: 150.0,
        ducker: crate::ducker::Ducker {
            on: true,
            key: 1,
            depth: 9.5,
            ..Default::default()
        },
    };
    project.tracks[6].solo = true;
    // An EQ that is off keeps its settings.
    project.tracks[7].eq.mid_db = 3.0;
    project.tracks[7].eq.peak2_db = -4.5;
    project.tracks[9].width = 0.0;
    project.clips[0].fades = Fades {
        fade_in: crate::engine::Fade {
            ticks: 64,
            curve: crate::engine::FadeCurve::EqualPower,
        },
        fade_out: crate::engine::Fade {
            ticks: 128,
            curve: crate::engine::FadeCurve::Exponential,
        },
    };
    project.clips[0].pitch_cents = -150;
    project.tracks[5].automation.volume = vec![
        crate::automation::Point {
            tick: 0,
            value: -60.0,
        },
        crate::automation::Point {
            tick: 512,
            value: 0.0,
        },
    ];
    project.tracks[5].automation.sends[1] = vec![crate::automation::Point {
        tick: 256,
        value: 0.75,
    }];
    project.tracks[5].automation.filter = vec![crate::automation::Point {
        tick: 1024,
        value: -0.5,
    }];
    project.fx.buses[0].module =
        crate::fx::Module::AetherformReverb(crate::fx::Aetherform::preset("Dark Hall").unwrap());
    project.fx.buses[0].return_level = 0.92;
    project.fx.buses[2].module =
        crate::fx::Module::EchoformDelay(crate::fx::Echoform::preset("Dub Feedback").unwrap());
    project.fx.buses[2].powered = false;
    project.fx.buses[3].limiter = false;
    project.master.booster_compressor.on = true;
    project.master.booster_compressor.drive = 368;
    project.master.booster_eq.bands[9] = 0;
    project.master.move_pedal(2, 0);
    project.master.trim = -2.5;
    project.master.limiter.on = true;
    project.master.limiter.ceiling = -0.3;
    project.master.tape.speed = 15;
    project.master.bus_compressor.attack = 0.1;
    project.master.bypass = true;
    project
}

/// The test song with a pan lane on track 6, as version 20 saves it.
fn panned(mut project: Project) -> Project {
    project.tracks[5].automation.pan = [(0, -1.0), (64, 1.0), (128, 0.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    project
}

/// The test song with a 1/8, 1/16, 1/32 roll on track 6, as version 21
/// saves it.
fn rolled(mut project: Project) -> Project {
    project.tracks[5].automation.repeat = [(256, 8.0), (320, 16.0), (384, 32.0), (512, 0.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    project
}

/// The test song with a tape-stop on track 6's Pitch lane, as version
/// 24 saves it.
fn stopped(mut project: Project) -> Project {
    project.tracks[5].automation.pitch = [(768, 0.0), (832, -4800.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    project
}

/// The test song with a drive on track 6, and one that is off but keeps
/// its settings on track 9, as version 22 saves them.
fn driven(mut project: Project) -> Project {
    project.tracks[5].drive = crate::drive::Drive {
        on: true,
        amount: 18.0,
        character: 60.0,
        mix: 40.0,
        focus: 98.0,
        output: -1.5,
    };
    project.tracks[8].drive.amount = 6.0;
    project
}

/// The test song with version 23's tools: a throw on FX 2's return lane,
/// a Repeat gate on track 6, and the clip on track 64 reversed.
fn thrown(mut project: Project) -> Project {
    let points = |pairs: &[(u32, f32)]| -> Vec<crate::automation::Point> {
        pairs
            .iter()
            .map(|&(tick, value)| crate::automation::Point { tick, value })
            .collect()
    };
    project.return_automation[1] = points(&[(0, 0.25), (448, 0.25), (480, 2.0), (512, 0.25)]);
    project.tracks[5].automation.repeat_gate = points(&[(256, 25.0), (384, 60.0)]);
    project.clips[0].reverse = true;
    project
}

/// The test song with two sections, as version 19 saves them.
fn sectioned(project: Project) -> Project {
    Project {
        sections: vec![
            crate::engine::Section {
                start_tick: 0,
                name: "Intro".into(),
            },
            crate::engine::Section {
                start_tick: 2_048,
                name: "Drift, part I".into(),
            },
        ],
        ..project
    }
}

/// A track as a version 17 song saves it: no second EQ peak, no
/// compressor, and 100% width.
fn before_version_18(track: &mut Track) {
    use crate::eq::EqParam;
    for param in [EqParam::Peak2Hz, EqParam::Peak2Db, EqParam::Peak2Q] {
        param.set(&mut track.eq, param.range().default);
    }
    track.compressor = Default::default();
    track.width = TRACK_WIDTH.default;
}

/// The same song as an earlier format version saves it: no master
/// pedalboard, no FX buses, no sends, no clip fades or pitch, no
/// automation, and no channel EQ.
fn earlier_document(expected: &mut Project, version: u32) -> serde_json::Value {
    expected.master = MasterChain::default();
    expected.fx = crate::fx::FxBuses::default();
    for track in &mut expected.tracks {
        track.sends = [0.0; crate::fx::BUSES];
        track.automation = Default::default();
        track.eq = Default::default();
        track.ducker = Default::default();
        before_version_18(track);
    }
    for clip in &mut expected.clips {
        clip.fades = Fades::default();
        clip.pitch_cents = 0;
    }
    let mut document = serde_json::to_value(Document::new(
        expected,
        "2020-01-02T03:04:05Z".into(),
        timestamp(0),
    ))
    .unwrap();
    document["formatVersion"] = version.into();
    document.as_object_mut().unwrap().remove("master");
    document.as_object_mut().unwrap().remove("fxBuses");
    for lane in document["lanes"].as_array_mut().unwrap() {
        lane.as_object_mut().unwrap().remove("sends");
    }
    document
}

/// The board of a version 11 to 15 document: the Booster pedals'
/// current keys, in the record and its order, changed to the earlier ones.
fn with_earlier_booster_keys(document: &mut serde_json::Value) {
    let record = document["master"].as_object_mut().unwrap();
    for (earlier, current) in BOOSTER_KEYS {
        if let Some(pedal) = record.remove(current) {
            record.insert(earlier.into(), pedal);
        }
    }
    for name in record["order"].as_array_mut().unwrap() {
        if let Some((earlier, _)) = BOOSTER_KEYS
            .iter()
            .find(|(_, current)| name.as_str() == Some(*current))
        {
            *name = (*earlier).into();
        }
    }
}
