//! Inspector panel: song, track and clip pages, drafts and undo steps.

use super::*;

#[test]
fn inspector_keeps_a_single_clip_after_shift_clicking_empty_score() {
    let (ctx, mut app) = fixture();
    app.open_inspector();
    let id = app.session.project().clips[0].id;
    app.select_clip(id, false);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let clip = app.session.project().clips[0].clone();
    let canvas = app.score.rect.unwrap();
    let empty = Pos2::new(
        canvas.left() + 3.5 * app.score.bar_width,
        app.clip_rect(&clip, canvas).center().y,
    );
    assert!(app.score.drop_rect.unwrap().contains(empty));
    click(&ctx, &mut app, empty, egui::Modifiers::SHIFT);
    assert_eq!(app.selection.clips, [id].into_iter().collect());
    assert_eq!(app.inspector_page(), inspector::Page::Clip(id));
}

#[test]
fn a_deleted_clip_leaves_the_inspector_on_the_song_not_an_older_track() {
    let (_ctx, mut app) = fixture();
    app.open_inspector();
    app.select_track(0);
    let id = app.session.project().clips[0].id;
    app.select_clip(id, false);
    assert_eq!(app.inspector_page(), inspector::Page::Clip(id));
    app.delete_selection();
    assert_eq!(app.inspector_page(), inspector::Page::Song);
}

#[test]
fn inspector_track_gain_draft_cannot_change_another_track() {
    for replace_song in [false, true] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.open_inspector();
        app.select_track(0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Inspector track gain value");
        frame(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("+6".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        if replace_song {
            app.new_mix();
            app.session.place(1, 0, 0).unwrap();
            app.select_track(0);
        } else {
            app.select_track(1);
        }
        let before = app.session.project().clone();
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project() == &before);
    }
}

#[test]
fn inspector_clip_draft_cannot_change_another_clip() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let first = app.session.project().clips[0].id;
    let second = app.session.project().clips[1].id;
    app.select_clip(first, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Inspector clip gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    let before = app.session.project().clone();
    app.select_clip(second, false);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("6".into())],
        egui::Modifiers::NONE,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project() == &before);
}

