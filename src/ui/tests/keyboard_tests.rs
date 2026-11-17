//! Keyboard focus and shortcuts: tab order, focus scrolling and shortcuts beside
//! focused controls.

use super::*;

/// The accessible name of the focused control, if any.
fn focused_label(output: &egui::FullOutput) -> Option<String> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == update.focus)
        .and_then(|(_, node)| node.label().map(str::to_owned))
}

#[test]
fn tab_walks_the_rail_top_to_bottom_and_reaches_the_panel_switches_last() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let first = TRANSPORT_BUTTONS[0];
    focus_control(&ctx, &mut app, &output, first);
    let mut order = vec![first.to_owned()];
    for _ in 0..400 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let Some(label) = focused_label(&output) else {
            continue;
        };
        if label == first {
            break;
        }
        order.push(label);
    }
    // The top bar, left to right: the transport, the recording track, the
    // tempo, then Help (Undo and Redo are disabled in the fixture).
    let top_bar: Vec<&str> = TRANSPORT_BUTTONS
        .into_iter()
        .chain(["Recording track", "Project BPM", "Help"])
        .collect();
    assert_eq!(
        &order[..top_bar.len()],
        top_bar.as_slice(),
        "The top bar comes first: {order:?}"
    );
    let rail = [
        "Shrink the tool rail to icons",
        "New",
        "Open",
        "Save",
        "Save as",
        "Import",
        "Export",
        "Erase",
        "Delete",
        "Mixer",
        "Mastering",
        "FX returns",
        "Step synth",
        "Edit",
        "Stretch",
        "Effects",
        "Album Review",
    ];
    let after = top_bar.len();
    assert_eq!(
        &order[after..after + rail.len()],
        rail.as_slice(),
        "The rail follows the top bar, top to bottom: {order:?}"
    );
    assert_eq!(
        order.get(after + rail.len()).map(String::as_str),
        Some("Export tracks"),
        "The lower panel follows the rail: {order:?}"
    );
    // The song overview leads the arrangement, then the score toolbar,
    // left to right.
    let toolbar = [
        "Song overview",
        "Hide the song overview",
        "Arrange view",
        "Mix view",
        "Automation lane",
        "Follow",
        "Sections",
        "Zoom",
        "Maximise the score",
        "Arrangement lanes",
    ];
    let at = order
        .iter()
        .position(|name| name == toolbar[0])
        .unwrap_or_else(|| panic!("The score toolbar is reachable: {order:?}"));
    assert_eq!(
        &order[at..at + toolbar.len()],
        toolbar.as_slice(),
        "{order:?}"
    );
    // The status bar leads with Options and All commands.
    let options = order.iter().position(|name| name == "Options");
    assert!(
        options.is_some()
            && order.get(options.unwrap() + 1).map(String::as_str) == Some("All commands")
            && options < order.iter().position(|name| name == "Tools"),
        "Options and All commands lead the status bar: {order:?}"
    );
    let tools = order.iter().position(|name| name == "Tools");
    let boards = order.iter().position(|name| name == "Boards");
    let sounds = order.iter().position(|name| name == "Sounds");
    let overview = order.iter().position(|name| name == "Overview");
    let inspector = order.iter().position(|name| name == "Inspector");
    assert!(
        tools.is_some()
            && boards == tools.map(|at| at + 1)
            && sounds == tools.map(|at| at + 2)
            && overview == tools.map(|at| at + 3)
            && inspector == tools.map(|at| at + 4),
        "The panel switches come together: {order:?}"
    );
    assert!(
        inspector.is_some_and(|at| at + 2 >= order.len()),
        "Only the version link may follow the panel switches: {order:?}"
    );
}

/// The fixture's library with 40 more sounds after its own, so the sound
/// list has rows to move among and to scroll.
fn with_more_sounds(app: &mut MixJamApp) {
    let library = &app.session.library().samples;
    let base = library[0].clone();
    let mut samples = library.to_vec();
    samples.extend((0..40).map(|index| {
        let mut sample = base.clone();
        sample.id = 1000 + index;
        sample.name = format!("Extra sound {index}");
        sample
    }));
    app.session = Session::headless(
        Arc::new(Library::fixture(samples)),
        app.session.project().clone(),
    )
    .unwrap();
}

