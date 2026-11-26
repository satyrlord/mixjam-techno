//! Tab order and accessible names: the Tab walk through the window, each board,
//! window, modal and menu, and the name of every stop.

use super::*;

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
        let Some(label) = focused_label(&output).map(str::to_owned) else {
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
        stops.push(focused_label(&output).map(str::to_owned));
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
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.boards.module = module;
            app.set_layout(|layout| layout.boards = true);
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
                "Unnamed Tab stops on {} at {size:?}: {unnamed:?}",
                module.label()
            );
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