#[test]
fn the_inspector_shows_the_song_a_track_one_clip_or_several_clips() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let result = app.session.edit(EditCommand::SetSections(vec![
        Section {
            start_tick: 0,
            name: "Intro".into(),
        },
        Section {
            start_tick: 8 * TICKS_PER_BAR,
            name: "Drop".into(),
        },
    ]));
    app.apply(result);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(has_control(&output, "Show the Inspector"));
    assert!(!has_control(&output, "Fold the Inspector"));
    click_named(&ctx, &mut app, "Show the Inspector");
    assert!(app.session.layout().inspector && app.panels.room.inspector);

    // Nothing selected: the song, its categories and sections.
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.inspector_page(), inspector::Page::Song);
    text_rect(&output, "CATEGORIES");
    text_rect(&output, "SECTIONS");
    assert!(has_control(&output, "Go to Intro, bar 1"));
    click_named(&ctx, &mut app, "Go to Drop, bar 9");

    // A track: its level, pan, outputs, sends, and processing.
    click_named(&ctx, &mut app, "Show track 1 in the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Track(0));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in [
        "Inspector track gain",
        "Inspector track gain value",
        "Reset Inspector track gain to unity",
        "Inspector track pan",
        "Center Inspector track pan",
        "Inspector track mute",
        "Inspector track solo",
        "Inspector track left output",
        "Inspector track right output",
        "Inspector track polarity invert",
        "Inspector track send FX 1",
        "Inspector track send FX 4",
        "Inspector track EQ response curve",
        "Inspector track EQ power",
        "Inspector track drive power",
        "Inspector track compressor power",
        "Inspector track ducker power",
        "Inspector track EQ",
    ] {
        assert!(has_control(&output, name), "{name}");
    }
    click_named(&ctx, &mut app, "Inspector track EQ");
    assert_eq!(app.eq_track, Some(0));
    app.eq_track = None;

    // One clip: its properties, under their own names.
    let ids: Vec<u64> = app
        .session
        .project()
        .clips
        .iter()
        .map(|clip| clip.id)
        .take(2)
        .collect();
    assert_eq!(ids.len(), 2, "The fixture has two clips.");
    app.select_clip(ids[0], false);
    assert_eq!(app.inspector_page(), inspector::Page::Clip(ids[0]));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in [
        "Inspector clip fade in handle",
        "Inspector clip fade out handle",
        "Inspector clip fade in (beats)",
        "Inspector clip fade in curve",
        "Inspector clip fade out (beats)",
        "Inspector clip fade out curve",
        "Inspector clip gain",
        "Inspector clip gain value",
        "Inspector clip pitch (semitones)",
        "Inspector clip pitch (cents)",
        "Inspector clip reverse",
        "Inspector clip start (bar)",
        "Inspector clip length (bars)",
        "Inspector clip trim start (beats)",
    ] {
        assert!(has_control(&output, name), "{name}");
    }

    // Several clips: their shared fades and sound, and a summary. Their
    // places stay their own.
    app.select_clip(ids[1], true);
    assert_eq!(app.inspector_page(), inspector::Page::Clips(2));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "2 CLIPS");
    for name in [
        "Inspector clip fade in (beats)",
        "Inspector clip gain value",
        "Inspector clip pitch (semitones)",
        "Inspector clip reverse",
    ] {
        assert!(has_control(&output, name), "{name}");
    }
    assert!(!has_control(&output, "Inspector clip start (bar)"));
    assert!(!has_control(&output, "Inspector clip fade in handle"));

    // An empty click in the score clears the selection: the song again.
    app.selection.clips.clear();
    app.selection.clip = None;
    app.select_track(2);
    app.clear_selection();
    assert_eq!(app.inspector_page(), inspector::Page::Song);

    // It folds to its strip.
    click_named(&ctx, &mut app, "Fold the Inspector");
    assert!(!app.session.layout().inspector && !app.panels.room.inspector);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(has_control(&output, "Show the Inspector"));
}

#[test]
fn a_shared_pitch_edit_preserves_the_other_component() {
    for (field, value, expected) in [("semitones", "3", [325, 375]), ("cents", "50", [150, 250])] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.open_inspector();
        let clips = app.session.project().clips.clone();
        for (clip, pitch_cents) in clips.iter().zip([125, 275]) {
            app.session
                .edit(EditCommand::SetClip {
                    id: clip.id,
                    start_tick: clip.start_tick,
                    gain: clip.gain,
                    offset_frames: clip.offset_frames,
                    length_ticks: clip.length_ticks,
                    fades: clip.fades,
                    pitch_cents,
                    reverse: clip.reverse,
                })
                .unwrap();
        }
        app.select_clip(clips[0].id, false);
        app.select_clip(clips[1].id, true);
        let before = app.session.project().clone();
        let name = format!("Inspector clip pitch ({field})");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, &name);
        type_inspector_value(&ctx, &mut app, value);
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        for (clip, pitch) in app.session.project().clips.iter().zip(expected) {
            assert_eq!(clip.pitch_cents, pitch, "{name}, clip {}", clip.id);
        }
        app.history_step(false);
        assert!(app.session.project() == &before);
    }
}

#[test]
fn a_shared_clip_draft_cannot_change_a_different_selection_of_the_same_size() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let first = app.session.project().clips[0].id;
    let second = app.session.project().clips[1].id;
    let third = app.session.place(1, 2, 0).unwrap();
    let before = app.session.project().clone();
    app.select_clip(first, false);
    app.select_clip(second, true);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Inspector clip gain value");
    type_inspector_value(&ctx, &mut app, "-");
    app.select_clip(second, true);
    app.select_clip(third, true);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("6".into())],
        egui::Modifiers::NONE,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project() == &before);
}