#[test]
fn every_tab_stop_has_a_name_and_the_sound_list_is_one_stop() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let rows = app.session.library().samples.len();
    assert!(rows > 1, "The fixture lists more than one sound.");
    let first = TRANSPORT_BUTTONS[0];
    focus_control(&ctx, &mut app, &output, first);
    let focus = |output: &egui::FullOutput| {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .map(|update| update.focus)
    };
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let start = focus(&output);
    let mut stops = Vec::new();
    for _ in 0..400 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        if focus(&output) == start {
            break;
        }
        stops.push(focused_label(&output));
    }
    let unnamed: Vec<String> = stops
        .iter()
        .enumerate()
        .filter(|(_, name)| name.is_none())
        .map(|(at, _)| {
            let before = stops[..at].iter().rev().flatten().next();
            format!("after {before:?}")
        })
        .collect();
    assert!(unnamed.is_empty(), "Unnamed Tab stops: {unnamed:?}");
    let names: Vec<&str> = stops.iter().flatten().map(String::as_str).collect();
    assert_eq!(
        names.iter().filter(|name| **name == "Sound list").count(),
        1,
        "The sound list is one Tab stop: {names:?}"
    );
    // The lanes stay a short Tab walk from the list, whatever its length.
    let list = names.iter().position(|name| *name == "Sound list");
    let lanes = names.iter().position(|name| *name == "Arrangement lanes");
    assert!(
        list.zip(lanes)
            .is_some_and(|(list, lanes)| lanes > list && lanes - list <= 12),
        "The lanes follow the sound list closely: {names:?}"
    );
    let library = app.session.library();
    assert!(
        !library
            .samples
            .iter()
            .any(|sample| names.contains(&sample.name.as_str())),
        "No sound row is a Tab stop of its own: {names:?}"
    );
}

/// Each Tab stop after the first transport button, in order, until the walk
/// returns to that button: its accessible name, or its role when it has none.
fn tab_stops(ctx: &egui::Context, app: &mut MixJamApp, size: Vec2) -> Vec<Result<String, String>> {
    let quiet = |app: &mut MixJamApp| frame_at_size(ctx, app, vec![], egui::Modifiers::NONE, size);
    for _ in 0..4 {
        quiet(app);
    }
    let output = quiet(app);
    let (target, _) = accessible_node(&output, TRANSPORT_BUTTONS[0]);
    frame_at_size(
        ctx,
        app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
        size,
    );
    let focused = |output: &egui::FullOutput| {
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        let node = update.nodes.iter().find(|(id, _)| *id == update.focus);
        (
            update.focus,
            node.map(|(_, node)| {
                node.label()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{:?}", node.role()))
            }),
        )
    };
    let (start, _) = focused(&quiet(app));
    let mut stops = Vec::new();
    for _ in 0..600 {
        frame_at_size(
            ctx,
            app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            size,
        );
        let (at, stop) = focused(&quiet(app));
        if at == start {
            return stops;
        }
        stops.push(stop.unwrap_or_else(|| Err("no node".into())));
    }
    panic!("The Tab walk does not return to its start: {stops:?}");
}

#[test]
fn every_tab_stop_of_every_board_has_a_name() {
    // The short window makes the boards scroll, so their scroll areas show.
    for size in [Vec2::new(1440.0, 1000.0), Vec2::new(1024.0, 768.0)] {
        for module in BottomModule::ALL {
            for mix in [false, true] {
                let (ctx, mut app) = fixture();
                ctx.enable_accesskit();
                app.boards.module = module;
                app.set_mix_view(mix);
                let stops = tab_stops(&ctx, &mut app, size);
                let unnamed: Vec<String> = stops
                    .iter()
                    .enumerate()
                    .filter_map(|(at, stop)| {
                        let role = stop.as_ref().err()?;
                        let before = stops[..at].iter().rev().flatten().next();
                        Some(format!("{role} after {before:?}"))
                    })
                    .collect();
                assert!(
                    unnamed.is_empty(),
                    "Unnamed Tab stops on {} at {size:?}, Mix view {mix}: {unnamed:?}",
                    module.label()
                );
            }
        }
    }
}

