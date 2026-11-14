//! Undo steps of control gestures, automation drags, and clip controls.
use super::*;

#[test]
fn saving_ends_a_control_gesture_before_the_next_edit() {
    let dir = fixture_dir();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let set_master = |session: &mut Session, master_gain| {
        session.edit(EditCommand::SetMixer {
            tracks: session.project().tracks.clone(),
            master_gain,
        })
    };
    set_master(&mut session, 0.5).unwrap();
    session.save(&dir.join("gesture.mixjam")).unwrap();
    let saved = session.project().clone();
    set_master(&mut session, 0.75).unwrap();
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &saved);
    assert!(session.history_step(false).unwrap());
    assert_eq!(
        session.project(),
        &Project {
            title: "gesture".into(),
            ..Project::default()
        }
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn automation_drags_include_additions_and_pauses_but_end_on_release() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let lane = crate::automation::Lane::Volume;
    let set = |session: &mut Session, value| {
        session
            .edit(EditCommand::SetAutomation {
                track: 0,
                lane,
                points: vec![crate::automation::Point { tick: 0, value }],
            })
            .unwrap();
    };
    session.begin_automation_gesture(0, lane).unwrap();
    set(&mut session, -12.0);
    // A held pointer can pause for longer than the ordinary gesture timeout.
    session.gesture = session
        .gesture
        .map(|(gesture, at)| (gesture, at - GESTURE_GAP));
    set(&mut session, -9.0);
    session.end_automation_gesture();
    // The next drag starts immediately, but remains a separate step.
    session.begin_automation_gesture(0, lane).unwrap();
    set(&mut session, -6.0);
    set(&mut session, -3.0);
    session.end_automation_gesture();
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[0].automation.volume[0].value, -9.0);
    assert!(session.history_step(false).unwrap());
    assert!(session.project().tracks[0].automation.volume.is_empty());
    assert!(!session.has_history_step(false));
    assert!(session.history_step(true).unwrap());
    assert_eq!(session.project().tracks[0].automation.volume[0].value, -9.0);
    assert!(session.begin_automation_gesture(TRACK_COUNT, lane).is_err());
    assert!(
        session
            .begin_automation_gesture(0, crate::automation::Lane::Send(4))
            .is_err()
    );
}

#[test]
fn separate_automation_edits_are_separate_undo_steps() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let set = |session: &mut Session, value| {
        session
            .edit(EditCommand::SetAutomation {
                track: 0,
                lane: crate::automation::Lane::Volume,
                points: vec![crate::automation::Point { tick: 0, value }],
            })
            .unwrap();
    };
    set(&mut session, -12.0);
    set(&mut session, -9.0);
    set(&mut session, -6.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[0].automation.volume[0].value, -9.0);
}

#[test]
fn fx_switches_and_different_knobs_end_continuous_history_steps() {
    use crate::fx::{Echoform, FxBuses, Module};
    let mut project = Project::default();
    project.fx.buses[0].module = Module::EchoformDelay(Echoform::default());
    let mut session = Session::headless(Arc::new(Library::empty()), project).unwrap();
    session.gesture_gap = std::time::Duration::MAX;
    let mut states = vec![session.project().fx];
    let mut change = |session: &mut Session, edit: &dyn Fn(&mut FxBuses)| {
        let mut buses = session.project().fx;
        edit(&mut buses);
        session.edit(EditCommand::SetEffects(buses)).unwrap();
        states.push(buses);
    };
    change(&mut session, &|buses| {
        if let Module::EchoformDelay(delay) = &mut buses.buses[0].module {
            delay.feedback = 50.0;
        }
    });
    change(&mut session, &|buses| {
        if let Module::EchoformDelay(delay) = &mut buses.buses[0].module {
            delay.width = 100.0;
        }
    });
    change(&mut session, &|buses| buses.buses[0].powered = false);
    change(&mut session, &|buses| {
        if let Module::EchoformDelay(delay) = &mut buses.buses[0].module {
            delay.width = 90.0;
        }
    });
    change(&mut session, &|buses| buses.buses[0].limiter = false);
    change(&mut session, &|buses| {
        if let Module::EchoformDelay(delay) = &mut buses.buses[0].module {
            delay.bypass = true;
            delay.width = 80.0;
        }
    });
    change(&mut session, &|buses| {
        if let Module::EchoformDelay(delay) = &mut buses.buses[0].module {
            delay.width = 70.0;
        }
    });
    states.pop();
    for expected in states.into_iter().rev() {
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().fx, expected);
    }
    assert!(!session.has_history_step(false));

    // Track and bus identity separate send drags.
    let mut states = vec![session.project().tracks.clone()];
    for (track, bus) in [(0, 0), (0, 1), (1, 1)] {
        for value in [0.25, 0.5] {
            let mut tracks = session.project().tracks.clone();
            tracks[track].sends[bus] = value;
            session
                .edit(EditCommand::SetMixer {
                    tracks,
                    master_gain: session.project().master_gain,
                })
                .unwrap();
        }
        states.push(session.project().tracks.clone());
    }
    states.pop();
    for expected in states.into_iter().rev() {
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().tracks, expected);
    }
    assert!(!session.has_history_step(false));
}

