//! Dialogs and utility windows: hardware-style windows, blocking dialogs, open paths,
//! file drops and replacement guards.

use super::*;

#[test]
fn unsaved_mix_requires_a_pending_action_before_replacement() {
    let (ctx, mut app) = fixture();
    app.session.edit(EditCommand::SetTempo(160.0)).unwrap();
    app.request_action(PendingAction::New, &ctx);
    assert!(matches!(app.files.pending_action, Some(PendingAction::New)));
    assert_eq!(app.session.project().clips.len(), 2);
    assert!(!app.files.allow_close);
}

#[test]
fn choosing_an_open_path_preserves_edits_made_while_the_chooser_was_collapsed() {
    for master_only in [false, true] {
        let (context, mut app, port) = agent_fixture();
        let folder = std::env::temp_dir().join(format!(
            "mixjam-stale-open-{}-{master_only}",
            std::process::id()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let song = folder.join("replacement.mixjam");
        let mut replacement =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        replacement.save(&song).unwrap();
        app.session = app.session.with_settings_root(folder.join("settings"));
        let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
        assert_eq!(
            send(&mut app, &["click", "target=Open"])["modal"],
            "Choose path"
        );
        send(&mut app, &["click", "target=Collapse Choose path"]);
        assert!(dialog::is_collapsed(&context, "Choose path"));
        if master_only {
            let mut master = app.session.project().master;
            master.bypass = !master.bypass;
            app.session.edit(EditCommand::SetMaster(master)).unwrap();
        } else {
            app.session.edit(EditCommand::SetTempo(160.0)).unwrap();
        }
        let edited = app.session.project().clone();
        send(&mut app, &["click", "target=Expand Choose path"]);
        let draft = song.display().to_string();
        send(&mut app, &["set", "target=Path", &format!("value={draft}")]);
        send(&mut app, &["click", "target=Confirm path"]);
        assert!(
            app.session.project() == &edited,
            "Open must retain edits made after its path choice started."
        );
        let prompt = app
            .files
            .path_prompt
            .as_ref()
            .expect("Keep the path draft.");
        assert_eq!(prompt.text, draft);
        assert!(prompt.error.as_deref().unwrap().contains("changed"));
        assert!(app.session.unsaved());
        std::fs::remove_dir_all(folder).unwrap();
    }
}

#[test]
fn choosing_an_open_path_keeps_the_approval_for_an_unchanged_dirty_song() {
    let (context, mut app, port) = agent_fixture();
    let folder = std::env::temp_dir().join(format!("mixjam-approved-open-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let song = folder.join("replacement.mixjam");
    let mut replacement =
        Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    replacement.save(&song).unwrap();
    app.session = app.session.with_settings_root(folder.join("settings"));
    app.session.edit(EditCommand::SetTempo(160.0)).unwrap();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    assert_eq!(
        send(&mut app, &["click", "target=Open"])["modal"],
        "Save your changes?"
    );
    assert_eq!(
        send(&mut app, &["click", "target=Discard changes"])["modal"],
        "Choose path"
    );
    send(
        &mut app,
        &["set", "target=Path", &format!("value={}", song.display())],
    );
    send(&mut app, &["click", "target=Confirm path"]);
    assert_eq!(app.session.project().bpm, 140.0);
    assert!(app.session.project().clips.is_empty());
    assert_eq!(app.session.file(), Some(song.as_path()));
    assert!(app.files.path_prompt.is_none());
    assert!(!app.session.unsaved());
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn a_mixer_gain_draft_cannot_change_a_replacement_song() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Master gain value");
    let before = app.session.project().master_gain;
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("+6".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(app.session.project().master_gain, before);
    app.new_mix();
    let replacement = app.session.project().clone();
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session.project() == &replacement,
        "A Mixer gain draft must not commit to the replacement song."
    );
    assert!(!app.session.unsaved());
}

#[test]
fn a_mixer_drag_cannot_change_a_replacement_song() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let gain = effects_control_rect(&output, "Master gain");
    let start = gain.center();
    pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(start - Vec2::new(0.0, 12.0))],
        egui::Modifiers::NONE,
    );
    app.new_mix();
    let replacement = app.session.project().clone();
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(start - Vec2::new(0.0, 30.0))],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session.project() == &replacement,
        "A held Mixer control must not edit the replacement song."
    );
    assert!(!app.session.unsaved());
}

