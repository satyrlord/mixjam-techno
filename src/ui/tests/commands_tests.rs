//! Command list: the Ctrl+K list, its filter, its rows and the command registry.

use super::*;

#[test]
fn every_rail_tool_panel_switch_and_lane_is_a_command() {
    use commands::{Command, PANELS, RAIL};
    use mixjam_techno::application::automation::Lane;
    let all = Command::all();
    // A rail tool exists only as a registered command with its icon.
    for (_, tools) in RAIL {
        for tool in tools {
            assert!(all.contains(tool), "{tool:?}");
            assert!(tool.icon().is_some(), "{tool:?} needs its rail icon");
        }
    }
    // Every rail icon but the top bar's recording destination is a command.
    for icon in RailIcon::ALL {
        if icon == RailIcon::Record {
            continue;
        }
        assert!(
            all.iter().any(|command| command.icon() == Some(icon)),
            "{} is not registered",
            icon.label()
        );
    }
    for panel in PANELS {
        assert!(all.contains(&panel), "{panel:?}");
    }
    let lanes: Vec<_> = all
        .iter()
        .filter(|command| matches!(command, Command::Lane(..)))
        .collect();
    assert_eq!(lanes.len(), 15, "Off and the 14 lanes");
    for lane in Lane::ALL {
        assert!(all.contains(&Command::Lane(Some(lane), None)), "{lane:?}");
    }
    for bus in 0..mixjam_techno::application::fx::BUSES {
        assert!(all.contains(&Command::Lane(None, Some(bus))));
    }
    let mut names: Vec<_> = all.iter().map(|command| command.name()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), all.len(), "Each command has its own name");
}

#[test]
fn ctrl_k_lists_every_command_in_groups_filters_and_runs_on_enter() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::K, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.commands.is_some());
    let labels = |output: &egui::FullOutput| -> Vec<String> {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .filter_map(|(_, node)| node.label().or(node.value()).map(str::to_owned))
            .collect()
    };
    let shown = labels(&output);
    // Everything shows before anything is typed, under its group.
    for name in [
        "FILE",
        "New",
        "TOOLS",
        "Album Review",
        "APPLICATION",
        "PANELS",
        "Sounds panel",
        "AUTOMATION",
        "Off: edit clips",
        "Repeat gate lane",
        "FX 4 send lane",
        "FX 4 return lane",
    ] {
        assert!(shown.iter().any(|label| label == name), "{name}: {shown:?}");
    }
    // Typing narrows the list; Enter runs the first match and closes it.
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("fx 2 return".into())],
        egui::Modifiers::NONE,
    );
    let shown = labels(&frame(&ctx, &mut app, vec![], egui::Modifiers::NONE));
    assert!(shown.iter().any(|label| label == "FX 2 return lane"));
    assert!(
        !shown.iter().any(|label| label == "Sounds panel"),
        "{shown:?}"
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.commands.is_none());
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (None, Some(1))
    );
    // Arrows move through the list; Esc closes it without running.
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::K, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("lane".into())],
        egui::Modifiers::NONE,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    // The first lane match is the Volume lane, the next Pan.
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (Some(Lane::Pan), None)
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::K, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.commands.is_none());
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (Some(Lane::Pan), None)
    );
}

#[test]
fn command_list_filters_before_enter_in_the_same_frame() {
    let (ctx, mut app) = fixture();
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let overview = app.session.layout().overview;
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::Text("song overview".into()),
            key_event(egui::Key::Enter, egui::Modifiers::NONE),
        ],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.layout().overview, !overview);
    assert!(app.commands.is_none());
    assert!(app.files.pending_action.is_none());
}

#[test]
fn tab_never_focuses_a_command_row() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let names: Vec<String> = commands::Command::all()
        .into_iter()
        .map(commands::Command::name)
        .collect();
    for _ in 0..4 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        let focused = update
            .nodes
            .iter()
            .find(|(id, _)| *id == update.focus)
            .and_then(|(_, node)| node.label().map(str::to_owned));
        assert!(
            !focused.as_ref().is_some_and(|label| names.contains(label)),
            "Tab focused the row {focused:?}, which Enter would not run"
        );
    }
    assert!(app.commands.is_some());
}

