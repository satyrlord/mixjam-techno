//! Agent control channel: requests, windows, controls, arrangement commands and quit.

use super::*;

#[test]
fn agent_scopes_step_synth_toolbar_controls_with_its_board() {
    let (ctx, mut app, port) = agent_fixture();
    let (_, missing) = missing_fixture();
    app.session = missing.session;
    app.select_clip(1, false);
    app.boards.module = BottomModule::StepSynth;
    let reply = agent(
        &ctx,
        &mut app,
        &port,
        &["ui", "all=true", "window=Step synth"],
    );
    let nodes = reply["nodes"].as_array().unwrap();
    for name in [
        "Step synth status",
        "Step synth help",
        "Import details",
        "Step synth BPM",
    ] {
        assert!(nodes.iter().any(|node| node["label"] == name), "{name}");
    }
    let reply = agent(
        &ctx,
        &mut app,
        &port,
        &["click", "target=Step synth help", "window=Step synth"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.help.step_synth);
    app.help.step_synth = false;
    let reply = agent(
        &ctx,
        &mut app,
        &port,
        &["click", "target=Import details", "window=Step synth"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.sample_import.details_open);
}

#[test]
fn agent_rejected_library_request_preserves_fallback_tempo() {
    let (context, mut app, port) = agent_fixture();
    let fallback_bpm = app.libraries.fallback_bpm;
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set_libraries", "folders=[]", "fallback_bpm=160"],
    );
    assert_eq!(reply["ok"], false, "{reply}");
    assert!(app.session.wave_scan().is_none());
    assert_eq!(app.libraries.fallback_bpm, fallback_bpm);
    let reply = agent(&context, &mut app, &port, &["libraries"]);
    assert_eq!(reply["fallback_bpm"], fallback_bpm);
}

#[test]
fn agent_arrangement_requests_obey_blocking_dialogs_and_resume_when_collapsed() {
    let (context, mut app, port) = agent_fixture();
    app.files.pending_action = Some(PendingAction::New);
    let project = app.session.project().clone();
    for words in [
        &["place", "sample=1", "track=3", "bar=1"][..],
        &["move_clips", "clips=[1]", "bars=1"],
        &["delete_clips", "clips=[1]"],
        &["select", "clips=[1]"],
        &["seek", "bar=2"],
        &["set_loop", "first_bar=1", "last_bar=2"],
    ] {
        let reply = agent(&context, &mut app, &port, words);
        assert_eq!(reply["ok"], false, "{words:?}: {reply}");
        assert!(
            reply["error"]
                .as_str()
                .unwrap()
                .contains("Save your changes?"),
            "{reply}"
        );
    }
    assert_eq!(app.session.project(), &project);
    assert_eq!(app.session.cursor_tick(), 0);
    assert!(app.selection.clips.is_empty());
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["click", "target=Collapse Save your changes?"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["place", "sample=1", "track=3", "bar=1"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.session.project().clips.len(), project.clips.len() + 1);
}

#[test]
fn agent_move_rejects_combined_tick_overflow_without_changing_the_song() {
    let (context, mut app, port) = agent_fixture();
    let project = app.session.project().clone();
    for (ticks, bars) in [(i64::MAX, 1), (i64::MIN, -1)] {
        let reply = agent(
            &context,
            &mut app,
            &port,
            &[
                "move_clips",
                &format!("clips=[{}]", project.clips[0].id),
                &format!("ticks={ticks}"),
                &format!("bars={bars}"),
            ],
        );
        assert_eq!(reply["ok"], false, "{reply}");
        assert_eq!(app.session.project(), &project);
    }
}

#[test]
fn agent_selects_radio_buttons_and_rejects_clearing_the_active_choice() {
    let (context, mut app, port) = agent_fixture();
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Record on track 2", "value=true"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.recording.track, 1);
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Record on track 2", "value=true"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Record on track 1", "value=false"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Record on track 2", "value=false"],
    );
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(app.recording.track, 1);
}

#[test]
fn agent_operates_windows_sliders_toggles_text_and_combo_boxes_by_name() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    send(&mut app, &["click", "target=Show the Sounds panel"]);

    let reply = send(&mut app, &["ui"]);
    let nodes = reply["nodes"].as_array().unwrap();
    for (label, role) in [
        ("Play / pause (Space)", "button"),
        ("Project BPM", "slider"),
        ("Mixer tab", "button"),
        ("Zoom", "slider"),
        ("Find a sound", "text_input"),
        ("Library filter", "combo_box"),
        ("Category filter", "combo_box"),
        ("Sort order", "combo_box"),
        ("Recording track", "combo_box"),
        ("Record on track 1", "radio"),
        ("Arrangement lanes", "other"),
        ("Bar ruler", "other"),
        ("good grasp 2", "button"),
    ] {
        assert!(
            nodes
                .iter()
                .any(|node| node["label"] == label && node["role"] == role),
            "Missing {role} {label}"
        );
    }
    assert!(
        nodes
            .iter()
            .all(|node| node["role"] != "label" && node["enabled"] == true)
    );
    let reply = send(&mut app, &["ui", "all=true", "contains=ui fixture"]);
    assert!(
        reply["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["role"] == "label" && node["label"] == "UI fixture"),
        "{reply}"
    );

    let reply = send(&mut app, &["click", "target=Effects"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.effects.open);
    assert_eq!(reply["windows"], serde_json::json!(["Effect Studio"]));
    let reply = send(
        &mut app,
        &["set", "target=Filter", "value=true", "window=Effect Studio"],
    );
    assert_eq!(reply["control"]["checked"], true, "{reply}");
    assert!(app.effects.state.filter.is_some());
    let reply = send(&mut app, &["set", "target=Cutoff", "value=1000"]);
    assert_eq!(reply["control"]["number"], 1000.0, "{reply}");
    assert_eq!(app.effects.state.filter.as_ref().unwrap().cutoff_hz, 1000.0);
    let reply = send(&mut app, &["set", "target=Filter", "value=false"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.effects.state.filter.is_none());

    let reply = send(&mut app, &["set", "target=Project BPM", "value=160"]);
    assert_eq!(reply["control"]["number"], 160.0, "{reply}");
    assert_eq!(app.session.project().bpm, 160.0);
    // The tempo slider snaps a value between notches to the nearest one.
    let reply = send(&mut app, &["set", "target=Project BPM", "value=133"]);
    assert_eq!(reply["control"]["number"], 135.0, "{reply}");
    assert_eq!(app.session.project().bpm, 135.0);

    // The tabs, the rail and the Sounds panel actions have distinct names.
    let reply = send(&mut app, &["click", "target=Step synth"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.board_shown(BottomModule::StepSynth) && app.sounds.group.is_none());
    // The board scopes its controls as the Step synth window did.
    let reply = send(&mut app, &["ui", "window=Step synth"]);
    assert!(
        reply["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["label"] == "Step synth BPM"),
        "{reply}"
    );
    let reply = send(&mut app, &["state"]);
    assert!(
        reply["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|window| window == "Step synth"),
        "{reply}"
    );
    // The tall board left the Sounds list too little height, so the
    // panel folded; opening it again folds the board instead.
    assert!(!app.sounds_panel_open());
    send(&mut app, &["click", "target=Show the Sounds panel"]);
    assert!(app.sounds_panel_open() && !app.panels.room.boards);
    send(&mut app, &["click", "target=Show Step synth sounds"]);
    assert_eq!(app.sounds.group.as_deref(), Some("Step synth"));
    send(&mut app, &["click", "target=Show All sounds"]);
    let reply = send(&mut app, &["click", "target=Mixer tab"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.boards.module, BottomModule::Mixer);
    // A name that several controls contain needs a selector.
    let reply = send(&mut app, &["click", "target=gain"]);
    assert!(
        reply["error"].as_str().unwrap().contains("index="),
        "{reply}"
    );
    // A choice can be named by its distinct word.
    send(&mut app, &["select", "sample=1"]);
    send(&mut app, &["click", "target=Stretch"]);
    let reply = send(&mut app, &["set", "target=Target BPM", "value=180"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.stretch.target_bpm, 180.0);
    let reply = send(&mut app, &["ui", "window=Time Stretcher"]);
    assert!(
        reply["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["label"] == "Source sound"),
        "{reply}"
    );
    send(&mut app, &["click", "target=Close Time Stretcher"]);
    let reply = send(&mut app, &["set", "target=Track 1 gain", "value=0.5"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(0.5 / 20.0)).abs() < 0.000_001);
    assert!(!app.session.project().tracks[0].polarity_inverted);
    send(&mut app, &["click", "target=Undo"]);
    assert_eq!(
        app.session.project().tracks[0].gain,
        Project::default().tracks[0].gain
    );

    // The Step synth board kept its height, so the Sounds panel opens
    // again to search.
    send(&mut app, &["click", "target=Show the Sounds panel"]);
    let reply = send(&mut app, &["set", "target=Find a sound", "value=grasp"]);
    assert_eq!(reply["control"]["text"], "grasp", "{reply}");
    assert_eq!(app.sounds.search, "grasp");
    send(&mut app, &["set", "target=Find a sound", "value="]);
    assert_eq!(app.sounds.search, "");

    let reply = send(&mut app, &["click", "target=No such control"]);
    assert_eq!(reply["ok"], false);
    let reply = send(&mut app, &["key", "key=Escape"]);
    assert_eq!(reply["ok"], true, "{reply}");
}

#[test]
fn agent_arranges_clips_by_command_and_by_pointer() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);

    let reply = send(&mut app, &["samples", "search=GRASP"]);
    assert_eq!(reply["total"], 1, "{reply}");
    assert_eq!(reply["samples"][0]["id"], 1);
    let reply = send(
        &mut app,
        &["place", "sample=good grasp 2", "track=3", "bar=5"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let placed = reply["clips"][0].as_u64().unwrap();
    let clip = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| clip.id == placed)
        .unwrap();
    assert_eq!((clip.track, clip.start_tick), (2, 4 * TICKS_PER_BAR));
    assert_eq!(app.selection.clips, [placed].into_iter().collect());
    for words in [
        &["place", "sample=1", "track=0", "bar=1"][..],
        &["place", "sample=1", "track=65", "bar=1"],
        &["place", "sample=nothing", "track=1", "bar=1"],
        &["place", "sample=1", "track=3", "bar=5"],
        &["delete_clips", "clips=[999]"],
        &["select", "clips=[999]"],
    ] {
        let reply = send(&mut app, words);
        assert_eq!(reply["ok"], false, "{words:?}: {reply}");
    }
    let count = app.session.project().clips.len();
    let reply = send(
        &mut app,
        &[
            "move_clips",
            &format!("clips=[{placed}]"),
            "tracks=1",
            "bars=2",
            "copy=true",
        ],
    );
    let copy = reply["clips"][0].as_u64().unwrap();
    assert_ne!(copy, placed);
    let clip = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| clip.id == copy)
        .unwrap();
    assert_eq!((clip.track, clip.start_tick), (3, 6 * TICKS_PER_BAR));
    let reply = send(&mut app, &["delete_clips", &format!("clips=[{copy}]")]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.session.project().clips.len(), count);

    send(&mut app, &["seek", "bar=3"]);
    assert_eq!(app.session.cursor_tick(), 2 * TICKS_PER_BAR);
    send(&mut app, &["set_loop", "first_bar=2", "last_bar=3"]);
    assert_eq!(
        app.session.project().loop_region,
        Some([TICKS_PER_BAR, 3 * TICKS_PER_BAR])
    );
    let state = send(&mut app, &["state"]);
    assert_eq!(state["song"]["loop_bars"], serde_json::json!([2, 3]));
    send(&mut app, &["set_loop"]);
    assert_eq!(app.session.project().loop_region, None);

    // Sections start at bars, and the state lists them in time order.
    let reply = send(
        &mut app,
        &[
            "set_sections",
            r#"sections=[{"bar":1,"name":"Intro"},{"bar":3,"name":"Drift"}]"#,
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(
        app.session.project().sections,
        vec![
            Section {
                start_tick: 0,
                name: "Intro".into()
            },
            Section {
                start_tick: 2 * TICKS_PER_BAR,
                name: "Drift".into()
            },
        ]
    );
    let state = send(&mut app, &["state"]);
    assert_eq!(state["song"]["sections"][1]["name"], "Drift");
    assert_eq!(state["song"]["sections"][1]["bar"], 3.0);
    let reply = send(
        &mut app,
        &[
            "set_sections",
            r#"sections=[{"bar":3,"name":"B"},{"bar":1,"name":"A"}]"#,
        ],
    );
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(app.session.project().sections.len(), 2);
    send(&mut app, &["set_sections", "sections=[]"]);
    assert!(app.session.project().sections.is_empty());

    // The state gives each clip's place in the window for pointer input.
    send(&mut app, &["select"]);
    assert!(app.selection.clips.is_empty() && app.selection.sample.is_none());
    let state = send(&mut app, &["state"]);
    assert_eq!(state["song"]["tracks"].as_array().unwrap().len(), 3);
    assert_eq!(
        state["arrangement"]["tracks"],
        serde_json::json!([1, 2, 3, 4])
    );
    let second = state["song"]["clips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|clip| clip["track"] == 2)
        .unwrap();
    let rect: Vec<f32> = second["rect"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap() as f32)
        .collect();
    let (x, y) = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0 + GRIP.y);
    let reply = send(
        &mut app,
        &[
            "pointer",
            "action=click",
            &format!("x={x}"),
            &format!("y={y}"),
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let id = second["id"].as_u64().unwrap();
    assert_eq!(app.selection.clips, [id].into_iter().collect());
    let state = send(&mut app, &["state"]);
    assert_eq!(
        state["selection"],
        serde_json::json!({"sample": 1, "clips": [id]})
    );
    // Dragging the clip to the Add row and two bars right moves it, as a person's drag does.
    let bar_width = state["arrangement"]["bar_width"].as_f64().unwrap() as f32;
    let row_height = state["arrangement"]["row_height"].as_f64().unwrap() as f32;
    let reply = send(
        &mut app,
        &[
            "pointer",
            "action=drag",
            &format!("x={x}"),
            &format!("y={y}"),
            &format!("to_x={}", x + 2.0 * bar_width),
            &format!("to_y={}", y + 2.0 * row_height),
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let clip = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| clip.id == id)
        .unwrap();
    assert_eq!((clip.track, clip.start_tick), (3, 3 * TICKS_PER_BAR));
    let reply = send(&mut app, &["pointer", "action=click", "x=-5", "y=10"]);
    assert_eq!(reply["ok"], false);
}

#[test]
fn agent_types_file_paths_in_the_choose_path_modal_and_resumes_a_pending_save() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    let dir = std::env::temp_dir().join(format!("mixjam-ui-agent-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let placeholder =
        Session::headless(Arc::new(Library::fixture(vec![])), Project::default()).unwrap();
    app.session =
        std::mem::replace(&mut app.session, placeholder).with_settings_root(dir.join("settings"));

    let reply = send(&mut app, &["click", "target=Export"]);
    assert_eq!(reply["modal"], "Choose path", "{reply}");
    let state = send(&mut app, &["state"]);
    assert_eq!(state["path_prompt"]["kind"], "save_file");
    assert_eq!(
        state["path_prompt"]["extensions"],
        serde_json::json!(["wav"])
    );
    let reply = send(&mut app, &["click", "target=Undo"]);
    assert!(
        reply["error"]
            .as_str()
            .unwrap()
            .contains("Choose path modal"),
        "{reply}"
    );
    let reply = send(&mut app, &["click", "target=Cancel"]);
    assert_eq!(reply["modal"], serde_json::Value::Null, "{reply}");
    assert!(!app.session.is_exporting());

    // An edit, then New: the unsaved-changes modal saves through a typed path.
    send(&mut app, &["place", "sample=1", "track=5", "bar=1"]);
    let reply = send(&mut app, &["click", "target=New"]);
    assert_eq!(reply["modal"], "Save your changes?", "{reply}");
    let reply = send(&mut app, &["click", "target=Save changes"]);
    assert_eq!(reply["modal"], "Choose path", "{reply}");
    send(&mut app, &["set", "target=Path", "value=song"]);
    let reply = send(&mut app, &["click", "target=Confirm path"]);
    assert_eq!(
        reply["modal"], "Choose path",
        "A relative path must be refused: {reply}"
    );
    let reply = send(&mut app, &["ui", "all=true", "contains=full path"]);
    assert!(reply["nodes"].as_array().unwrap().len() >= 2, "{reply}");
    // The extension of the file type is added, as a native dialog adds it.
    let song = dir.join("agent song");
    let reply = send(
        &mut app,
        &["set", "target=Path", &format!("value={}", song.display())],
    );
    assert_eq!(
        reply["control"]["text"],
        song.display().to_string(),
        "{reply}"
    );
    let reply = send(&mut app, &["click", "target=Confirm path"]);
    assert_eq!(reply["ok"], true, "{reply}");
    let song = dir.join("agent song.mixjam");
    assert!(song.is_file(), "{reply}");
    let reply = send(&mut app, &["state"]);
    assert_eq!(reply["modal"], serde_json::Value::Null, "{reply}");
    assert_eq!(reply["status"]["text"], "New mix.");
    assert!(app.session.project().clips.is_empty());

    let reply = send(&mut app, &["click", "target=Open"]);
    assert_eq!(reply["modal"], "Choose path", "{reply}");
    send(
        &mut app,
        &["set", "target=Path", &format!("value={}", song.display())],
    );
    send(&mut app, &["key", "key=Enter"]);
    assert_eq!(app.session.file(), Some(song.as_path()));
    assert_eq!(app.session.project().clips.len(), 3);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn agent_refuses_a_screenshot_of_a_minimized_window() {
    let (context, mut app, (port, key)) = agent_fixture();
    let client = std::thread::spawn(move || {
        let mut output = Vec::new();
        let words = ["screenshot", r"path=C:\Temp\window.png"].map(str::to_string);
        crate::agent::send(
            port,
            &key,
            [crate::agent::request_line(&words)],
            &mut output,
        )
        .unwrap();
        String::from_utf8(output).unwrap()
    });
    // The platform reports a minimized window with each frame's input.
    let started = std::time::Instant::now();
    while !client.is_finished() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "The window did not reply."
        );
        let info = egui::ViewportInfo {
            minimized: Some(true),
            ..Default::default()
        };
        let mut input = egui::RawInput {
            viewports: [(egui::ViewportId::ROOT, info)].into_iter().collect(),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
            ..Default::default()
        };
        app.agent_frame(&context, &mut input);
        let _ = context.run(input, |ctx| app.draw(ctx));
        std::thread::sleep(Duration::from_millis(1));
    }
    let reply: serde_json::Value = serde_json::from_str(&client.join().unwrap()).unwrap();
    assert_eq!(reply["ok"], false, "{reply}");
    assert!(
        reply["error"].as_str().unwrap().contains("minimized"),
        "{reply}"
    );
}

#[test]
fn agent_quit_asks_about_unsaved_work_and_replies_before_the_window_closes() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    send(&mut app, &["place", "sample=1", "track=5", "bar=1"]);
    let reply = send(&mut app, &["quit"]);
    assert_eq!(reply["modal"], "Save your changes?", "{reply}");
    assert!(reply.get("closing").is_none() && !app.files.allow_close);
    // Controls behind the modal stay out of reach, as they are for a pointer.
    let reply = send(&mut app, &["click", "target=Undo"]);
    assert!(
        reply["error"]
            .as_str()
            .unwrap()
            .contains("Save your changes? modal"),
        "{reply}"
    );
    assert!(app.session.unsaved());
    let reply = send(&mut app, &["click", "target=Discard changes"]);
    assert_eq!(reply["closing"], true, "{reply}");
    assert!(app.files.allow_close);
}