#[test]
fn long_send_and_fx_drags_preserve_prior_history() {
    use crate::fx::{Aetherform, Echoform, Module};
    let mut project = Project::default();
    project.fx.buses[0].module = Module::EchoformDelay(Echoform::default());
    project.fx.buses[1].module = Module::AetherformReverb(Aetherform::default());
    let initial = project.clone();
    let mut session = Session::headless(Arc::new(Library::empty()), project).unwrap();
    session.gesture_gap = std::time::Duration::MAX;
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    for frame in 1..=150 {
        let mut tracks = session.project().tracks.clone();
        tracks[0].sends[0] = frame as f32 / 150.0;
        session
            .edit(EditCommand::SetMixer {
                tracks,
                master_gain: session.project().master_gain,
            })
            .unwrap();
    }
    for frame in 1..=150 {
        let mut buses = session.project().fx;
        if let Module::EchoformDelay(delay) = &mut buses.buses[0].module {
            delay.feedback = frame as f32 / 2.0;
        }
        session.edit(EditCommand::SetEffects(buses)).unwrap();
    }
    for frame in 1..=150 {
        let mut buses = session.project().fx;
        buses.buses[0].return_level = 1.0 - frame as f32 / 150.0;
        session.edit(EditCommand::SetEffects(buses)).unwrap();
    }
    for frame in 1..=150 {
        let mut buses = session.project().fx;
        if let Module::AetherformReverb(reverb) = &mut buses.buses[1].module {
            reverb.decay_seconds = 1.0 + frame as f32 / 100.0;
        }
        session.edit(EditCommand::SetEffects(buses)).unwrap();
    }
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().fx.buses[1], initial.fx.buses[1]);
    assert_eq!(session.project().fx.buses[0].return_level, 0.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().fx.buses[0].return_level, 1.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().fx, initial.fx);
    assert_eq!(session.project().tracks[0].sends[0], 1.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks, initial.tracks);
    assert_eq!(session.project().bpm, 160.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &initial);
    assert!(!session.has_history_step(false));
}

#[test]
fn live_level_history_keeps_the_prepared_mix_and_updates_playback() {
    let changes: [fn(&mut Project); 4] = [
        |project| project.tracks[0].gain = 0.25,
        |project| project.tracks[0].pan = -0.5,
        |project| project.tracks[0].polarity_inverted = true,
        |project| project.master_gain = 0.5,
    ];
    for change in changes {
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        let before = session.project().clone();
        let mix = Arc::new(audio::prepare_project(&before, &session.library).unwrap());
        session.render = Some(Arc::clone(&mix));
        session.render_dirty = false;
        let revision = session.revision;
        let mut after = before.clone();
        change(&mut after);
        session
            .edit_discrete(EditCommand::SetMixer {
                tracks: after.tracks.clone(),
                master_gain: after.master_gain,
            })
            .unwrap();
        for (redo, expected) in [(false, &before), (true, &after)] {
            assert!(session.history_step(redo).unwrap());
            assert_eq!(session.project(), expected);
            assert_eq!(session.revision, revision);
            assert!(!session.render_dirty);
            assert!(Arc::ptr_eq(session.render.as_ref().unwrap(), &mix));
            assert_eq!(
                session
                    .master
                    .levels(&std::array::from_fn(|track| {
                        !expected.tracks[track].automation.pan.is_empty()
                    }))
                    .map(|(levels, _)| levels[0]),
                Some(audio::track_levels(expected)[0])
            );
        }
    }
}