/// One Tab cycle from no focus: each stop's accessible name, or its role
/// when it has none. The walk ends when a stop comes again.
fn tab_cycle(ctx: &egui::Context, app: &mut MixJamApp, size: Vec2) -> Vec<Result<String, String>> {
    let quiet = |app: &mut MixJamApp| frame_at_size(ctx, app, vec![], egui::Modifiers::NONE, size);
    for _ in 0..4 {
        quiet(app);
    }
    let mut seen = Vec::new();
    let mut stops = Vec::new();
    for _ in 0..900 {
        frame_at_size(
            ctx,
            app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            size,
        );
        let output = quiet(app);
        let Some(at) = ctx.memory(|memory| memory.focused()) else {
            stops.push(Err("no focus".into()));
            return stops;
        };
        if seen.contains(&at) {
            return stops;
        }
        seen.push(at);
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        let node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == update.focus)
            .map(|(_, node)| node);
        stops.push(match node {
            Some(node) => node
                .label()
                .map(str::to_owned)
                .ok_or_else(|| format!("{:?}", node.role())),
            None => Err("no node".into()),
        });
    }
    panic!("The Tab walk does not end: {stops:?}");
}

#[test]
fn every_tab_stop_of_every_window_modal_and_menu_has_a_name() {
    type Setup = Box<dyn Fn(&egui::Context, &mut MixJamApp)>;
    let options = |page: options::Page| -> Setup {
        Box::new(move |_, app| {
            app.options = Some(options::OptionsWindow::new(&app.session, page));
        })
    };
    let all_effects = || EffectsState {
        filter: Some(Default::default()),
        echo: Some(Default::default()),
        robotize: Some(Default::default()),
        ..Default::default()
    };
    // Each state, a stop that shows the walk reached it, and its setup.
    let states: Vec<(&str, &str, Setup)> = vec![
        (
            "Effect Studio and its help",
            "Close Effect Studio help",
            Box::new(move |_, app| {
                app.selection.sample = Some(1);
                app.effects.open = true;
                app.effects.state = all_effects();
                app.help.effects = true;
            }),
        ),
        (
            "Sample editor and its help",
            "Close Sample editor help",
            Box::new(|_, app| {
                app.selection.sample = Some(1);
                app.sample_edit.open = true;
                app.help.sample_edit = true;
            }),
        ),
        (
            "Time Stretcher and its help",
            "Close Time Stretcher help",
            Box::new(|_, app| {
                app.selection.sample = Some(1);
                app.stretch.open = true;
                app.help.stretch = true;
            }),
        ),
        (
            "Album Review",
            "Close Album Review",
            Box::new(|_, app| app.album_review.open = true),
        ),
        (
            "Album Review page",
            "← All albums",
            Box::new(|ctx, app| {
                app.album_review.open = true;
                frame(ctx, app, vec![], egui::Modifiers::NONE);
                let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
                let target = screen(&output)
                    .nodes()
                    .find(|node| {
                        node.node()
                            .label()
                            .is_some_and(|label| label.starts_with("Open review: "))
                    })
                    .expect("a review")
                    .id();
                frame(
                    ctx,
                    app,
                    vec![egui::Event::AccessKitActionRequest(
                        egui::accesskit::ActionRequest {
                            action: egui::accesskit::Action::Click,
                            target,
                            data: None,
                        },
                    )],
                    egui::Modifiers::NONE,
                );
            }),
        ),
        (
            "Options, Audio",
            "Playback buffer target",
            options(options::Page::Audio),
        ),
        (
            "Options, Workspace",
            "Starting module",
            options(options::Page::Workspace),
        ),
        (
            "Options, Files",
            "Song dialog folder",
            options(options::Page::Files),
        ),
        (
            "Sample libraries",
            "Add library folder",
            Box::new(|_, app| app.libraries.open = true),
        ),
        (
            "Discard changes?",
            "Keep editing",
            Box::new(|_, app| {
                let mut window =
                    options::OptionsWindow::new(&app.session, options::Page::Workspace);
                window.values.follow_on_startup = !window.values.follow_on_startup;
                window.confirm_discard = true;
                app.options = Some(window);
            }),
        ),
        (
            "Help",
            "Close Help",
            Box::new(|_, app| app.help.main = true),
        ),
        (
            "Step synth help",
            "Close Step synth help",
            Box::new(|_, app| {
                app.boards.module = BottomModule::StepSynth;
                app.help.step_synth = true;
            }),
        ),
        (
            "Mastering help",
            "Close Mastering help",
            Box::new(|_, app| {
                app.boards.module = BottomModule::Mastering;
                app.help.master = true;
            }),
        ),
        (
            "Step synth Effects pane",
            "Robotize volume",
            Box::new(move |_, app| {
                app.boards.module = BottomModule::StepSynth;
                app.step_synth.state.effects = all_effects();
                app.step_synth.effects_expanded = true;
            }),
        ),
        (
            "Channel EQ, sections off",
            "Close Track 1 EQ",
            Box::new(|_, app| app.eq_track = Some(0)),
        ),
        (
            "Channel EQ, sections on",
            "Track 1 ducker release value",
            Box::new(|_, app| {
                let mut tracks = app.session.project().tracks.clone();
                tracks[0].eq.on = true;
                tracks[0].drive.on = true;
                tracks[0].compressor.on = true;
                tracks[0].ducker.on = true;
                tracks[0].ducker.key = 2;
                let master_gain = app.session.project().master_gain;
                app.session
                    .edit(EditCommand::SetMixer {
                        tracks,
                        master_gain,
                    })
                    .unwrap();
                app.eq_track = Some(0);
            }),
        ),
        (
            "Import details",
            "Close Import details",
            Box::new(|_, app| app.sample_import.details_open = true),
        ),
        (
            "Import sound",
            "Import into Wave",
            Box::new(|_, app| {
                app.sample_import.path = Some(PathBuf::from(r"C:\Sounds\New sound.wav"));
            }),
        ),
        (
            "Save your changes?",
            "Save changes",
            Box::new(|_, app| app.files.pending_action = Some(PendingAction::New)),
        ),
        (
            "Choose path",
            "Confirm path",
            Box::new(|_, app| {
                app.files.path_prompt = Some(PathPrompt {
                    request: FileRequest::SaveSong,
                    open_token: None,
                    text: String::new(),
                    error: None,
                    focused: false,
                });
            }),
        ),
        (
            "All commands",
            "Find a command",
            Box::new(|_, app| app.commands = Some(commands::CommandList::new())),
        ),
        (
            "Clip menu",
            "Clip reverse",
            Box::new(|ctx, app| {
                let clip = app.session.project().clips[0].id;
                open_clip_menu(ctx, app, clip);
            }),
        ),
        (
            "Clip fade curve menu",
            "Menu 2",
            Box::new(|ctx, app| {
                let clip = app.session.project().clips[0].id;
                open_clip_menu(ctx, app, clip);
                click_named(ctx, app, "Clip fade in curve");
            }),
        ),
        (
            "Automation lane menu",
            "FX 4 return",
            Box::new(|ctx, app| {
                click_named(ctx, app, "Automation lane");
            }),
        ),
        (
            "Recording track list",
            "Menu vertical scroll bar",
            Box::new(|ctx, app| {
                click_named(ctx, app, "Recording track");
            }),
        ),
        (
            "Mastering preset list",
            "Menu drag to scroll",
            Box::new(|ctx, app| {
                app.boards.module = BottomModule::Mastering;
                frame(ctx, app, vec![], egui::Modifiers::NONE);
                click_named(ctx, app, "Mastering preset");
            }),
        ),
        (
            "Scale root list",
            "Menu",
            Box::new(|ctx, app| {
                app.boards.module = BottomModule::StepSynth;
                frame(ctx, app, vec![], egui::Modifiers::NONE);
                click_named(ctx, app, "Scale root");
            }),
        ),
        (
            "Theme list",
            "Menu",
            Box::new(|ctx, app| {
                app.options = Some(options::OptionsWindow::new(
                    &app.session,
                    options::Page::Workspace,
                ));
                click_named(ctx, app, "Theme");
            }),
        ),
    ];
    let mut faults = Vec::new();
    let mut check = |state: &str, size: Vec2, reached: &str, stops: &[Result<String, String>]| {
        let unnamed: Vec<_> = stops
            .iter()
            .filter_map(|stop| stop.as_ref().err())
            .collect();
        if !unnamed.is_empty() {
            faults.push(format!(
                "{state} at {size}: stops without a name: {unnamed:?}"
            ));
        }
        if !stops.iter().any(|stop| stop.as_deref() == Ok(reached)) {
            faults.push(format!("{state} at {size}: the walk has no {reached:?}"));
        }
    };
    for size in [Vec2::new(1440.0, 1000.0), Vec2::new(960.0, 720.0)] {
        for (state, reached, setup) in &states {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            setup(&ctx, &mut app);
            check(state, size, reached, &tab_cycle(&ctx, &mut app, size));
        }
        let (ctx, mut app, dir) = sample_conflicts_fixture("tab-stops");
        let stops = tab_cycle(&ctx, &mut app, size);
        std::fs::remove_dir_all(dir).unwrap();
        check("Sample conflicts", size, "Keep choices", &stops);
        let (ctx, mut app) = missing_fixture();
        ctx.enable_accesskit();
        app.sample_import.details_open = true;
        let stops = tab_cycle(&ctx, &mut app, size);
        check(
            "Import details, missing sound",
            size,
            "Close Import details",
            &stops,
        );
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn focused_sound_list_shows_its_focus_without_a_current_sound() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    app.selection.sample = None;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let marked = |output: &egui::FullOutput| {
        let list = effects_control_rect(output, "Sound list");
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(mark)
                if mark.stroke.color == roles().accent
                    && mark.stroke.width == 2.0
                    && list.expand(1.0).contains_rect(mark.rect))
        })
    };
    assert!(!marked(&output), "An unfocused list has no focus mark.");
    focus_control(&ctx, &mut app, &output, "Sound list");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(focused_label(&output).as_deref(), Some("Sound list"));
    assert!(
        marked(&output),
        "The focused list shows where the focus is before a sound is chosen."
    );
}