#[test]
fn hardware_utility_windows_keep_identity_keyboard_close_and_scoped_style() {
    for title in [
        "Help",
        "Time Stretcher help",
        "Effect Studio help",
        "Step synth help",
        "Sample editor help",
        "Options",
        "Import details",
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        let original_style = ctx.style();
        match title {
            "Help" => app.help.main = true,
            "Time Stretcher help" => app.help.stretch = true,
            "Effect Studio help" => app.help.effects = true,
            "Step synth help" => app.help.step_synth = true,
            "Sample editor help" => app.help.sample_edit = true,
            "Options" => app.open_sample_libraries(&ctx),
            "Import details" => app.sample_import.details_open = true,
            _ => unreachable!(),
        }
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let uppercase = title_text(title);
        let identity = accessible_node(&output, &uppercase).0;
        assert!(
            ctx.memory(|memory| memory.area_rect(egui::Id::new(title)))
                .is_some(),
            "{title} must keep the original mixed-case native area ID."
        );
        let next = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            accessible_node(&next, &uppercase).0,
            identity,
            "{title} must retain its native window identity between frames."
        );
        assert_eq!(ctx.style().text_styles, original_style.text_styles);
        assert_eq!(
            ctx.style().visuals.widgets.inactive.bg_fill,
            original_style.visuals.widgets.inactive.bg_fill,
            "The hardware body theme must not change the application theme."
        );
        focus_control(&ctx, &mut app, &next, &format!("Close {title}"));
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(
            !match title {
                "Help" => app.help.main,
                "Time Stretcher help" => app.help.stretch,
                "Effect Studio help" => app.help.effects,
                "Step synth help" => app.help.step_synth,
                "Sample editor help" => app.help.sample_edit,
                "Options" => app.options.is_some(),
                "Import details" => app.sample_import.details_open,
                _ => unreachable!(),
            },
            "{title} must keep the keyboard-accessible native X action."
        );
    }
}

#[test]
fn hardware_blocking_dialogs_keep_named_identity_and_safe_explicit_cancel() {
    for identity in ["Save your changes?", "Import sound", "Choose path"] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        let project = app.session.project().clone();
        let unsaved = app.session.unsaved();
        match identity {
            "Save your changes?" => app.files.pending_action = Some(PendingAction::New),
            "Import sound" => app.sample_import.path = Some(PathBuf::from(r"C:\Sounds\unread.wav")),
            "Choose path" => {
                app.files.path_prompt = Some(PathPrompt {
                    request: FileRequest::SaveSong,
                    open_token: None,
                    text: String::new(),
                    error: None,
                    focused: false,
                })
            }
            _ => unreachable!(),
        }
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            accessible_node(&output, identity).1.role(),
            egui::accesskit::Role::Dialog,
            "Visible title formatting must preserve the exact agent modal identity."
        );
        if identity == "Choose path" {
            assert_eq!(
                output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .focus,
                accessible_node(&output, "Path").0,
                "The typed-path dialog must initially focus its path field."
            );
        }
        focus_control(&ctx, &mut app, &output, &format!("Collapse {identity}"));
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(dialog::is_collapsed(&ctx, identity));
        assert!(
            ctx.memory(|memory| memory.top_modal_layer()).is_none(),
            "Collapsed pending action must release workspace."
        );
        assert!(match identity {
            "Save your changes?" => app.files.pending_action.is_some(),
            "Import sound" => app.sample_import.path.is_some(),
            _ => app.files.path_prompt.is_some(),
        });
        assert_eq!(app.session.project(), &project);
        focus_control(&ctx, &mut app, &output, &format!("Expand {identity}"));
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Cancel");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(
            app.files.pending_action.is_none()
                && app.sample_import.path.is_none()
                && app.files.path_prompt.is_none(),
            "Cancel must release the {identity} modal."
        );
        assert_eq!(app.session.project(), &project);
        assert_eq!(
            app.session.unsaved(),
            unsaved,
            "Cancel must not mutate the song or its saved state."
        );
    }
}