#[test]
fn compressor_histories_follow_explicit_transport_and_switch_lifetimes() {
    let mut project = Project::default();
    project.tracks[0].compressor.on = true;
    let mut session = Session::headless(Arc::new(Library::empty()), project).unwrap();
    session.master.set_levels(session.project());
    let seed = |session: &Session| {
        session.master.keep_compressor(
            0,
            Default::default(),
            10_000,
            session.master.compressor_epoch(),
        );
        assert!(session.master.kept_compressor(0).is_some());
    };
    seed(&session);
    session.pause();
    assert!(session.master.kept_compressor(0).is_some());
    session.seek(1);
    assert!(session.master.kept_compressor(0).is_none());
    seed(&session);
    session.stop();
    assert!(session.master.kept_compressor(0).is_none());
    seed(&session);
    session.set_library(Library::empty());
    assert!(session.master.kept_compressor(0).is_none());
    seed(&session);
    let mut tracks = session.project().tracks.clone();
    tracks[0].compressor.on = false;
    session
        .edit_discrete(EditCommand::SetMixer {
            tracks,
            master_gain: 1.0,
        })
        .unwrap();
    assert!(session.master.kept_compressor(0).is_none());
    session.history_step(false).unwrap();
    seed(&session);
    session.history_step(true).unwrap();
    assert!(session.master.kept_compressor(0).is_none());
    session.history_step(false).unwrap();
    seed(&session);
    session.new_mix().unwrap();
    assert!(session.master.kept_compressor(0).is_none());
}

#[test]
fn master_pedal_edits_keep_the_mix_and_form_undo_steps() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let revision = session.revision;
    session.render_dirty = false;
    let set = |session: &mut Session, change: &dyn Fn(&mut MasterChain)| {
        let mut chain = session.project().master;
        change(&mut chain);
        session.edit(EditCommand::SetMaster(chain))
    };
    set(&mut session, &|chain| chain.booster_compressor.on = true).unwrap();
    // A drag of one knob is one step; another knob or a switch starts a new one.
    for drive in [510, 520, 530] {
        set(&mut session, &|chain| {
            chain.booster_compressor.drive = drive
        })
        .unwrap();
    }
    set(&mut session, &|chain| chain.booster_eq.bands[2] = 70).unwrap();
    set(&mut session, &|chain| chain.booster_eq.bands[2] = 80).unwrap();
    // The mastering pedals and the trim form gestures the same way.
    for attack in [5.0, 2.0] {
        set(&mut session, &|chain| chain.bus_compressor.attack = attack).unwrap();
    }
    set(&mut session, &|chain| chain.trim = -3.5).unwrap();
    set(&mut session, &|chain| chain.trim = -4.0).unwrap();
    // Moving a pedal is a step of its own, even within a knob gesture.
    set(&mut session, &|chain| chain.move_pedal(0, 2)).unwrap();
    assert_eq!(session.project().master.order[2], PedalKind::BoosterEq);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().master.order, PedalKind::ALL);
    assert_eq!(session.project().master.trim, -4.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().master.trim, 0.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().master.bus_compressor.attack, 10.0);
    assert_eq!(session.project().master.booster_eq.bands[2], 80);
    assert!(session.unsaved());
    assert_eq!(session.master.get(), session.project().master);
    // The prepared mix does not include the pedalboard, so it stays valid.
    assert_eq!(session.revision, revision);
    assert!(!session.render_dirty);

    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().master.booster_eq.bands[2], 50);
    assert_eq!(session.master.get().booster_eq.bands[2], 50);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().master.booster_compressor.drive, 500);
    assert!(session.history_step(false).unwrap());
    assert!(!session.project().master.is_active());
    assert!(!session.render_dirty);

    let mut invalid = session.project().master;
    invalid.booster_wide.spread = KNOB_MAX + 1;
    assert!(session.edit(EditCommand::SetMaster(invalid)).is_err());
    assert!(session.history_step(true).unwrap());
    assert!(session.project().master.booster_compressor.on);
}

