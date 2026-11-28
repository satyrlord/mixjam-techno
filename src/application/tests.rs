use super::*;

mod conflicts;
mod preferences;
mod state;
mod stretch;

#[test]
fn placement_ticks_reports_the_length_that_place_uses() {
    let sample = |id, frame_count| Sample {
        id,
        code: "FIXTURE".into(),
        name: "fixture".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 0,
        frame_count,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    };
    // No product owns this ID, so its length comes from the audio.
    let wave = 900_001;
    assert!(Product::sample_key(wave).is_none());
    let library = Library::fixture(vec![sample(1, 75_600), sample(wave, 151_200)]);
    let mut session = Session::headless(Arc::new(library), Project::default()).unwrap();
    assert_eq!(session.placement_ticks(1), Some(TICKS_PER_BAR));
    assert_eq!(session.placement_ticks(wave), Some(2 * TICKS_PER_BAR));
    assert_eq!(session.placement_ticks(7), None);
    for (track, id) in [1, wave].into_iter().enumerate() {
        let clip = session.place(id, track, 0).unwrap();
        let placed = session
            .project()
            .clips
            .iter()
            .find(|c| c.id == clip)
            .unwrap();
        assert_eq!(Some(placed.length_ticks), session.placement_ticks(id));
    }
}

fn manual_fixture_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "mixjam-manual-resolution-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
fn conflict_fixture() -> (Session, PathBuf) {
    let root = manual_fixture_dir();
    let first = root.join("first.wav");
    let second = root.join("second.wav");
    for (path, rate, frames, value) in [
        (&first, 22_050, 22_050, 0.25),
        (&second, 44_100, 88_200, -0.25),
    ] {
        Pcm {
            sample_rate: rate,
            channels: 1,
            samples: vec![value; frames],
        }
        .write_wav(path)
        .unwrap();
    }
    let first = std::fs::canonicalize(first).unwrap();
    let second = std::fs::canonicalize(second).unwrap();
    let sources: Vec<_> = [256, 512]
        .into_iter()
        .enumerate()
        .map(|(index, ticks)| crate::engine::ImportedSample {
            id: 1_000_000 + index as u32,
            name: "Lead".into(),
            group_name: "MixJam".into(),
            path: first.clone(),
            source_bpm: f64::from(ticks) * 240.0 / f64::from(TICKS_PER_BAR),
            source_sample_rate: Some(22_050),
            origin: None,
        })
        .collect();
    let mut library = Library::empty();
    library.apply_imports(&sources).unwrap();
    let project = Project {
        imported_samples: sources.into(),
        sample_conflicts: vec![SampleConflict {
            reference: "Old/lead.wav".into(),
            sample_ids: vec![1_000_000, 1_000_001],
            candidates: vec![first, second],
            selected: 0,
            confirmed: false,
            source_spans: [(1_000_000, 256), (1_000_001, 512)].into(),
        }],
        clips: [1_000_000, 1_000_001, 1_000_000]
            .into_iter()
            .enumerate()
            .map(|(index, id)| Clip {
                id: index as u64 + 1,
                sample_id: id,
                track: index * 4,
                start_tick: 17,
                length_ticks: 256,
                offset_frames: 11_025,
                gain: 0.75,
                fades: Default::default(),
                pitch_cents: 0,
                reverse: false,
            })
            .collect(),
        ..Project::default()
    };
    (Session::headless(Arc::new(library), project).unwrap(), root)
}

fn fixture() -> Session {
    let sample = Sample {
        id: 1,
        code: "FIXTURE".into(),
        name: "warm pulse 2".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 0,
        frame_count: 75600,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    };
    let library = Arc::new(Library::fixture(vec![sample]));
    let mut editor = Editor::new(Project::default());
    editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
    editor.place(1, 1, TICKS_PER_BAR, TICKS_PER_BAR).unwrap();
    Session::headless(library, editor.into_project()).unwrap()
}