#[test]
fn short_lists_show_whole_and_do_not_scroll() {
    type Setup = fn(&egui::Context, &mut MixJamApp);
    let sounds: Setup = |_, app| {
        with_more_sounds(app);
        app.open_sounds_panel();
    };
    let step_synth: Setup = |_, app| app.boards.module = BottomModule::StepSynth;
    let providers: Setup = |ctx, app| {
        app.open_sample_libraries(ctx);
        frame(ctx, app, vec![], egui::Modifiers::NONE);
        click_named(ctx, app, "Optional product providers");
    };
    for (list, setup) in [
        ("Library filter", sounds),
        ("Scale", step_synth),
        ("Scale root", step_synth),
        ("Provider product", providers),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        setup(&ctx, &mut app);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        click_named(&ctx, &mut app, list);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let menu = effects_control_rect(&output, "Menu");
        assert!(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0)).contains_rect(menu),
            "The {list} list stays in the window: {menu:?}."
        );
        // egui adds the scroll parts only when the rows do not fit.
        let scrolls = screen(&output).nodes().any(|node| {
            node.node()
                .label()
                .is_some_and(|label| label.starts_with("Menu ") && label.contains("scroll"))
        });
        assert!(!scrolls, "The {list} list must show all its rows.");
    }
}

#[test]
fn focused_menu_surface_shows_a_focus_outline() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Automation lane");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // The outline goes round the whole surface, inside its edge.
    let marked = |output: &egui::FullOutput| {
        let menu = effects_control_rect(output, "Menu");
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(mark)
                if mark.stroke.color == roles().accent
                    && mark.stroke.width == 2.0
                    && menu.contains_rect(mark.rect)
                    && mark.rect.expand(2.0).contains_rect(menu))
        })
    };
    let tab = |app: &mut MixJamApp| {
        frame(
            &ctx,
            app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, app, vec![], egui::Modifiers::NONE)
    };
    assert!(!marked(&output), "An unfocused menu has no focus mark.");
    let output = tab(&mut app);
    assert_eq!(focused_label(&output).as_deref(), Some("Menu"));
    assert!(marked(&output), "The focused menu surface shows the focus.");
    let output = tab(&mut app);
    assert_eq!(focused_label(&output).as_deref(), Some("Off"));
    assert!(!marked(&output), "The mark goes when the focus moves on.");
}

