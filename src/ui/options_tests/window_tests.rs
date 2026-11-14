//! Options window: keyboard use, focus, escape, and applying, cancelling and restoring
//! the draft.

use super::*;

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn focus(ctx: &egui::Context, app: &mut MixJamApp, name: &str) {
    let size = Vec2::new(1024.0, 768.0);
    let output = frame(ctx, app, size, vec![]);
    let id = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(name))
        .unwrap()
        .0;
    frame(
        ctx,
        app,
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: id,
                data: None,
            },
        )],
    );
}

#[test]
fn options_keyboard_edits_applies_restores_and_cancels() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    frame(&ctx, &mut app, size, vec![]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
    );
    assert_eq!(app.options.as_ref().unwrap().page, Page::Workspace);
    // Tab traverses the page selector before the option controls.
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    let focus_id = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .focus;
    assert_eq!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(id, _)| *id == focus_id)
            .and_then(|(_, node)| node.label()),
        Some("Files")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Sample libraries")
    );
    // The page starts with the theme and the lane height, then the startup
    // switches.
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Theme")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Lane height")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    assert!(app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Apply");
    let wrapped = frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let tree = wrapped.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Collapse Options")
    );
    let reversed = frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)],
    );
    let tree = reversed.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Apply")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(!app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Restore options defaults");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(app.options.as_ref().unwrap().values.follow_on_startup);
    assert!(!app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Apply");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Cancel");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(app.options.is_none());
    assert!(!app.session.has_history_step(false));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_escape_respects_a_higher_utility_window() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    app.sample_import.details_open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, size, vec![]);
    }
    assert!(dialog::top_window(&ctx, "Import details"));
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.sample_import.details_open);
    assert!(app.options.is_some());
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn options_actions_and_page_navigation_fit_supported_windows() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        for page in [Page::Audio, Page::Workspace, Page::Files, Page::Libraries] {
            let (ctx, mut app, root) = fixture();
            app.options = Some(OptionsDraft::new(&app.session, page));
            let mut output = frame(&ctx, &mut app, size, vec![]);
            for _ in 0..2 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            let actions: &[&str] = if page == Page::Libraries {
                &["Done"]
            } else {
                &["Apply", "Cancel", "Restore options defaults"]
            };
            for name in ["Audio", "Workspace", "Files", "Sample libraries"]
                .iter()
                .chain(actions)
            {
                let bounds = node(&output, name).bounds().unwrap();
                assert!(
                    bounds.x0 >= 0.0
                        && bounds.y0 >= 0.0
                        && bounds.x1 <= f64::from(size.x)
                        && bounds.y1 <= f64::from(size.y),
                    "{name} is outside {size:?}: {bounds:?}"
                );
            }
            assert!(
                node(&output, actions[0]).bounds().unwrap().y0
                    > node(&output, "Sample libraries").bounds().unwrap().y1
            );
            if root.exists() {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
}

#[test]
fn options_gear_focuses_one_window_and_preserves_the_draft() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    frame(&ctx, &mut app, size, vec![]);
    click(&ctx, &mut app, "Options");
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    click(&ctx, &mut app, "Files");
    click(&ctx, &mut app, "Sample libraries");
    assert!(app.sample_libraries_open());
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    assert!(app.session.options().follow_on_startup);
    click(&ctx, &mut app, "Options");
    assert!(app.sample_libraries_open());
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    let output = frame(&ctx, &mut app, size, vec![]);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|(_, node)| node.role() == egui::accesskit::Role::Dialog
                && node.label() == Some("OPTIONS"))
            .count(),
        1
    );
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(_, node)| node.role() == egui::accesskit::Role::Dialog
                && node.label() == Some("SAMPLE LIBRARIES"))
    );
    click(&ctx, &mut app, "Collapse Options");
    assert!(dialog::is_collapsed(&ctx, "Options"));
    click(&ctx, &mut app, "Options");
    assert!(!dialog::is_collapsed(&ctx, "Options"));
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    click(&ctx, &mut app, "Done");
    assert!(app.options.as_ref().unwrap().confirm_discard);
    click(&ctx, &mut app, "Discard changes");
    assert!(app.options.is_none());
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(
        node(&output, "Options").toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn options_keyboard_navigates_all_four_pages_without_changing_preferences() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    for page in [Page::Workspace, Page::Files, Page::Libraries, Page::Audio] {
        frame(
            &ctx,
            &mut app,
            size,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        );
        assert_eq!(app.options.as_ref().unwrap().page, page);
    }
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE)],
    );
    assert_eq!(app.options.as_ref().unwrap().page, Page::Libraries);
    assert_eq!(app.options.as_ref().unwrap().values, *app.session.options());
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn options_keyboard_cancel_restores_focus_and_keeps_draft_out_of_song() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    let output = frame(&ctx, &mut app, size, vec![]);
    let follow = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Follow"))
        .unwrap()
        .0;
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: follow,
                data: None,
            },
        )],
    );
    let previous_focus = ctx.memory(|memory| memory.focused());
    assert!(previous_focus.is_some());
    let project = app.session.project().clone();
    let revision = app.session.options_revision();
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)],
    );
    assert!(app.options.is_some());
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
    );
    assert_eq!(app.options.as_ref().unwrap().page, Page::Workspace);
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(app.options.as_ref().unwrap().confirm_discard);
    focus(&ctx, &mut app, "Discard changes");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(app.options.is_none());
    assert_eq!(ctx.memory(|memory| memory.focused()), previous_focus);
    assert!(app.session.options().follow_on_startup);
    assert_eq!(app.session.options_revision(), revision);
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.has_history_step(false));
    assert!(!root.join("options.json").exists());
}

