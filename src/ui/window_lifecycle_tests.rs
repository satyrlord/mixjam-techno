use super::*;
use mixjam_techno::library::Library;

fn fixture() -> (egui::Context, MixJamApp) {
    let ctx = egui::Context::default();
    configure_style(&ctx);
    let mut app = MixJamApp::from_session(
        Session::headless(Arc::new(Library::empty()), Project::default()).unwrap(),
        "Printed panel lifecycle".into(),
        false,
    );
    app.capture_path = None;
    (ctx, app)
}

fn collapsed(ctx: &egui::Context, identity: &str, value: bool) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(("hardware-collapsed", identity)), value));
}

fn frame(ctx: &egui::Context, app: &mut MixJamApp, events: Vec<egui::Event>) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| app.draw(ctx),
    );
}

fn prompt() -> PathPrompt {
    PathPrompt {
        request: FileRequest::SaveSong,
        text: "C:\\Draft\\song.mixjam".into(),
        error: Some("Keep this draft error.".into()),
        focused: false,
    }
}

#[test]
fn collapsed_dialogs_release_workspace_keyboard_without_cancelling_tasks() {
    for identity in ["Save your changes?", "Import sound", "Choose path"] {
        let (ctx, mut app) = fixture();
        match identity {
            "Save your changes?" => app.pending_action = Some(PendingAction::New),
            "Import sound" => app.import_path = Some(PathBuf::from("unread.wav")),
            _ => app.path_prompt = Some(prompt()),
        }
        assert_eq!(app.blocking_modal(&ctx), Some(identity));
        collapsed(&ctx, identity, true);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        assert_eq!(app.blocking_modal(&ctx), None);
        assert!(ctx.memory(|memory| memory.top_modal_layer().is_none()));
        app.erase = true;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            !app.erase,
            "Workspace Escape must work while {identity} is collapsed."
        );
        collapsed(&ctx, identity, false);
        assert_eq!(
            app.blocking_modal(&ctx),
            Some(identity),
            "Collapse must retain the pending task."
        );
    }
}

#[test]
fn collapsed_nested_path_suspends_parent_confirmation_and_preserves_path_draft() {
    let (ctx, mut app) = fixture();
    app.pending_action = Some(PendingAction::New);
    app.path_prompt = Some(prompt());
    collapsed(&ctx, "Choose path", true);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(app.blocking_modal(&ctx), None);
    assert!(
        ctx.memory(|memory| memory.top_modal_layer().is_none()),
        "The parent must not retain a hidden modal blocker."
    );
    assert!(matches!(app.pending_action, Some(PendingAction::New)));
    assert_eq!(
        app.path_prompt.as_ref().unwrap().text,
        "C:\\Draft\\song.mixjam"
    );
    assert_eq!(
        app.path_prompt.as_ref().unwrap().error.as_deref(),
        Some("Keep this draft error.")
    );
    assert!(
        !app.path_prompt.as_ref().unwrap().focused,
        "Collapsed contents must not take text focus."
    );
}

#[test]
fn resumed_save_waits_for_expansion_before_running_pending_action() {
    let (ctx, mut app) = fixture();
    app.pending_action = Some(PendingAction::Close);
    app.resume_save_changes = true;
    collapsed(&ctx, "Save your changes?", true);
    frame(&ctx, &mut app, vec![]);
    assert!(!app.allow_close);
    assert!(app.resume_save_changes);
    assert!(matches!(app.pending_action, Some(PendingAction::Close)));
    collapsed(&ctx, "Save your changes?", false);
    frame(&ctx, &mut app, vec![]);
    assert!(app.allow_close);
    assert!(!app.resume_save_changes);
    assert!(app.pending_action.is_none());
}

#[test]
fn workspace_file_requests_cannot_replace_paused_tasks() {
    let (ctx, mut app) = fixture();
    app.pending_action = Some(PendingAction::New);
    collapsed(&ctx, "Save your changes?", true);
    app.request_action(PendingAction::Close, &ctx);
    assert!(matches!(app.pending_action, Some(PendingAction::New)));
    assert!(!dialog::is_collapsed(&ctx, "Save your changes?"));
    assert!(!app.allow_close);

    app.pending_action = None;
    app.path_prompt = Some(prompt());
    collapsed(&ctx, "Choose path", true);
    app.ask_path(FileRequest::ExportMix);
    assert!(matches!(
        app.path_prompt.as_ref().unwrap().request,
        FileRequest::SaveSong
    ));
    assert_eq!(
        app.path_prompt.as_ref().unwrap().text,
        "C:\\Draft\\song.mixjam"
    );
    app.request_action(PendingAction::New, &ctx);
    assert!(app.pending_action.is_none());
    assert!(!dialog::is_collapsed(&ctx, "Choose path"));

    app.path_prompt = None;
    app.import_path = Some(PathBuf::from("first.wav"));
    app.import_bpm = 127.0;
    app.ask_path(FileRequest::ImportSound);
    app.finish_path(FileRequest::ImportSound, Some(PathBuf::from("second.wav")));
    assert_eq!(app.import_path, Some(PathBuf::from("first.wav")));
    assert_eq!(app.import_bpm, 127.0);
    app.request_action(PendingAction::New, &ctx);
    assert!(app.pending_action.is_none());
    assert_eq!(app.import_path, Some(PathBuf::from("first.wav")));
}