#[test]
fn sound_list_keys_choose_sounds_and_enter_places_the_current_one() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    app.selection.sample = None;
    app.recording.track = 3;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Sound list");
    // The repaint that the list asks for when it gains focus.
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let press = |app: &mut MixJamApp, key: egui::Key| {
        frame(
            &ctx,
            app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, app, vec![], egui::Modifiers::NONE);
    };
    let (order, _) = app.palette_view();
    let id_at = |app: &MixJamApp, row: usize| app.session.library().samples[order[row]].id;
    press(&mut app, egui::Key::ArrowDown);
    assert_eq!(
        app.selection.sample,
        Some(id_at(&app, 0)),
        "Down starts at the first sound."
    );
    press(&mut app, egui::Key::ArrowDown);
    assert_eq!(app.selection.sample, Some(id_at(&app, 1)));
    press(&mut app, egui::Key::End);
    assert_eq!(app.selection.sample, Some(id_at(&app, order.len() - 1)));
    press(&mut app, egui::Key::Home);
    assert_eq!(app.selection.sample, Some(id_at(&app, 0)));
    let clips = app.session.project().clips.len();
    press(&mut app, egui::Key::Enter);
    assert_eq!(
        app.session.project().clips.len(),
        clips + 1,
        "Enter places the current sound."
    );
}