#[test]
fn options_apply_cancel_restore_and_stale_reload_share_one_active_value() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    click(&ctx, &mut app, "Workspace");
    click(&ctx, &mut app, "Show section bookmarks");
    assert!(app.session.options().show_section_bookmarks);
    click(&ctx, &mut app, "Apply");
    assert!(!app.session.options().show_section_bookmarks);
    assert!(!app.sections.bookmarks);
    assert!(app.options.is_some());
    click(&ctx, &mut app, "Restore options defaults");
    assert!(!app.session.options().show_section_bookmarks);
    app.session.remember_section_bookmarks(true).unwrap();
    let old_revision = app.options.as_ref().unwrap().revision;
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(node(&output, "Apply").is_disabled());
    assert_eq!(app.options.as_ref().unwrap().revision, old_revision);
    assert!(app.options.as_ref().unwrap().restored);
    click(&ctx, &mut app, "Reload active options");
    assert_eq!(
        app.options.as_ref().unwrap().revision,
        app.session.options_revision()
    );
    click(&ctx, &mut app, "Cancel");
    assert!(app.options.is_none());
    assert!(app.session.options().show_section_bookmarks);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_apply_and_close_are_honest_about_unapplied_changes() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(node(&output, "Apply").is_disabled());
    assert_eq!(
        node(&output, "Apply").description(),
        Some("No unapplied changes.")
    );
    // A clean draft closes without asking.
    click(&ctx, &mut app, "Close Options");
    assert!(app.options.is_none());
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    click(&ctx, &mut app, "Workspace");
    click(&ctx, &mut app, "Show section bookmarks");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(!node(&output, "Apply").is_disabled());
    assert!(has_text(&output, "1 unapplied change"));
    // The marker and the count are text for screen readers too.
    assert_eq!(
        node(&output, "Workspace").description(),
        Some("1 unapplied change")
    );
    assert_eq!(node(&output, "Audio").description(), None);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert!(tree.nodes.iter().any(|(_, node)| {
        node.value() == Some("1 unapplied change")
            && node.live() == Some(egui::accesskit::Live::Polite)
    }));
    assert_eq!(
        app.options.as_ref().unwrap().changes(&app.session),
        [0, 1, 0, 0]
    );
    // Setting the value back makes the draft clean again.
    click(&ctx, &mut app, "Show section bookmarks");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(node(&output, "Apply").is_disabled());
    click(&ctx, &mut app, "Show section bookmarks");
    // Close asks first; Keep editing and Escape keep the draft.
    click(&ctx, &mut app, "Close Options");
    assert!(app.options.as_ref().unwrap().confirm_discard);
    assert_eq!(app.blocking_modal(&ctx), Some(DISCARD));
    click(&ctx, &mut app, "Keep editing");
    assert!(!app.options.as_ref().unwrap().confirm_discard);
    // The closed prompt releases its modal layer on the next frame.
    frame(&ctx, &mut app, size, vec![]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(app.options.as_ref().unwrap().confirm_discard);
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    let draft = app.options.as_ref().unwrap();
    assert!(!draft.confirm_discard && !draft.values.show_section_bookmarks);
    // Apply changes from the prompt applies, then closes.
    click(&ctx, &mut app, "Cancel");
    click(&ctx, &mut app, "Apply changes");
    assert!(app.options.is_none());
    assert!(!app.session.options().show_section_bookmarks);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_collapsed_discard_prompt_clears_when_its_draft_becomes_clean() {
    for action in ["apply", "reload", "revert"] {
        let (ctx, mut app, root) = fixture();
        let size = Vec2::new(1024.0, 768.0);
        app.open_options(&ctx);
        frame(&ctx, &mut app, size, vec![]);
        click(&ctx, &mut app, "Workspace");
        click(&ctx, &mut app, "Show section bookmarks");
        click(&ctx, &mut app, "Cancel");
        assert!(app.options.as_ref().unwrap().confirm_discard);
        click(&ctx, &mut app, "Collapse Discard changes?");
        frame(&ctx, &mut app, size, vec![]);
        assert_eq!(app.blocking_modal(&ctx), None);
        let control = match action {
            "apply" => "Apply",
            "reload" => {
                let mut active = app.session.options().clone();
                active.follow_on_startup = false;
                app.session
                    .apply_options(active, app.session.options_revision(), false)
                    .unwrap();
                "Reload active options"
            }
            "revert" => "Show section bookmarks",
            _ => unreachable!(),
        };
        focus(&ctx, &mut app, control);
        frame(
            &ctx,
            &mut app,
            size,
            vec![key(egui::Key::Space, egui::Modifiers::NONE)],
        );
        let output = frame(&ctx, &mut app, size, vec![]);
        let draft = app.options.as_ref().unwrap();
        assert!(!draft.pending(&app.session), "{action}");
        assert!(!draft.confirm_discard, "{action} left an obsolete prompt");
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        assert!(
            !tree
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Expand Discard changes?")),
            "{action} left the obsolete confirmation on screen"
        );
        click(&ctx, &mut app, "Cancel");
        assert!(app.options.is_none());
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn restored_defaults_stay_pending_while_they_replace_an_invalid_file() {
    let root = std::env::temp_dir().join(format!("mixjam-options-invalid-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("options.json"), "{").unwrap();
    let session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(root.clone());
    assert!(session.options_invalid());
    let mut draft = OptionsDraft::new(&session, Page::Audio);
    assert!(!draft.pending(&session));
    draft.values = AppOptions::default();
    draft.restored = true;
    assert!(draft.pending(&session));
    assert_eq!(draft.changes(&session), [0; 4]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_folder_picker_cancel_retains_draft_and_selection_is_not_apply() {
    let (ctx, mut app, root) = fixture();
    app.open_options(&ctx);
    let before = app.options.as_ref().unwrap().values.clone();
    app.finish_path(FileRequest::OptionsFolder, None);
    assert_eq!(app.options.as_ref().unwrap().values, before);
    app.finish_path(FileRequest::OptionsFolder, Some(root.clone()));
    assert_eq!(
        app.options.as_ref().unwrap().values.fixed_song_folder,
        Some(root.clone())
    );
    assert_eq!(app.session.options(), &before);
    assert!(!root.join("options.json").exists());
}