#[test]
fn merely_showing_a_short_clip_in_the_inspector_keeps_its_length() {
    let (ctx, mut app) = fixture();
    app.open_inspector();
    let clip = app.session.project().clips[0].clone();
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: clip.gain,
            offset_frames: clip.offset_frames,
            length_ticks: 1,
            fades: clip.fades,
            pitch_cents: clip.pitch_cents,
            reverse: clip.reverse,
        })
        .unwrap();
    let before = app.session.project().clone();
    app.select_clip(clip.id, false);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.session.project() == &before);
}

#[test]
fn leaving_an_unedited_inspector_field_does_not_round_the_clip_value() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let clip = app.session.project().clips[0].clone();
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: 1.003,
            offset_frames: 1,
            length_ticks: clip.length_ticks,
            fades: clip.fades,
            pitch_cents: clip.pitch_cents,
            reverse: clip.reverse,
        })
        .unwrap();
    let before = app.session.project().clone();
    app.select_clip(clip.id, false);
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in [
        "Inspector clip gain value",
        "Inspector clip trim start (beats)",
    ] {
        focus_control(&ctx, &mut app, &output, name);
        output = frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project() == &before, "{name}");
    }
    focus_control(&ctx, &mut app, &output, "Inspector clip trim start (beats)");
    type_inspector_value(&ctx, &mut app, "0.000");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().clips[0].offset_frames, 0);
    app.history_step(false);
    assert!(app.session.project() == &before);
}

#[test]
fn exact_inspector_values_make_separate_undo_steps() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.session.hold_gestures();
    app.open_inspector();
    let clip = app.session.project().clips[0].clone();
    app.select_clip(clip.id, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, _) = accessible_node(&output, "Inspector clip pitch (semitones)");
    for value in [3.0, 4.0] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target,
                    data: Some(egui::accesskit::ActionData::NumericValue(value)),
                },
            )],
            egui::Modifiers::NONE,
        );
    }
    assert_eq!(app.session.project().clips[0].pitch_cents, 400);
    app.history_step(false);
    assert_eq!(app.session.project().clips[0].pitch_cents, 300);
    app.history_step(false);
    assert_eq!(app.session.project().clips[0], clip);
}

#[test]
fn a_mixed_gain_field_can_commit_the_first_clips_gain_or_cancel_an_edit() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let clip = app.session.project().clips[1].clone();
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: 0.5,
            offset_frames: clip.offset_frames,
            length_ticks: clip.length_ticks,
            fades: clip.fades,
            pitch_cents: clip.pitch_cents,
            reverse: clip.reverse,
        })
        .unwrap();
    let before = app.session.project().clone();
    app.select_clip(before.clips[0].id, false);
    app.select_clip(clip.id, true);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Inspector clip gain value");
    type_inspector_value(&ctx, &mut app, "-6");
    let output = frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session.project() == &before,
        "Escape must cancel the draft."
    );
    focus_control(&ctx, &mut app, &output, "Inspector clip gain value");
    type_inspector_value(&ctx, &mut app, "0");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .all(|clip| clip.gain == 1.0)
    );
    app.history_step(false);
    assert!(app.session.project() == &before);

    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, _) = accessible_node(&output, "Inspector clip gain value");
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target,
                data: Some(egui::accesskit::ActionData::NumericValue(0.0)),
            },
        )],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .all(|clip| clip.gain == 1.0)
    );
    app.history_step(false);
    assert!(app.session.project() == &before);
}

fn type_inspector_value(ctx: &egui::Context, app: &mut MixJamApp, value: &str) {
    frame(
        ctx,
        app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text(value.into()),
        ],
        egui::Modifiers::COMMAND,
    );
}

