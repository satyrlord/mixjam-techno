//! The Options window in the app: keyboard use, focus, the fixed footer, the
//! discard prompt, and the effects that reach the Session.

use super::*;
use mixjam_techno::application::options::{LaneHeight, SampleQuality, Theme};
use options::labels::{self, *};
use options::{OptionsWindow, Page};

const NONE: egui::Modifiers = egui::Modifiers::NONE;

/// The open window's draft.
fn draft(app: &MixJamApp) -> &options::OptionsDraft {
    &app.options.as_ref().expect("Options is open").draft
}

/// Focus the control named `name`, as Tab would.
fn focus(ctx: &egui::Context, app: &mut MixJamApp, name: &str) {
    let output = frame(ctx, app, vec![], NONE);
    focus_control(ctx, app, &output, name);
}

/// Open Options on `page` without a keyboard focus to give back.
fn open_on(ctx: &egui::Context, app: &mut MixJamApp, page: Page) -> egui::FullOutput {
    app.options = Some(OptionsWindow::new(&app.session, page));
    let mut output = frame(ctx, app, vec![], NONE);
    for _ in 0..2 {
        output = frame(ctx, app, vec![], NONE);
    }
    output
}

#[test]
fn options_keyboard_edits_applies_restores_and_cancels() {
    let (ctx, mut app, root) = isolated_fixture();
    frame(&ctx, &mut app, vec![], NONE);
    press(&ctx, &mut app, egui::Key::Comma, egui::Modifiers::COMMAND);
    press(&ctx, &mut app, egui::Key::ArrowDown, NONE);
    assert_eq!(draft(&app).page, Page::Workspace);
    // Tab passes the page selector, then the page's controls in order.
    for name in [
        labels::page(Page::Files),
        THEME,
        LANE_HEIGHT,
        FOLLOW_ON_STARTUP,
    ] {
        press(&ctx, &mut app, egui::Key::Tab, NONE);
        let output = frame(&ctx, &mut app, vec![], NONE);
        assert_eq!(focused_label(&output), Some(name));
    }
    press(&ctx, &mut app, egui::Key::Space, NONE);
    assert!(!draft(&app).values.follow_on_startup);
    assert!(app.session.options().follow_on_startup);
    // Apply ends the ring, which wraps to the window's own controls.
    focus(&ctx, &mut app, APPLY);
    let wrapped = press(&ctx, &mut app, egui::Key::Tab, NONE);
    assert_eq!(focused_label(&wrapped), Some("Collapse Options"));
    let reversed = press(&ctx, &mut app, egui::Key::Tab, egui::Modifiers::SHIFT);
    assert_eq!(focused_label(&reversed), Some(APPLY));
    press(&ctx, &mut app, egui::Key::Enter, NONE);
    assert!(!app.session.options().follow_on_startup);
    focus(&ctx, &mut app, RESTORE);
    press(&ctx, &mut app, egui::Key::Space, NONE);
    assert!(draft(&app).values.follow_on_startup);
    assert!(!app.session.options().follow_on_startup);
    focus(&ctx, &mut app, APPLY);
    press(&ctx, &mut app, egui::Key::Enter, NONE);
    assert!(app.session.options().follow_on_startup);
    focus(&ctx, &mut app, CANCEL);
    press(&ctx, &mut app, egui::Key::Space, NONE);
    assert!(app.options.is_none());
    assert!(!app.session.has_history_step(false));
    remove_settings(&root);
}

#[test]
fn expanded_device_details_tabs_from_its_header_into_its_body() {
    let (ctx, mut app, root) = isolated_fixture();
    open_on(&ctx, &mut app, Page::Audio);
    click_named(&ctx, &mut app, DEVICE_DETAILS);
    focus(&ctx, &mut app, BUFFER_TARGET);
    for name in [DEVICE_DETAILS, SAVE_CAPTURE, PREVIEW_LEVEL] {
        let output = press(&ctx, &mut app, egui::Key::Tab, NONE);
        assert_eq!(focused_label(&output), Some(name));
    }
    remove_settings(&root);
}

#[test]
fn options_escape_respects_a_higher_utility_window() {
    let (ctx, mut app, root) = isolated_fixture();
    app.open_options(&ctx);
    frame(&ctx, &mut app, vec![], NONE);
    app.sample_import.details_open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], NONE);
    }
    assert!(dialog::top_window(&ctx, "Import details"));
    press(&ctx, &mut app, egui::Key::Escape, NONE);
    assert!(!app.sample_import.details_open);
    assert!(app.options.is_some());
    remove_settings(&root);
}