#[test]
fn mute_and_solo_are_undo_steps_that_end_a_gain_gesture() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let mut tracks = session.project().tracks.clone();
    tracks[2].gain = 0.5;
    session
        .edit(EditCommand::SetMixer {
            tracks: tracks.clone(),
            master_gain: 1.0,
        })
        .unwrap();
    // A mixer edit that only flips Mute must not join the gain drag.
    tracks[2].muted = true;
    session
        .edit(EditCommand::SetMixer {
            tracks,
            master_gain: 1.0,
        })
        .unwrap();
    session
        .edit(EditCommand::SetSwitch {
            track: 2,
            switch: TrackSwitch::Solo,
            on: true,
        })
        .unwrap();
    assert!(session.unsaved());
    for (muted, solo, gain) in [(true, false, 0.5), (false, false, 0.5), (false, false, 1.0)] {
        assert!(session.history_step(false).unwrap());
        let track = &session.project().tracks[2];
        assert_eq!((track.muted, track.solo, track.gain), (muted, solo, gain));
    }
    assert!(!session.history_step(false).unwrap());
}

#[test]
fn polarity_at_silence_is_an_independent_undo_step_between_gain_gestures() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let change = |session: &mut Session, gain, polarity_inverted| {
        let mut tracks = session.project().tracks.clone();
        tracks[3].gain = gain;
        tracks[3].polarity_inverted = polarity_inverted;
        session
            .edit(EditCommand::SetMixer {
                tracks,
                master_gain: session.project().master_gain,
            })
            .unwrap();
    };
    change(&mut session, 0.5, false);
    change(&mut session, 0.0, false);
    change(&mut session, 0.0, true);
    change(&mut session, 1.0, true);
    for (gain, polarity) in [(0.0, true), (0.0, false), (1.0, false)] {
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().tracks[3].gain, gain);
        assert_eq!(session.project().tracks[3].polarity_inverted, polarity);
        assert_eq!(session.project().tracks[3].pan, 0.0);
        assert!(session.project().tracks[3].left_enabled);
        assert!(session.project().tracks[3].right_enabled);
    }
    assert!(!session.history_step(false).unwrap());
    for (gain, polarity) in [(0.0, false), (0.0, true), (1.0, true)] {
        assert!(session.history_step(true).unwrap());
        assert_eq!(session.project().tracks[3].gain, gain);
        assert_eq!(session.project().tracks[3].polarity_inverted, polarity);
    }
}