#[test]
fn in_place_continuing_steps_match_full_transaction_across_controls() {
    let setup = || -> (Session, u64, u64) {
        let mut s = fixture();
        s.gesture_gap = std::time::Duration::MAX;
        let ids: Vec<u64> = s.project().clips.iter().map(|c| c.id).collect();
        (s, ids[0], ids[1])
    };
    let (mut session, clip1, clip2) = setup();

    // 1. SetTempo (valid & invalid)
    session.edit(EditCommand::SetTempo(135.0)).unwrap();
    let mut ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    let gen0 = session.editor.generation();
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    ref_s.edit_discrete(EditCommand::SetTempo(160.0)).unwrap();
    assert_eq!(session.project(), ref_s.project());
    assert_eq!(session.editor.generation(), gen0 + 1);
    let err_in_place = session.edit(EditCommand::SetTempo(123.0)).unwrap_err();
    let err_full = ref_s
        .edit_discrete(EditCommand::SetTempo(123.0))
        .unwrap_err();
    assert_eq!(err_in_place, err_full);
    assert_eq!(session.project(), ref_s.project());

    // 2. SetTrack + SetMaster + SetEffects + SetAutomation + SetReturnAutomation + SetClip + SetClipSounds
    let mut t = session.project().tracks[0].clone();
    t.gain = 0.7;
    session
        .edit(EditCommand::SetTrack {
            index: 0,
            track: Box::new(t.clone()),
        })
        .unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    t.gain = 0.8;
    session
        .edit(EditCommand::SetTrack {
            index: 0,
            track: Box::new(t.clone()),
        })
        .unwrap();
    ref_s
        .edit_discrete(EditCommand::SetTrack {
            index: 0,
            track: Box::new(t.clone()),
        })
        .unwrap();
    assert_eq!(session.project(), ref_s.project());
    let mut bad_t = session.project().tracks[0].clone();
    bad_t.gain = -1.0;
    assert_eq!(
        session
            .edit(EditCommand::SetTrack {
                index: 0,
                track: Box::new(bad_t.clone()),
            })
            .unwrap_err(),
        ref_s
            .edit_discrete(EditCommand::SetTrack {
                index: 0,
                track: Box::new(bad_t),
            })
            .unwrap_err()
    );
    assert_eq!(session.project(), ref_s.project());

    let mut mixer_tracks = session.project().tracks.clone();
    mixer_tracks[1].gain = 0.7;
    session
        .edit(EditCommand::SetMixer {
            tracks: mixer_tracks.clone(),
            master_gain: 0.9,
        })
        .unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    mixer_tracks[1].gain = 0.8;
    session
        .edit(EditCommand::SetMixer {
            tracks: mixer_tracks.clone(),
            master_gain: 0.9,
        })
        .unwrap();
    ref_s
        .edit_discrete(EditCommand::SetMixer {
            tracks: mixer_tracks,
            master_gain: 0.9,
        })
        .unwrap();
    assert_eq!(session.project(), ref_s.project());

    let mut master = session.project().master;
    master.trim = -2.0;
    session.edit(EditCommand::SetMaster(master)).unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    master.trim = -4.0;
    session.edit(EditCommand::SetMaster(master)).unwrap();
    ref_s.edit_discrete(EditCommand::SetMaster(master)).unwrap();
    assert_eq!(session.project(), ref_s.project());

    let mut fx = session.project().fx;
    fx.buses[0].return_level = 0.4;
    session.edit(EditCommand::SetEffects(fx)).unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    fx.buses[0].return_level = 0.6;
    session.edit(EditCommand::SetEffects(fx)).unwrap();
    ref_s.edit_discrete(EditCommand::SetEffects(fx)).unwrap();
    assert_eq!(session.project(), ref_s.project());

    let c1 = session
        .project()
        .clips
        .iter()
        .find(|c| c.id == clip1)
        .unwrap()
        .clone();
    session
        .edit(EditCommand::SetClip {
            id: clip1,
            start_tick: c1.start_tick,
            gain: 0.9,
            offset_frames: c1.offset_frames,
            length_ticks: c1.length_ticks,
            fades: c1.fades,
            pitch_cents: c1.pitch_cents,
            reverse: c1.reverse,
        })
        .unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    session
        .edit(EditCommand::SetClip {
            id: clip1,
            start_tick: c1.start_tick,
            gain: 0.5,
            offset_frames: c1.offset_frames,
            length_ticks: c1.length_ticks,
            fades: c1.fades,
            pitch_cents: c1.pitch_cents,
            reverse: c1.reverse,
        })
        .unwrap();
    ref_s
        .edit_discrete(EditCommand::SetClip {
            id: clip1,
            start_tick: c1.start_tick,
            gain: 0.5,
            offset_frames: c1.offset_frames,
            length_ticks: c1.length_ticks,
            fades: c1.fades,
            pitch_cents: c1.pitch_cents,
            reverse: c1.reverse,
        })
        .unwrap();
    assert_eq!(session.project(), ref_s.project());

    session
        .edit(EditCommand::SetClipSounds(vec![
            ClipSound {
                id: clip1,
                gain: 0.7,
                fades: c1.fades,
                pitch_cents: 0,
                reverse: false,
            },
            ClipSound {
                id: clip2,
                gain: 0.7,
                fades: c1.fades,
                pitch_cents: 0,
                reverse: false,
            },
        ]))
        .unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    let c_sounds = vec![
        ClipSound {
            id: clip1,
            gain: 0.6,
            fades: c1.fades,
            pitch_cents: 0,
            reverse: false,
        },
        ClipSound {
            id: clip2,
            gain: 0.6,
            fades: c1.fades,
            pitch_cents: 0,
            reverse: false,
        },
    ];
    session
        .edit(EditCommand::SetClipSounds(c_sounds.clone()))
        .unwrap();
    ref_s
        .edit_discrete(EditCommand::SetClipSounds(c_sounds))
        .unwrap();
    assert_eq!(session.project(), ref_s.project());

    session
        .begin_automation_gesture(0, crate::automation::Lane::Volume)
        .unwrap();
    session
        .edit(EditCommand::SetAutomation {
            track: 0,
            lane: crate::automation::Lane::Volume,
            points: vec![crate::automation::Point {
                tick: 0,
                value: 0.5,
            }],
        })
        .unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    session
        .edit(EditCommand::SetAutomation {
            track: 0,
            lane: crate::automation::Lane::Volume,
            points: vec![crate::automation::Point {
                tick: 0,
                value: 0.8,
            }],
        })
        .unwrap();
    ref_s
        .edit_discrete(EditCommand::SetAutomation {
            track: 0,
            lane: crate::automation::Lane::Volume,
            points: vec![crate::automation::Point {
                tick: 0,
                value: 0.8,
            }],
        })
        .unwrap();
    assert_eq!(session.project(), ref_s.project());
    session.end_automation_gesture();

    session.begin_return_gesture(0).unwrap();
    session
        .edit(EditCommand::SetReturnAutomation {
            bus: 0,
            points: vec![crate::automation::Point {
                tick: 0,
                value: 0.4,
            }],
        })
        .unwrap();
    ref_s = Session::headless(session.library.clone(), session.project().clone()).unwrap();
    session
        .edit(EditCommand::SetReturnAutomation {
            bus: 0,
            points: vec![crate::automation::Point {
                tick: 0,
                value: 0.9,
            }],
        })
        .unwrap();
    ref_s
        .edit_discrete(EditCommand::SetReturnAutomation {
            bus: 0,
            points: vec![crate::automation::Point {
                tick: 0,
                value: 0.9,
            }],
        })
        .unwrap();
    assert_eq!(session.project(), ref_s.project());
}