#[test]
fn options_selector_and_footer_fit_supported_windows() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        for page in Page::ALL {
            let (ctx, mut app, root) = isolated_fixture();
            app.options = Some(OptionsWindow::new(&app.session, page));
            let mut output = frame_at_size(&ctx, &mut app, vec![], NONE, size);
            for _ in 0..2 {
                output = frame_at_size(&ctx, &mut app, vec![], NONE, size);
            }
            let screen_rect = Rect::from_min_size(Pos2::ZERO, size);
            for name in Page::ALL
                .map(labels::page)
                .iter()
                .chain(&[APPLY, CANCEL, RESTORE])
            {
                let bounds = screen(&output).node(name).rect();
                assert!(
                    screen_rect.contains_rect(bounds),
                    "{name} is outside {size:?}: {bounds:?}"
                );
            }
            assert!(
                screen(&output).node(APPLY).rect().top()
                    > screen(&output)
                        .node(labels::page(Page::Files))
                        .rect()
                        .bottom()
            );
            remove_settings(&root);
        }
    }
}

#[test]
fn the_footer_stays_in_place_as_the_pages_change() {
    let (ctx, mut app, root) = isolated_fixture();
    app.open_options(&ctx);
    let mut output = frame(&ctx, &mut app, vec![], NONE);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, vec![], NONE);
    }
    let apply = screen(&output).node(APPLY).rect();
    for page in [Page::Workspace, Page::Files, Page::Audio] {
        click_named(&ctx, &mut app, labels::page(page));
        let output = frame(&ctx, &mut app, vec![], NONE);
        assert_eq!(draft(&app).page, page);
        assert_eq!(
            screen(&output).node(APPLY).rect(),
            apply,
            "{page:?} moved the footer"
        );
    }
    remove_settings(&root);
}

#[test]
fn options_gear_focuses_one_window_and_keeps_the_draft_beside_libraries() {
    let (ctx, mut app, root) = isolated_fixture();
    frame(&ctx, &mut app, vec![], NONE);
    click_named(&ctx, &mut app, "Options");
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    click_named(&ctx, &mut app, labels::page(Page::Files));
    click_named(&ctx, &mut app, OPEN_LIBRARIES);
    assert!(app.sample_libraries_open());
    assert!(!draft(&app).values.follow_on_startup);
    assert!(app.session.options().follow_on_startup);
    click_named(&ctx, &mut app, "Options");
    assert!(app.sample_libraries_open());
    assert!(!draft(&app).values.follow_on_startup);
    let output = frame(&ctx, &mut app, vec![], NONE);
    let dialogs = |title: &str| {
        screen(&output)
            .nodes()
            .filter(|node| {
                node.role() == egui::accesskit::Role::Dialog && node.node().label() == Some(title)
            })
            .count()
    };
    assert_eq!(dialogs("OPTIONS"), 1);
    assert_eq!(dialogs("SAMPLE LIBRARIES"), 1);
    click_named(&ctx, &mut app, "Collapse Options");
    assert!(dialog::is_collapsed(&ctx, WINDOW));
    click_named(&ctx, &mut app, "Options");
    assert!(!dialog::is_collapsed(&ctx, WINDOW));
    assert!(!draft(&app).values.follow_on_startup);
    click_named(&ctx, &mut app, CANCEL);
    assert!(draft(&app).confirm_discard);
    click_named(&ctx, &mut app, DISCARD_CHANGES);
    assert!(app.options.is_none());
    // Closing Options leaves Sample libraries open.
    assert!(app.sample_libraries_open());
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert_eq!(
        screen(&output).node("Options").node().toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    remove_settings(&root);
}

#[test]
fn options_arrows_walk_the_pages_without_changing_preferences() {
    let (ctx, mut app, root) = isolated_fixture();
    app.open_options(&ctx);
    frame(&ctx, &mut app, vec![], NONE);
    for page in [Page::Workspace, Page::Files, Page::Audio] {
        press(&ctx, &mut app, egui::Key::ArrowDown, NONE);
        assert_eq!(draft(&app).page, page);
    }
    press(&ctx, &mut app, egui::Key::ArrowUp, NONE);
    assert_eq!(draft(&app).page, Page::Files);
    assert_eq!(draft(&app).values, *app.session.options());
    remove_settings(&root);
}

#[test]
fn options_keyboard_cancel_restores_focus_and_keeps_draft_out_of_song() {
    let (ctx, mut app, root) = isolated_fixture();
    focus(&ctx, &mut app, "Follow");
    let previous_focus = ctx.memory(|memory| memory.focused());
    assert!(previous_focus.is_some());
    let project = app.session.project().clone();
    let revision = app.session.options_revision();
    press(&ctx, &mut app, egui::Key::Comma, egui::Modifiers::COMMAND);
    assert!(app.options.is_some());
    press(&ctx, &mut app, egui::Key::ArrowDown, NONE);
    assert_eq!(draft(&app).page, Page::Workspace);
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    press(&ctx, &mut app, egui::Key::Escape, NONE);
    assert!(draft(&app).confirm_discard);
    focus(&ctx, &mut app, DISCARD_CHANGES);
    press(&ctx, &mut app, egui::Key::Space, NONE);
    assert!(app.options.is_none());
    assert_eq!(ctx.memory(|memory| memory.focused()), previous_focus);
    assert!(app.session.options().follow_on_startup);
    assert_eq!(app.session.options_revision(), revision);
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.has_history_step(false));
    assert!(!root.join("options.json").exists());
    remove_settings(&root);
}

