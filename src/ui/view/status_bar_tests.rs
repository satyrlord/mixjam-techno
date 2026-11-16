use super::*;

fn bar_fixture() -> (egui::Context, MixJamApp) {
    let ctx = egui::Context::default();
    configure_style(&ctx);
    ctx.enable_accesskit();
    let session = Session::headless(
        Arc::new(mixjam_techno::library::Library::empty()),
        Project::default(),
    )
    .unwrap();
    let app = MixJamApp::from_session(session, DEFAULT_STATUS.to_owned(), false);
    (ctx, app)
}

fn bar_frame(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ctx| {
            let bar = app.status_bar_space(ctx);
            app.status_bar(ctx, bar);
        },
    )
}

fn open_history(ctx: &egui::Context, app: &mut MixJamApp) {
    let output = bar_frame(ctx, app, vec![]);
    let target = output
        .platform_output
        .accesskit_update
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Status"))
        .unwrap()
        .0;
    bar_frame(
        ctx,
        app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
    );
    assert!(egui::Popup::is_any_open(ctx));
}

#[test]
fn the_status_popup_trigger_exposes_its_name_and_message_as_a_button() {
    let (ctx, mut app) = bar_fixture();
    app.error("Cannot save the song.");
    let output = bar_frame(&ctx, &mut app, vec![]);
    let update = output.platform_output.accesskit_update.unwrap();
    let (_, status) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Status"))
        .unwrap();
    assert_eq!(status.role(), egui::accesskit::Role::Button);
    assert_eq!(status.value(), Some("Error: Cannot save the song."));
}

#[test]
fn a_faded_error_keeps_its_history_popup_open_after_becoming_read() {
    let (ctx, mut app) = bar_fixture();
    app.error("Cannot save the song.");
    bar_frame(&ctx, &mut app, vec![]);
    app.status_log.entries[0].at = Instant::now() - Duration::from_secs(61);
    let status_id = bar_frame(&ctx, &mut app, vec![])
        .platform_output
        .accesskit_update
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Status"))
        .unwrap()
        .0;
    open_history(&ctx, &mut app);
    for _ in 0..4 {
        let output = bar_frame(&ctx, &mut app, vec![]);
        assert!(
            egui::Popup::is_any_open(&ctx),
            "Marking an old error read must not close its popup."
        );
        assert!(
            output
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .iter()
                .any(|(id, node)| *id == status_id && node.label() == Some("Status")),
            "Adding the faded-error dot must preserve the status control's identity."
        );
    }
}

#[test]
fn long_history_scrolls_to_the_oldest_message_inside_the_viewport() {
    let (ctx, mut app) = bar_fixture();
    let oldest = format!(
        "Oldest error. {}",
        "A long error with a saved path. ".repeat(35)
    );
    for index in 0..STATUS_HISTORY {
        app.error(if index == 0 {
            oldest.clone()
        } else {
            format!(
                "Error {index}. {}",
                "The file could not be opened. ".repeat(35)
            )
        });
        bar_frame(&ctx, &mut app, vec![]);
    }
    open_history(&ctx, &mut app);
    let mut output = bar_frame(&ctx, &mut app, vec![]);
    for _ in 0..3 {
        output = bar_frame(&ctx, &mut app, vec![]);
    }
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Recent messages vertical scroll bar"))
        .expect("Long history must have a named scroll bar.")
        .1
        .bounds()
        .unwrap();
    assert!(bounds.y0 >= 0.0 && bounds.y1 <= 720.0);
    let at = Pos2::new(
        bounds.x0 as f32 - 20.0,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    );
    bar_frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -20_000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    output = bar_frame(&ctx, &mut app, vec![]);
    assert!(
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.job.text == oldest
                && text.galley.rect.translate(text.pos.to_vec2()).intersects(shape.clip_rect))
        }),
        "The oldest full message must be reachable by scrolling."
    );
}

#[test]
fn a_cleared_error_stays_in_the_log_until_the_next_message() {
    let mut log = StatusLog::default();
    let now = Instant::now();
    log.observe("Cannot save.", true, true, now);
    log.clear();
    log.observe("Cannot save.", true, true, now);
    assert!(log.cleared, "The same error stays cleared.");
    assert_eq!(log.entries[0].text, "Cannot save.");
    log.observe("Saved.", false, true, now);
    assert!(!log.cleared, "A new message shows again.");
}

#[test]
fn the_mark_counts_errors_until_the_popup_shows_them() {
    let mut log = StatusLog::default();
    let now = Instant::now();
    log.observe("Cannot save.", true, true, now);
    log.observe("Saved.", false, true, now);
    log.renew();
    log.observe("Saved.", false, true, now);
    log.observe("Cannot open.", true, true, now);
    assert_eq!(log.unread_errors(false), 2, "Only errors count.");
    assert_eq!(
        log.unread_errors(true),
        1,
        "The error on the line in Coral does not count yet."
    );
    log.mark_read();
    assert_eq!(log.unread_errors(false), 0);
    log.observe("Cannot export.", true, true, now);
    assert_eq!(log.unread_errors(false), 1, "A new error counts again.");
}

#[test]
fn the_log_times_each_new_message_and_skips_the_idle_text() {
    let mut log = StatusLog::default();
    let start = Instant::now();
    assert_eq!(log.observe(DEFAULT_STATUS, false, false, start), None);
    assert_eq!(
        log.observe("Saved.", false, true, start),
        Some(Duration::ZERO)
    );
    let later = start + Duration::from_secs(9);
    assert_eq!(
        log.observe("Saved.", false, true, later),
        Some(Duration::from_secs(9)),
        "The same message keeps its age."
    );
    log.renew();
    assert_eq!(
        log.observe("Saved.", false, true, later),
        Some(Duration::ZERO),
        "A message set again is new, with the same text."
    );
    assert_eq!(
        log.observe("Saved.", true, true, later),
        Some(Duration::ZERO)
    );
    assert_eq!(log.entries.len(), 3);
    assert_eq!(log.observe(DEFAULT_STATUS, false, false, later), None);
    for index in 0..20 {
        log.observe(&format!("Message {index}."), false, true, later);
    }
    assert_eq!(log.entries.len(), STATUS_HISTORY);
    assert_eq!(log.entries[0].text, "Message 19.");
}

#[test]
fn error_code_text_inside_a_selected_filename_stays_intact() {
    for message in [
        "Cannot open sound (os error 5).wav. Access is denied. (os error 5)",
        "Cannot open C:\\sounds (os error 5)\\sound.wav. Access is denied. (os error 5)",
        "Saved C:\\Songs\\sound (os error 5) copy.wav and added it to Wave.",
        "Saved C:\\Songs\\sound (os error 5)",
        "Keep (os error x) as is.",
    ] {
        assert_eq!(status_text(message), message);
    }
}

#[test]
fn status_paths_drop_the_long_path_prefix_without_changing_the_filename() {
    assert_eq!(
        status_text(r"Saved \\?\C:\Songs\sound (os error 5) copy.wav and added it to Wave."),
        r"Saved C:\Songs\sound (os error 5) copy.wav and added it to Wave."
    );
}

#[test]
fn ages_read_in_their_largest_whole_unit() {
    assert_eq!(age_text(Duration::from_secs(2)), "Just now");
    assert_eq!(age_text(Duration::from_secs(42)), "42 s ago");
    assert_eq!(age_text(Duration::from_secs(185)), "3 min ago");
    assert_eq!(age_text(Duration::from_secs(7300)), "2 h ago");
}