#[test]
fn command_list_search_reveals_the_first_match_after_scrolling() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for _ in 0..commands::Command::all().len() {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
    }
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("a".into())],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let nodes = agent::nodes(output.platform_output.accesskit_update.as_ref().unwrap());
    let search = nodes
        .iter()
        .find(|node| node.label == "Find a command")
        .unwrap();
    assert_eq!(search.text.as_deref(), Some("a"));
    let first = commands::matching("a")[0].name();
    let shape = output
        .shapes
        .iter()
        .find(|shape| match &shape.shape {
            egui::Shape::Text(text) => text.galley.job.text == first && text.pos.x > 300.0,
            _ => false,
        })
        .expect("The first match must be painted");
    let egui::Shape::Text(text) = &shape.shape else {
        unreachable!()
    };
    assert!(
        shape
            .clip_rect
            .contains(text.pos + text.galley.size() / 2.0),
        "The match at {:?} must be inside {:?}",
        text.pos,
        shape.clip_rect
    );
}

#[test]
fn every_command_row_says_what_it_does_after_its_name() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for command in commands::Command::all() {
        let text = command.description();
        assert!(!text.is_empty(), "{command:?} needs a description");
        assert!(!text.ends_with('.'), "{text}: one line, no full stop");
    }
    // The row paints the description after the name and gives it to the
    // screen reader; the accessible name stays the command's name.
    let painted = |wanted: &str| {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == wanted => Some(text.pos.x),
            _ => None,
        })
    };
    let name = painted("New").expect("New is painted");
    let description = painted("Start an empty song").expect("its description is painted");
    assert!(description > name, "The description follows the name");
    // The rail's New tool shares the name and describes itself with its
    // hint, so the row is the one in the command list's window.
    let nodes = agent::nodes(output.platform_output.accesskit_update.as_ref().unwrap());
    let described = |window: Option<&str>| {
        nodes
            .iter()
            .find(|node| node.label == "New" && node.window.as_deref() == window)
            .and_then(|node| node.description.as_deref())
    };
    assert_eq!(
        described(Some("All commands")),
        Some("Start an empty song"),
        "The New row carries its description"
    );
    assert_eq!(
        described(None),
        Some(RailIcon::New.hint()),
        "The rail's New tool keeps its own hint"
    );
}

#[test]
fn command_list_registers_its_modal_and_rejects_song_drops() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.blocking_modal(&ctx), Some("All commands"));
    let nodes = agent::nodes(output.platform_output.accesskit_update.as_ref().unwrap());
    for name in ["Find a command", "New", "Song overview"] {
        assert!(
            nodes.iter().any(|node| {
                node.label == name && node.window.as_deref() == Some("All commands")
            }),
            "{name} must belong to the command modal"
        );
    }
    let _ = ctx.run(
        egui::RawInput {
            dropped_files: vec![egui::DroppedFile {
                path: Some(PathBuf::from("blocked.mixjam")),
                ..Default::default()
            }],
            ..Default::default()
        },
        |ctx| app.file_drop(ctx),
    );
    assert!(app.files.pending_action.is_none());
    assert!(app.commands.is_some());
}

#[test]
fn command_list_yields_to_a_pending_close_confirmation() {
    let (ctx, mut app) = fixture();
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("song overview".into())],
        egui::Modifiers::NONE,
    );
    app.files.pending_action = Some(PendingAction::Close);
    let overview = app.session.layout().overview;
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.layout().overview, overview);
    assert!(app.commands.is_none());
    assert_eq!(app.blocking_modal(&ctx), Some("Save your changes?"));
}

#[test]
fn command_list_blocks_over_a_collapsed_path_prompt() {
    let (ctx, mut app) = fixture();
    app.files.path_prompt = Some(PathPrompt {
        request: FileRequest::SaveSong,
        open_token: None,
        text: String::new(),
        error: None,
        focused: false,
    });
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new(("hardware-collapsed", "Choose path")), true);
    });
    app.commands = Some(commands::CommandList::new());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.blocking_modal(&ctx), Some("All commands"));
    assert!(app.files.path_prompt.is_some());
}

#[test]
fn the_rail_opens_the_command_list_and_a_command_runs_its_tool() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    click_named(&ctx, &mut app, "All commands");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.commands.is_some());
    let names = app.session.layout().tool_names;
    click_named(&ctx, &mut app, "Tool names in the rail");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.commands.is_none(), "Running a command closes the list");
    assert_eq!(
        app.session.layout().tool_names,
        !names,
        "The list runs the status bar's Tools switch"
    );
}