#[test]
fn options_apply_cancel_restore_and_stale_reload_share_one_active_value() {
    let (ctx, mut app, root) = isolated_fixture();
    open_on(&ctx, &mut app, Page::Workspace);
    click_named(&ctx, &mut app, SECTION_BOOKMARKS);
    assert!(app.session.options().show_section_bookmarks);
    click_named(&ctx, &mut app, APPLY);
    assert!(!app.session.options().show_section_bookmarks);
    assert!(!app.sections.bookmarks);
    assert!(app.options.is_some());
    click_named(&ctx, &mut app, RESTORE);
    assert!(!app.session.options().show_section_bookmarks);
    app.session.remember_section_bookmarks(true).unwrap();
    let old_revision = draft(&app).revision;
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert!(screen(&output).node(APPLY).node().is_disabled());
    assert_eq!(
        screen(&output).node(APPLY).node().description(),
        Some(mixjam_techno::application::options::STALE_DRAFT)
    );
    assert_eq!(draft(&app).revision, old_revision);
    assert!(draft(&app).restored);
    click_named(&ctx, &mut app, RELOAD);
    assert_eq!(draft(&app).revision, app.session.options_revision());
    click_named(&ctx, &mut app, CANCEL);
    assert!(app.options.is_none());
    assert!(app.session.options().show_section_bookmarks);
    remove_settings(&root);
}

#[test]
fn options_apply_and_close_are_honest_about_unapplied_changes() {
    let (ctx, mut app, root) = isolated_fixture();
    let output = open_on(&ctx, &mut app, Page::Audio);
    assert!(screen(&output).node(APPLY).node().is_disabled());
    assert_eq!(
        screen(&output).node(APPLY).node().description(),
        Some(NOTHING_TO_APPLY)
    );
    // A clean draft closes without asking.
    click_named(&ctx, &mut app, "Close Options");
    assert!(app.options.is_none());
    open_on(&ctx, &mut app, Page::Workspace);
    click_named(&ctx, &mut app, SECTION_BOOKMARKS);
    let output = frame(&ctx, &mut app, vec![], NONE);
    let count = unapplied(1);
    assert!(!screen(&output).node(APPLY).node().is_disabled());
    // The count is in the warning colour, and screen readers hear it.
    assert_eq!(screen(&output).text(&count).color(), roles().warning);
    assert!(screen(&output).nodes().any(|node| {
        node.value() == Some(count.as_str())
            && node.node().live() == Some(egui::accesskit::Live::Polite)
    }));
    // The page marker is text for screen readers too.
    let workspace = screen(&output).node(labels::page(Page::Workspace));
    assert_eq!(workspace.node().description(), Some(count.as_str()));
    let audio = screen(&output).node(labels::page(Page::Audio));
    assert_eq!(audio.node().description(), None);
    assert_eq!(
        draft(&app).changes(&options::facts(&app.session)),
        [0, 1, 0]
    );
    // Setting the value back makes the draft clean again.
    click_named(&ctx, &mut app, SECTION_BOOKMARKS);
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert!(screen(&output).node(APPLY).node().is_disabled());
    click_named(&ctx, &mut app, SECTION_BOOKMARKS);
    // Close asks first; Keep editing and Escape keep the draft.
    click_named(&ctx, &mut app, "Close Options");
    assert!(draft(&app).confirm_discard);
    assert_eq!(app.blocking_modal(&ctx), Some(DISCARD));
    click_named(&ctx, &mut app, KEEP_EDITING);
    assert!(!draft(&app).confirm_discard);
    // The closed prompt releases its modal layer on the next frame.
    frame(&ctx, &mut app, vec![], NONE);
    press(&ctx, &mut app, egui::Key::Escape, NONE);
    assert!(draft(&app).confirm_discard);
    press(&ctx, &mut app, egui::Key::Escape, NONE);
    assert!(!draft(&app).confirm_discard && !draft(&app).values.show_section_bookmarks);
    // Apply changes from the prompt applies, then closes.
    click_named(&ctx, &mut app, CANCEL);
    click_named(&ctx, &mut app, APPLY_AND_CLOSE);
    assert!(app.options.is_none());
    assert!(!app.session.options().show_section_bookmarks);
    remove_settings(&root);
}

