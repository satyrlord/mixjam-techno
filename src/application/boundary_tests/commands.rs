//! Typed commands: validation, atomic rejection, and edits that change nothing.
use super::*;

#[test]
fn constructors_reject_invalid_projects_before_commands_can_index_them() {
    for project in [
        Project {
            tracks: vec![],
            ..Project::default()
        },
        Project {
            bpm: 146.0,
            ..Project::default()
        },
    ] {
        assert!(Session::headless(Arc::new(Library::empty()), project).is_err());
    }
}
#[test]
fn invalid_typed_commands_preserve_project_history_and_dirty_state() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let before = session.project().clone();
    for command in [
        EditCommand::SetTempo(146.0),
        EditCommand::SetLoop(Some([256, 256])),
        EditCommand::SetSwitch {
            track: TRACK_COUNT,
            switch: TrackSwitch::Left,
            on: false,
        },
        EditCommand::SetMixer {
            tracks: vec![],
            master_gain: 1.0,
        },
        EditCommand::SetTrack {
            index: TRACK_COUNT,
            track: Box::default(),
        },
        EditCommand::DeleteClips(vec![5]),
    ] {
        assert!(session.edit(command).is_err());
        assert_eq!(session.project(), &before);
        assert!(!session.unsaved());
        assert!(!session.history_step(false).unwrap());
    }
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    assert_eq!(session.project().bpm, 160.0);
    assert!(session.unsaved());
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &before);
}
#[test]
fn clip_trim_bounds_reject_silent_out_of_source_edits_atomically() {
    let dir = fixture_dir();
    let wave = dir.join("source.wav");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 1000],
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let source = session.register_file(&wave, "Wave", 140.0).unwrap();
    let clip = session.place(source, 0, 0).unwrap();
    session.save(&dir.join("project.mixjam")).unwrap();
    let before = session.project().clone();
    let revision = session.revision;
    let command = |offset| EditCommand::SetClip {
        id: clip,
        start_tick: 0,
        gain: 1.0,
        offset_frames: offset,
        length_ticks: before.clips[0].length_ticks,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    };
    assert!(session.edit(command(1000)).is_err());
    assert_eq!(session.project(), &before);
    assert!(!session.unsaved());
    assert_eq!(session.revision, revision);
    session.edit(command(999)).unwrap();
    assert_eq!(session.project().clips[0].offset_frames, 999);
    session.history_step(false).unwrap();
    assert_eq!(session.project(), &before);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn one_track_edits_join_a_drag_and_a_switch_is_its_own_step() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let before = session.project().clone();
    let with = |session: &Session, change: &dyn Fn(&mut Track)| {
        let mut track = session.project().tracks[3].clone();
        change(&mut track);
        EditCommand::SetTrack {
            index: 3,
            track: Box::new(track),
        }
    };
    for gain in [0.9, 0.8, 0.7] {
        let command = with(&session, &|track| track.gain = gain);
        session.edit(command).unwrap();
    }
    let command = with(&session, &|track| track.muted = true);
    session.edit(command).unwrap();
    assert_eq!(session.project().tracks[3].gain, 0.7);
    assert!(session.project().tracks[3].muted);
    assert_eq!(session.project().tracks[2], before.tracks[2]);
    assert!(session.history_step(false).unwrap());
    assert!(!session.project().tracks[3].muted);
    assert_eq!(session.project().tracks[3].gain, 0.7);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &before);
}
#[test]
fn accepted_commands_that_change_nothing_are_not_edits() {
    let dir = fixture_dir();
    let wave = dir.join("source.wav");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 1000],
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let source = session.register_file(&wave, "Wave", 140.0).unwrap();
    let clip = session.place(source, 0, 0).unwrap();
    session.place(source, 1, 0).unwrap();
    session.save(&dir.join("project.mixjam")).unwrap();
    let revision = session.revision;
    session.move_clips(&[clip], 0, 0, false).unwrap();
    session
        .edit(EditCommand::SetMixer {
            tracks: session.project().tracks.clone(),
            master_gain: session.project().master_gain,
        })
        .unwrap();
    session
        .edit(EditCommand::SetTempo(session.project().bpm))
        .unwrap();
    assert!(!session.unsaved());
    assert_eq!(session.revision, revision);
    std::fs::remove_dir_all(dir).unwrap();
}