#[test]
fn hardware_utility_short_and_tall_dialogs_fit_and_keep_content_reachable_in_every_theme() {
    let size = Vec2::new(960.0, 720.0);
    let viewport = Rect::from_min_size(Pos2::ZERO, size);
    for density in themed_densities() {
        for tall in [false, true] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            let mut open = true;
            let mut draw = |events: Vec<egui::Event>| {
                ctx.run(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ctx| {
                        dialog::window("Utility fixture")
                            .open(&mut open)
                            .default_width(440.0)
                            .max_height(500.0)
                            .constrain_to(viewport.shrink(12.0))
                            .show(ctx, |ui| {
                                ui.label("First instruction");
                                if tall {
                                    for row in 0..60 {
                                        ui.label(format!(
                                            "Diagnostic row {row}: source details remain \
                                             readable."
                                        ));
                                    }
                                }
                                let _ = ui.button("Last action");
                            });
                    },
                )
            };
            let mut output = egui::FullOutput::default();
            for _ in 0..4 {
                output = draw(vec![]);
            }
            assert_folded_assets(&ctx, &output, density, "Utility");
            assert!(
                viewport.contains_rect(text_rect(&output, "UTILITY FIXTURE")),
                "The title must fit."
            );
            let first = text_rect(&output, "First instruction");
            assert!(
                viewport.contains_rect(first),
                "The first instruction must fit."
            );
            let visible = |output: &egui::FullOutput| {
                output.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Last action" => {
                        let bounds = Rect::from_min_size(text.pos, text.galley.size());
                        viewport.contains_rect(bounds) && shape.clip_rect.contains_rect(bounds)
                    }
                    _ => false,
                })
            };
            for _ in 0..30 {
                if visible(&output) {
                    break;
                }
                output = draw(vec![
                    egui::Event::PointerMoved(first.center()),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -160.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                for _ in 0..12 {
                    output = draw(vec![]);
                }
            }
            assert!(
                visible(&output),
                "The last action must remain reachable in a {tall:?} dialog."
            );
            assert!(
                viewport.contains_rect(text_rect(&output, "UTILITY FIXTURE")),
                "Scrolling body content must not move the title out of view."
            );
        }
    }
}

#[test]
fn resized_musical_content_keeps_nested_grid_effects_and_status_inside_the_chassis() {
    for (tool, title, status) in [
        (
            "effects",
            "EFFECT STUDIO",
            "Listen before saving. Save sound adds a new WAV to Effect.",
        ),
        (
            "stretch",
            "TIME STRETCHER",
            "Same tempo. Duration is unchanged.",
        ),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| {
            style.animation_time = 0.0;
            style.scroll_animation = egui::style::ScrollAnimation::none();
        });
        app.selection.sample = Some(1);
        match tool {
            "effects" => {
                app.effects.open = true;
                app.effects.state.filter = Some(Default::default());
            }
            "stretch" => app.stretch.open = true,
            _ => unreachable!(),
        }
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let before = effects_control_rect(&output, title);
        let grip = before.right_bottom() - Vec2::splat(3.0);
        pointer(&ctx, &mut app, grip, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(grip - Vec2::new(200.0, 150.0))],
            egui::Modifiers::NONE,
        );
        pointer(
            &ctx,
            &mut app,
            grip - Vec2::new(200.0, 150.0),
            false,
            egui::Modifiers::NONE,
        );
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let window = effects_control_rect(&output, title);
        assert!(
            window.width() < before.width() - 100.0 && window.height() < before.height() - 100.0,
            "Resize must shrink {tool}."
        );
        let body = Rect::from_min_max(
            window.min + Vec2::new(12.0, 56.0),
            window.max - Vec2::splat(12.0),
        );
        for phase in 0..2 {
            let mut found = 0;
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let label = text.galley.job.text.as_str();
                    if shape.clip_rect.is_positive()
                        && [
                            "Cutoff",
                            "Filter",
                            "Sequence effects",
                            "Pitch",
                            "Original BPM",
                            status,
                        ]
                        .contains(&label)
                    {
                        found += 1;
                        assert!(
                            body.expand(0.1).contains_rect(shape.clip_rect),
                            "{tool} {label} clip must stay in body after resize and scroll: {:?}/{body:?}",
                            shape.clip_rect
                        );
                    }
                }
            }
            assert!(found > 0, "Expected musical body paint in phase {phase}.");
            if phase == 0 {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(body.left_top() + Vec2::splat(4.0)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, -1200.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    egui::Modifiers::NONE,
                );
                for _ in 0..5 {
                    output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                }
            }
        }
        let message = output
            .shapes
            .iter()
            .find(|shape| {
                matches!(
                    &shape.shape,
                    egui::Shape::Text(text) if text.galley.job.text == status
                )
            })
            .expect("Scrolling the shell must reach the status footer.");
        assert!(body.intersects(message.clip_rect));
    }
}

#[test]
fn a_hovering_file_says_whether_it_opens_as_a_song() {
    let (ctx, mut app) = fixture();
    let hover = |app: &mut MixJamApp, name: &str| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
            hovered_files: vec![egui::HoveredFile {
                path: Some(PathBuf::from(name)),
                mime: String::new(),
            }],
            focused: true,
            ..Default::default()
        };
        ctx.run(input, |ctx| app.draw(ctx))
    };
    let output = hover(&mut app, "song.mixjam");
    assert!(shows_text(&output, "Drop to open the song"));
    let output = hover(&mut app, "notes.txt");
    assert!(shows_text(&output, "Only song files open here"));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!shows_text(&output, "Drop to open the song"));
    assert!(!shows_text(&output, "Only song files open here"));
}