#[test]
fn a_collapsed_discard_prompt_clears_when_its_draft_becomes_clean() {
    for action in ["apply", "reload", "revert"] {
        let (ctx, mut app, root) = isolated_fixture();
        open_on(&ctx, &mut app, Page::Workspace);
        click_named(&ctx, &mut app, SECTION_BOOKMARKS);
        click_named(&ctx, &mut app, CANCEL);
        assert!(draft(&app).confirm_discard);
        click_named(&ctx, &mut app, &format!("Collapse {DISCARD}"));
        frame(&ctx, &mut app, vec![], NONE);
        assert_eq!(app.blocking_modal(&ctx), None);
        let control = match action {
            "apply" => APPLY,
            "reload" => {
                let mut active = app.session.options().clone();
                active.follow_on_startup = false;
                app.session
                    .apply_options(active, app.session.options_revision(), false)
                    .unwrap();
                RELOAD
            }
            "revert" => SECTION_BOOKMARKS,
            _ => unreachable!(),
        };
        focus(&ctx, &mut app, control);
        press(&ctx, &mut app, egui::Key::Space, NONE);
        let output = frame(&ctx, &mut app, vec![], NONE);
        let facts = options::facts(&app.session);
        assert!(!draft(&app).pending(&facts), "{action}");
        assert!(
            !draft(&app).confirm_discard,
            "{action} left an obsolete prompt"
        );
        assert!(
            !has_control(&output, &format!("Expand {DISCARD}")),
            "{action} left the obsolete confirmation on screen"
        );
        click_named(&ctx, &mut app, CANCEL);
        assert!(app.options.is_none());
        remove_settings(&root);
    }
}

#[test]
fn options_folder_picker_cancel_retains_draft_and_selection_is_not_apply() {
    let (ctx, mut app, root) = isolated_fixture();
    app.open_options(&ctx);
    let before = draft(&app).values.clone();
    app.finish_path(FileRequest::OptionsFolder, None);
    assert_eq!(draft(&app).values, before);
    app.finish_path(FileRequest::OptionsFolder, Some(root.clone()));
    assert_eq!(draft(&app).values.fixed_song_folder, Some(root.clone()));
    assert_eq!(app.session.options(), &before);
    assert!(!root.join("options.json").exists());
    remove_settings(&root);
}

#[test]
fn files_opens_sample_libraries_beside_options() {
    let (ctx, mut app, root) = isolated_fixture();
    open_on(&ctx, &mut app, Page::Files);
    click_named(&ctx, &mut app, OPEN_LIBRARIES);
    assert!(app.sample_libraries_open());
    assert!(app.options.is_some(), "Options stays open");
    remove_settings(&root);
}

#[test]
fn sample_playback_quality_applies_restores_cancels_and_flags_reduced_playback() {
    let (ctx, mut app, root) = isolated_fixture();
    let output = open_on(&ctx, &mut app, Page::Audio);
    assert_eq!(screen(&output).node(SAMPLE_QUALITY).value(), Some("High"));
    choose(&ctx, &mut app, SAMPLE_QUALITY, "Low");
    assert_eq!(
        draft(&app).values.sample_playback_quality,
        SampleQuality::Low
    );
    // Only Apply changes the active preference.
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::High
    );
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert_eq!(screen(&output).node(SAMPLE_QUALITY).value(), Some("Low"));
    assert!(shows_text(&output, QUALITY_REDUCED));
    click_named(&ctx, &mut app, APPLY);
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::Low
    );
    assert!(app.session.options().quality_reduced());
    let saved = std::fs::read_to_string(root.join("options.json")).unwrap();
    assert!(
        saved.contains("\"samplePlaybackQuality\": \"low\""),
        "{saved}"
    );
    // Restore puts High in the draft; Cancel then asks and discards it.
    click_named(&ctx, &mut app, RESTORE);
    assert_eq!(
        draft(&app).values.sample_playback_quality,
        SampleQuality::High
    );
    click_named(&ctx, &mut app, CANCEL);
    click_named(&ctx, &mut app, DISCARD_CHANGES);
    assert!(app.options.is_none());
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::Low
    );
    // The main window's status flags the reduced playback too, in short.
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert!(shows_text(&output, "Reduced quality"));
    // Restore and Apply return to High, and the flag goes.
    open_on(&ctx, &mut app, Page::Audio);
    click_named(&ctx, &mut app, RESTORE);
    click_named(&ctx, &mut app, APPLY);
    assert!(!app.session.options().quality_reduced());
    click_named(&ctx, &mut app, CANCEL);
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert!(!shows_text(&output, "Reduced quality"));
    assert!(!app.session.has_history_step(false));
    remove_settings(&root);
}