#[test]
fn delete_removes_the_selected_clips_while_the_sound_list_has_focus() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    let clip = app.session.project().clips[0].id;
    app.selection.clip = Some(clip);
    app.selection.clips = [clip].into_iter().collect();
    let count = app.session.project().clips.len();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Sound list");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(focused_label(&output).as_deref(), Some("Sound list"));
    let sound = app.selection.sample;
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Delete, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().clips.len(),
        count - 1,
        "Delete removes the selected clip while the list has focus."
    );
    assert!(app.selection.clips.is_empty());
    assert_eq!(app.selection.sample, sound, "The current sound stays.");
    assert_eq!(
        focused_label(&output).as_deref(),
        Some("Sound list"),
        "The list keeps the focus."
    );
}

#[test]
fn tab_scrolls_rail_controls_into_view_in_short_windows() {
    each_theme(|_| {
        for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0)] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            ctx.style_mut(|style| {
                style.scroll_animation = egui::style::ScrollAnimation::none();
            });
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let target = accessible_node(&output, "Shrink the tool rail to icons").0;
            frame_at_size(
                &ctx,
                &mut app,
                vec![egui::Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::Focus,
                        target,
                        data: None,
                    },
                )],
                egui::Modifiers::NONE,
                size,
            );
            let mut reached_last = false;
            for _ in 0..18 {
                frame_at_size(
                    &ctx,
                    &mut app,
                    vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
                    egui::Modifiers::NONE,
                    size,
                );
                // Tab resolves focus after drawing. The next frame requests
                // scrolling, and the following frame paints that offset.
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                let label = focused_label(&output).expect("A rail control must have focus.");
                let rect = effects_control_rect(&output, &label);
                // The rail ends at the 30-point status bar.
                assert!(
                    rect.top() >= 64.0 && rect.bottom() <= size.y - 30.0,
                    "Focused {label} must be visible at {size:?}: {rect:?}."
                );
                if label == "Album Review" {
                    reached_last = true;
                    break;
                }
            }
            assert!(reached_last);
        }
    });
}