#[test]
fn one_control_drag_is_one_undo_step_and_keeps_earlier_history() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    let mixer = |session: &Session, track: usize, gain: f32, pan: f32| {
        let mut tracks = session.project().tracks.clone();
        tracks[track].gain = gain;
        tracks[track].pan = pan;
        EditCommand::SetMixer {
            tracks,
            master_gain: session.project().master_gain,
        }
    };
    // More frames than the history holds: the tempo edit must survive them.
    for frame in 0..150 {
        let command = mixer(&session, 3, 1.0 + (frame + 1) as f32 * 0.01, 0.0);
        session.edit(command).unwrap();
    }
    // Another control, and the same control after a pause, start new steps.
    let command = mixer(&session, 3, 2.5, 0.5);
    session.edit(command).unwrap();
    session.gesture = session
        .gesture
        .map(|(gesture, at)| (gesture, at - GESTURE_GAP));
    let command = mixer(&session, 3, 2.5, -0.5);
    session.edit(command).unwrap();
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[3].pan, 0.5);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[3].pan, 0.0);
    assert_eq!(session.project().tracks[3].gain, 2.5);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[3].gain, 1.0);
    assert_eq!(session.project().bpm, 160.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &Project::default());
    assert!(!session.history_step(false).unwrap());
}
#[test]
fn a_held_pointer_keeps_one_step_and_a_new_press_starts_another() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let gain = |session: &mut Session, gain: f32| {
        let mut tracks = session.project().tracks.clone();
        tracks[1].gain = gain;
        session
            .edit(EditCommand::SetMixer {
                tracks,
                master_gain: session.project().master_gain,
            })
            .unwrap();
    };
    let pause = |session: &mut Session| {
        session.gesture = session
            .gesture
            .map(|(gesture, at)| (gesture, at - GESTURE_GAP));
    };
    // A drag held still past the pause is still one step.
    session.pointer_gesture(true, true);
    gain(&mut session, 0.8);
    pause(&mut session);
    session.pointer_gesture(false, true);
    gain(&mut session, 0.6);
    // Unity pressed at once after it is a step of its own.
    session.pointer_gesture(false, false);
    session.pointer_gesture(true, true);
    gain(&mut session, 1.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[1].gain, 0.6);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &Project::default());
    assert!(!session.history_step(false).unwrap());
}
#[test]
fn an_eq_dial_drag_is_one_undo_step_and_its_switch_another() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let eq = |session: &Session, change: &dyn Fn(&mut crate::eq::ChannelEq)| {
        let mut tracks = session.project().tracks.clone();
        change(&mut tracks[2].eq);
        EditCommand::SetMixer {
            tracks,
            master_gain: session.project().master_gain,
        }
    };
    let command = eq(&session, &|eq| eq.on = true);
    session.edit(command).unwrap();
    for step in 1..=20 {
        let command = eq(&session, &|eq| eq.high_db = -0.5 * step as f32);
        session.edit(command).unwrap();
    }
    // Another dial starts its own step.
    let command = eq(&session, &|eq| eq.high_hz = 6000.0);
    session.edit(command).unwrap();
    assert!(session.unsaved());
    assert_eq!(session.project().tracks[2].eq.high_db, -10.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[2].eq.high_hz, 8000.0);
    assert_eq!(session.project().tracks[2].eq.high_db, -10.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[2].eq.high_db, 0.0);
    assert!(session.project().tracks[2].eq.on);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &Project::default());
    // Out-of-range settings are not an edit.
    let command = eq(&session, &|eq| eq.mid_q = 9.0);
    assert!(session.edit(command).is_err());
}
#[test]
fn ducker_dial_drags_keep_one_undo_step_per_control() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let set = |session: &mut Session, change: &dyn Fn(&mut crate::ducker::Ducker)| {
        let mut tracks = session.project().tracks.clone();
        change(&mut tracks[2].ducker);
        session
            .edit(EditCommand::SetMixer {
                tracks,
                master_gain: session.project().master_gain,
            })
            .unwrap();
    };
    set(&mut session, &|ducker| ducker.on = true);
    for depth in [7.0, 8.0, 9.0] {
        set(&mut session, &|ducker| ducker.depth = depth);
    }
    for key in [4, 5, 6] {
        set(&mut session, &|ducker| ducker.key = key);
    }
    for attack in [3.0, 4.0, 5.0] {
        set(&mut session, &|ducker| ducker.attack = attack);
    }
    set(&mut session, &|ducker| ducker.on = false);
    assert!(session.history_step(false).unwrap());
    assert!(session.project().tracks[2].ducker.on);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[2].ducker.attack, 2.0);
    assert_eq!(session.project().tracks[2].ducker.key, 6);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[2].ducker.key, 1);
    assert_eq!(session.project().tracks[2].ducker.depth, 9.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().tracks[2].ducker.depth, 6.0);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &Project::default());
    assert!(!session.history_step(false).unwrap());
}