#[test]
fn theme_applies_at_once_restores_and_cancels_without_touching_the_song() {
    let (ctx, mut app, root) = isolated_fixture();
    let project = app.session.project().clone();
    let output = open_on(&ctx, &mut app, Page::Workspace);
    assert_eq!(
        screen(&output).node(THEME).value(),
        Some(theme(Theme::Studio))
    );
    assert_eq!(theme::current(), Theme::Studio);
    choose(&ctx, &mut app, THEME, theme(Theme::Aero));
    // A draft does not repaint the application.
    frame(&ctx, &mut app, vec![], NONE);
    assert_eq!(theme::current(), Theme::Studio);
    assert_eq!(
        ctx.style().visuals.panel_fill,
        theme::of(Theme::Studio).canvas
    );
    click_named(&ctx, &mut app, APPLY);
    frame(&ctx, &mut app, vec![], NONE);
    assert_eq!(app.session.options().theme, Theme::Aero);
    assert_eq!(theme::current(), Theme::Aero);
    assert_eq!(
        ctx.style().visuals.panel_fill,
        theme::of(Theme::Aero).canvas
    );
    let saved = std::fs::read_to_string(root.join("options.json")).unwrap();
    assert!(saved.contains("\"theme\": \"aero\""), "{saved}");
    // Restore puts Studio in the draft; Cancel discards it and Aero stays.
    click_named(&ctx, &mut app, RESTORE);
    assert_eq!(draft(&app).values.theme, Theme::Studio);
    click_named(&ctx, &mut app, CANCEL);
    click_named(&ctx, &mut app, DISCARD_CHANGES);
    frame(&ctx, &mut app, vec![], NONE);
    assert!(app.options.is_none());
    assert_eq!(theme::current(), Theme::Aero);
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.unsaved());
    assert!(!app.session.has_history_step(false));
    theme::paint_as(Theme::Studio);
    remove_settings(&root);
}

#[test]
fn lane_height_applies_at_once_without_touching_the_song() {
    let (ctx, mut app, root) = isolated_fixture();
    let project = app.session.project().clone();
    let output = open_on(&ctx, &mut app, Page::Workspace);
    assert_eq!(app.score.row_height, 30.0);
    assert_eq!(
        screen(&output).node(LANE_HEIGHT).value(),
        Some(lane_height(LaneHeight::Px30))
    );
    for (height, saved_as) in [(LaneHeight::Px20, "20px"), (LaneHeight::Px40, "40px")] {
        choose(&ctx, &mut app, LANE_HEIGHT, lane_height(height));
        // A draft does not resize the rows.
        frame(&ctx, &mut app, vec![], NONE);
        assert_eq!(app.score.row_height, 30.0);
        click_named(&ctx, &mut app, APPLY);
        frame(&ctx, &mut app, vec![], NONE);
        assert_eq!(app.session.options().lane_height, height);
        assert_eq!(app.score.row_height, height.points());
        let saved = std::fs::read_to_string(root.join("options.json")).unwrap();
        assert!(
            saved.contains(&format!("\"laneHeight\": \"{saved_as}\"")),
            "{saved}"
        );
        // Put the draft's starting point back at 30 px for the next choice.
        choose(&ctx, &mut app, LANE_HEIGHT, lane_height(LaneHeight::Px30));
        click_named(&ctx, &mut app, APPLY);
        frame(&ctx, &mut app, vec![], NONE);
        assert_eq!(app.score.row_height, 30.0);
    }
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.unsaved());
    assert!(!app.session.has_history_step(false));
    remove_settings(&root);
}

#[test]
fn hiding_tooltips_keeps_accessible_control_descriptions() {
    let (ctx, mut app, root) = isolated_fixture();
    let mut values = app.session.options().clone();
    values.show_tooltips = false;
    app.session
        .apply_options(values, app.session.options_revision(), false)
        .unwrap();
    let output = frame(&ctx, &mut app, vec![], NONE);
    assert_eq!(
        screen(&output).node("Follow").node().description(),
        Some("Keep the playhead in view during playback and seeking.")
    );
    remove_settings(&root);
}