#[test]
fn every_inspector_edit_is_one_undo_step() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    app.select_track(0);
    let track = app.session.project().tracks[0].clone();
    for (name, changed) in [
        (
            "Inspector track mute",
            (|track: &Track| track.muted) as fn(&Track) -> bool,
        ),
        ("Inspector track polarity invert", |track: &Track| {
            track.polarity_inverted
        }),
        ("Inspector track drive power", |track: &Track| {
            track.drive.on
        }),
        ("Inspector track ducker power", |track: &Track| {
            track.ducker.on
        }),
    ] {
        click_named(&ctx, &mut app, name);
        assert_ne!(
            changed(&app.session.project().tracks[0]),
            changed(&track),
            "{name}"
        );
        app.history_step(false);
        assert_eq!(app.session.project().tracks[0], track, "{name}");
    }
    let id = app.session.project().clips[0].id;
    let clip = app.session.project().clips[0].clone();
    app.select_clip(id, false);
    click_named(&ctx, &mut app, "Inspector clip reverse");
    assert_ne!(app.session.project().clips[0].reverse, clip.reverse);
    app.history_step(false);
    assert_eq!(app.session.project().clips[0], clip);
}

#[test]
fn a_second_click_on_the_track_name_shows_the_song_again() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    click_named(&ctx, &mut app, "Show track 1 in the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Track(0));
    click_named(&ctx, &mut app, "Show the song in the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Song);
    click_named(&ctx, &mut app, "Show track 2 in the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Track(1));
}

#[test]
fn a_track_name_reopens_the_folded_inspector_for_the_selected_track() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    click_named(&ctx, &mut app, "Show track 1 in the Inspector");
    click_named(&ctx, &mut app, "Fold the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Track(0));
    assert!(!app.panels.room.inspector);

    click_named(&ctx, &mut app, "Show track 1 in the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Track(0));
    assert!(app.panels.room.inspector);
}

#[test]
fn a_track_name_opens_the_track_page_after_placing_a_clip() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    click_named(&ctx, &mut app, "Show track 1 in the Inspector");
    let id = app.try_place(1, 0, 2 * TICKS_PER_BAR).unwrap();
    assert_eq!(app.inspector_page(), inspector::Page::Clip(id));

    click_named(&ctx, &mut app, "Show track 1 in the Inspector");
    assert_eq!(app.inspector_page(), inspector::Page::Track(0));
}

#[test]
fn a_shared_clip_edit_sets_every_selected_clip_in_one_undo_step() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let clip = app.session.project().clips[1].clone();
    // Different gains, so the field starts as a dash.
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: 0.5,
            offset_frames: clip.offset_frames,
            length_ticks: clip.length_ticks,
            fades: clip.fades,
            pitch_cents: clip.pitch_cents,
            reverse: clip.reverse,
        })
        .unwrap();
    let before = app.session.project().clone();
    let ids = [before.clips[0].id, clip.id];
    app.select_clip(ids[0], false);
    app.select_clip(ids[1], true);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Inspector clip gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-6".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let want = mixjam_techno::application::db_to_amplitude(-6.0);
    for id in ids {
        let gain = app
            .session
            .project()
            .clips
            .iter()
            .find(|clip| clip.id == id)
            .unwrap()
            .gain;
        assert!((gain - want).abs() < 1e-4, "{gain}");
    }
    app.history_step(false);
    assert!(app.session.project() == &before);

    // A switch sets them all too, as its own step.
    click_named(&ctx, &mut app, "Inspector clip reverse");
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .filter(|clip| ids.contains(&clip.id))
            .all(|clip| clip.reverse)
    );
    app.history_step(false);
    assert!(app.session.project() == &before);
}

#[test]
fn a_clip_with_a_missing_sound_offers_to_locate_it() {
    let (ctx, mut app) = missing_fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let id = app.session.project().clips[0].id;
    app.select_clip(id, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "LOST LEAD");
    assert!(has_control(&output, "Inspector clip locate sound"));
    assert!(!has_control(&output, "Inspector clip gain value"));
}

#[test]
fn inspector_clip_gain_fader_uses_clip_limits_and_can_raise_silence() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    app.select_clip(app.session.project().clips[0].id, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let name = "Inspector clip gain";
    let (_, node) = accessible_node(&output, name);
    assert_eq!(node.min_numeric_value(), Some(-60.0));
    assert!(
        node.max_numeric_value()
            .is_some_and(|db| (6.0..6.1).contains(&db))
    );
    focus_control(&ctx, &mut app, &output, name);

    for (key, expected) in [
        (egui::Key::Home, 0.0),
        (
            egui::Key::ArrowUp,
            mixjam_techno::application::db_to_amplitude(-59.5),
        ),
        (egui::Key::End, 2.0),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!((app.session.project().clips[0].gain - expected).abs() < 1e-7);
    }
}