#[test]
fn audio_capture_shortcut_works_in_a_modal_and_does_not_repeat() {
    let (ctx, mut app) = fixture();
    app.files.pending_action = Some(PendingAction::New);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.blocking_modal(&ctx), Some("Save your changes?"));
    app.session.take_notice();
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::F12, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let (message, error) = app
        .session
        .take_notice()
        .expect("F12 must reach the session.");
    assert!(error && message.contains("Audio output is unavailable"));
    assert_eq!(app.blocking_modal(&ctx), Some("Save your changes?"));
    let mut repeat = key_event(egui::Key::F12, egui::Modifiers::NONE);
    if let egui::Event::Key { repeat, .. } = &mut repeat {
        *repeat = true;
    }
    frame(&ctx, &mut app, vec![repeat], egui::Modifiers::NONE);
    assert!(app.session.take_notice().is_none());
}

#[test]
fn focused_controls_keep_command_shortcuts_and_only_typing_blocks_them() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.set_layout(|layout| layout.sounds = true);
    let before = app.session.project().bpm;
    let other = if before == 160.0 { 140.0 } else { 160.0 };
    app.session.edit(EditCommand::SetTempo(other)).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Stop");
    assert!(ctx.memory(|memory| memory.focused()).is_some());
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.session.project().bpm,
        before,
        "Undo must work while a button has keyboard focus."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Y, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(app.session.project().bpm, other, "Ctrl+Y must redo.");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Find a sound");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.session.project().bpm,
        other,
        "A text field that is being typed in keeps Ctrl+Z."
    );
}

#[test]
fn escape_ends_erase_while_its_rail_button_has_focus() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, RailIcon::Erase.label());
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.score_tools.erase,
        "Space on the focused Erase button starts erasing."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    // egui releases focus on Esc before the workspace reads the key.
    assert!(
        !app.score_tools.erase,
        "Esc must end erasing, as the Erase tooltip says."
    );
}

#[test]
fn step_grid_is_one_tab_stop_with_arrow_key_navigation() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let cells = screen(&output)
        .nodes()
        .filter(|node| {
            node.node().label().is_some_and(|label| {
                label.starts_with("Step ")
                    && (label.ends_with("semitones") || label.ends_with("semitone"))
            })
        })
        .count();
    // 25 pitch rows over one, two, or four bars, as the board's width allows.
    assert!(
        [400, 800, 1600].contains(&cells),
        "Every cell in view stays available to assistive technology: {cells}"
    );
    focus_control(&ctx, &mut app, &output, "Step 1, +0 semitones");
    // egui holds arrow keys for a control from its second focused frame.
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for key in [
        egui::Key::ArrowRight,
        egui::Key::ArrowRight,
        egui::Key::ArrowUp,
    ] {
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.step_synth.state.steps[2],
        Step::note(1),
        "Arrow keys move the cell cursor and Enter sets its note."
    );
    assert!(
        app.board_shown(BottomModule::StepSynth),
        "Arrow keys must stay inside the grid."
    );
}

#[test]
fn mixer_pointer_gain_edit_keeps_transport_and_history_shortcuts() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let rail = effects_control_rect(&output, "Track 1 gain");
    click(
        &ctx,
        &mut app,
        rail.center_bottom() - Vec2::new(0.0, 20.0),
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        app.session.project().tracks[0].gain < 1.0,
        "The rail click must lower the gain."
    );
    // A pointer edit leaves keyboard focus unchanged.
    assert!(
        !ctx.wants_keyboard_input(),
        "A pointer edit must not leave the rail holding keyboard focus."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.session.project().tracks[0].gain,
        1.0,
        "Undo must work directly after a pointer gain edit."
    );
}