#[test]
fn bypass_is_one_undo_step_that_keeps_the_mix_prepared() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let mut chain = session.project().master;
    chain.limiter.on = true;
    session.edit(EditCommand::SetMaster(chain)).unwrap();
    chain.bypass = true;
    session.edit(EditCommand::SetMaster(chain)).unwrap();
    assert!(session.project().master.bypass && session.project().master.limiter.on);
    assert!(session.master.get().bypass);
    assert!(session.history_step(false).unwrap());
    assert!(!session.project().master.bypass && session.project().master.limiter.on);
    // A preset keeps the bypass as it keeps the trim.
    let preset = crate::master::presets::PRESETS[0].applied_to(&chain);
    assert!(preset.bypass);
}

#[test]
fn a_longer_clip_cannot_cover_the_next_clip_on_its_track() {
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
    let first = session.place(source, 0, 0).unwrap();
    let length = session.project().clips[0].length_ticks;
    session.place(source, 0, length + 16).unwrap();
    let before = session.project().clone();
    let resize = |length_ticks| EditCommand::SetClip {
        id: first,
        start_tick: 0,
        gain: 1.0,
        offset_frames: 0,
        length_ticks,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    };
    assert!(session.edit(resize(length + 17)).is_err());
    assert_eq!(session.project(), &before);
    session.edit(resize(length + 16)).unwrap();
    assert_eq!(session.project().clips[0].length_ticks, length + 16);
    // The left edge of the second clip cannot reach back into the first.
    let second = session.project().clips[1].clone();
    let start = |start_tick| EditCommand::SetClip {
        id: second.id,
        start_tick,
        gain: 1.0,
        offset_frames: 0,
        length_ticks: second.start_tick + second.length_ticks - start_tick,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    };
    let before = session.project().clone();
    assert!(session.edit(start(second.start_tick - 1)).is_err());
    assert_eq!(session.project(), &before);
    session.edit(start(second.start_tick + 16)).unwrap();
    assert_eq!(
        session.project().clips[1].start_tick,
        second.start_tick + 16
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn a_shorter_clip_shortens_its_fades_and_undo_restores_them() {
    let dir = fixture_dir();
    let wave = dir.join("source.wav");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 4000],
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let source = session.register_file(&wave, "Wave", 140.0).unwrap();
    let clip = session.place(source, 0, 0).unwrap();
    let length = session.project().clips[0].length_ticks;
    let fades = Fades {
        fade_in: Fade {
            ticks: 64,
            curve: FadeCurve::EqualPower,
        },
        fade_out: Fade {
            ticks: 128,
            curve: FadeCurve::Exponential,
        },
    };
    let set = |length_ticks, pitch_cents| EditCommand::SetClip {
        id: clip,
        start_tick: 0,
        gain: 1.0,
        offset_frames: 0,
        length_ticks,
        fades,
        pitch_cents,
        reverse: false,
    };
    session.edit(set(length, -30)).unwrap();
    assert_eq!(session.project().clips[0].fades, fades);
    assert_eq!(session.project().clips[0].pitch_cents, -30);
    session.edit(set(100, -30)).unwrap();
    let fitted = session.project().clips[0].fades;
    assert_eq!((fitted.fade_in.ticks, fitted.fade_out.ticks), (64, 36));
    assert!(session.edit(set(length, 2401)).is_err());
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().clips[0].fades, fades);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn each_clip_control_drag_is_its_own_undo_step() {
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
    let length = session.project().clips[0].length_ticks;
    let set = |gain, offset_frames, length_ticks| EditCommand::SetClip {
        id: clip,
        start_tick: 0,
        gain,
        offset_frames,
        length_ticks,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    };
    // Frames of a gain drag, then at once frames of a trim drag: two
    // controls, so two steps.
    session.edit(set(0.5, 0, length)).unwrap();
    session.edit(set(0.6, 0, length)).unwrap();
    session.edit(set(0.6, 10, length)).unwrap();
    session.edit(set(0.6, 20, length)).unwrap();
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().clips[0].offset_frames, 0);
    assert_eq!(session.project().clips[0].gain, 0.6);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project().clips[0].gain, 1.0);
    std::fs::remove_dir_all(dir).unwrap();
}