#[test]
fn inspector_picture_fade_handles_respond_to_keyboard_and_accesskit() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    app.select_clip(app.session.project().clips[0].id, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, node) = accessible_node(&output, "Inspector clip fade in handle");
    assert_eq!(node.numeric_value(), Some(0.0));
    assert_eq!(node.min_numeric_value(), Some(0.0));
    assert_eq!(node.max_numeric_value(), Some(4.0));
    focus_control(&ctx, &mut app, &output, "Inspector clip fade in handle");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::CTRL)],
        egui::Modifiers::CTRL,
    );
    assert_eq!(app.session.project().clips[0].fades.fade_in.ticks, 1);

    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target,
                data: Some(egui::accesskit::ActionData::NumericValue(2.0)),
            },
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.session.project().clips[0].fades.fade_in.ticks,
        TICKS_PER_BAR / 2
    );
}

#[test]
fn inspector_picture_fade_drag_uses_the_release_position() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let clip = app.session.project().clips[0].clone();
    app.select_clip(clip.id, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let handle = screen(&output).node("Inspector clip fade in handle").rect();
    let origin = handle.center();
    pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(origin + Vec2::new(32.0, 0.0))],
        egui::Modifiers::NONE,
    );
    let during = app.session.project().clips[0].fades.fade_in.ticks;
    assert!(during > 0);
    let middle = origin + Vec2::new(32.0, 0.0);
    for pressed in [true, false] {
        button_at(
            &ctx,
            &mut app,
            middle,
            egui::PointerButton::Secondary,
            pressed,
        );
    }
    assert_eq!(app.session.project().clips[0].fades.fade_in.ticks, during);
    pointer(
        &ctx,
        &mut app,
        origin + Vec2::new(128.0, 0.0),
        false,
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().clips[0].fades.fade_in.ticks > during);
    app.history_step(false);
    assert_eq!(app.session.project().clips[0], clip);
}

#[test]
fn a_secondary_drag_does_not_change_an_inspector_fade() {
    for add_primary in [false, true] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.open_inspector();
        let clip = app.session.project().clips[0].clone();
        app.select_clip(clip.id, false);
        let none = egui::Modifiers::NONE;
        let output = frame(&ctx, &mut app, vec![], none);
        let origin = screen(&output)
            .node("Inspector clip fade in handle")
            .rect()
            .center();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(origin)],
            none,
        );
        let secondary = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: none,
        };
        frame(&ctx, &mut app, vec![secondary(origin, true)], none);
        let end = origin + Vec2::new(80.0, 0.0);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(end)], none);
        if add_primary {
            pointer(&ctx, &mut app, end, true, none);
            pointer(&ctx, &mut app, end, false, none);
        }
        frame(&ctx, &mut app, vec![secondary(end, false)], none);
        assert_eq!(app.session.project().clips[0], clip);
    }
}

#[test]
fn inspector_picture_fade_handles_can_both_drag_when_they_meet() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let clip = app.session.project().clips[0].clone();
    let mut fades = clip.fades;
    let half = clip.length_ticks / 2;
    fades.fade_in.ticks = half;
    fades.fade_out.ticks = half;
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: clip.gain,
            offset_frames: clip.offset_frames,
            length_ticks: clip.length_ticks,
            fades,
            pitch_cents: clip.pitch_cents,
            reverse: clip.reverse,
        })
        .unwrap();
    app.select_clip(clip.id, false);

    for (name, dx) in [("in", -48.0), ("out", 48.0)] {
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let in_target = screen(&output).node("Inspector clip fade in handle").rect();
        let out_target = screen(&output)
            .node("Inspector clip fade out handle")
            .rect();
        assert!(in_target.right() <= out_target.left());
        let from = if name == "in" { in_target } else { out_target }.center();
        drag(
            &ctx,
            &mut app,
            from,
            from + Vec2::new(dx, 0.0),
            egui::Modifiers::NONE,
        );
        let result = app.session.project().clips[0].fades;
        if name == "in" {
            assert!(result.fade_in.ticks < half);
            assert_eq!(result.fade_out.ticks, half);
        } else {
            assert!(result.fade_out.ticks < half);
            assert_eq!(result.fade_in.ticks, half);
        }
        app.history_step(false);
        assert_eq!(app.session.project().clips[0].fades, fades);
    }
}

