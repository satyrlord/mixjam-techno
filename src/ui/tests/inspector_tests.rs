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
    focus_control(&ctx, &mut app, &output, "Inspector clip gain");
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
        "Inspector clip start (bar)",
        "Inspector clip gain",
        "Inspector clip trim start (frames)",
        "Inspector clip length (bars)",
        "Inspector clip fade in (beats)",
        "Inspector clip fade out curve",
        "Inspector clip pitch (semitones)",
        "Inspector clip pitch (cents)",
        "Inspector clip reverse",
    ] {
        assert!(has_control(&output, name), "{name}");
    }

    // Several clips: a summary.
    app.select_clip(ids[1], true);
    assert_eq!(app.inspector_page(), inspector::Page::Clips(2));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "2 CLIPS");
    assert!(!has_control(&output, "Inspector clip gain"));

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