#[test]
fn inspector_track_sends_stay_below_the_channel_outputs() {
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.open_inspector();
        app.select_track(0);
        let size = Vec2::new(1440.0, 1000.0);
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let outputs = effects_control_rect(&output, "Inspector track polarity invert");
        let heading = text_rect(&output, "SENDS");
        let send = effects_control_rect(&output, "Inspector track send FX 1");
        assert!(
            outputs.bottom() < heading.top(),
            "{theme:?}: {outputs:?}, {heading:?}"
        );
        assert!(
            heading.bottom() < send.top(),
            "{theme:?}: {heading:?}, {send:?}"
        );
    });
}

#[test]
fn inspector_fade_duration_and_curve_fit_side_by_side() {
    use mixjam_techno::application::{FadeCurve, MAX_TICKS};

    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.open_inspector();
        let clip = app.session.project().clips[0].clone();
        let mut fades = clip.fades;
        fades.fade_in.ticks = MAX_TICKS - 35;
        fades.fade_in.curve = FadeCurve::Exponential;
        fades.fade_out.ticks = 35;
        fades.fade_out.curve = FadeCurve::EqualPower;
        app.session
            .edit(EditCommand::SetClip {
                id: clip.id,
                start_tick: clip.start_tick,
                gain: clip.gain,
                offset_frames: clip.offset_frames,
                length_ticks: MAX_TICKS,
                fades,
                pitch_cents: clip.pitch_cents,
                reverse: clip.reverse,
            })
            .unwrap();
        app.select_clip(clip.id, false);
        let size = Vec2::new(960.0, 720.0);
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        for (edge, label) in [("in", "Exponential"), ("out", "Equal power")] {
            let duration =
                effects_control_rect(&output, &format!("Inspector clip fade {edge} (beats)"));
            let curve = effects_control_rect(&output, &format!("Inspector clip fade {edge} curve"));
            assert!(
                duration.right() < curve.left(),
                "{theme:?}: {duration:?}, {curve:?}"
            );
            assert!(curve.right() < size.x - 12.0, "{theme:?}: {curve:?}");
            assert!(
                output.shapes.iter().any(|shape| matches!(
                    &shape.shape,
                    egui::Shape::Text(text) if text.galley.text() == label && !text.galley.elided
                )),
                "{theme:?}: {label} must fit in {curve:?}"
            );
        }
    });
}

#[test]
fn mixed_clip_fader_can_set_the_first_clips_existing_gain() {
    use mixjam_techno::application::ClipSound;

    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.open_inspector();
    let first = app.session.project().clips[0].id;
    let clip = app.session.project().clips[1].clone();
    app.session
        .edit(EditCommand::SetClipSounds(vec![ClipSound {
            id: clip.id,
            gain: 0.5,
            fades: clip.fades,
            pitch_cents: clip.pitch_cents,
            reverse: clip.reverse,
        }]))
        .unwrap();
    let before = app.session.project().clone();
    app.select_clip(first, false);
    app.select_clip(clip.id, true);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, node) = accessible_node(&output, "Inspector clip gain");
    assert_eq!(node.numeric_value(), Some(0.0));

    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target,
                data: Some(egui::accesskit::ActionData::NumericValue(0.0)),
            },
        )],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .all(|clip| clip.gain == 1.0)
    );
    app.history_step(false);
    assert!(app.session.project() == &before);
}
