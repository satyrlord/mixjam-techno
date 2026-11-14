//! Automation lanes reached through the lane menu, the keyboard and the agent channel.

use super::*;

#[test]
fn agent_automates_the_pan_lane_and_reports_it_in_the_state() {
    let (context, mut app, port) = agent_fixture();
    let reply = agent(
        &context,
        &mut app,
        &port,
        &[
            "automate",
            "track=1",
            "lane=pan",
            "points=[[1,-1],[1.25,1],[2,0]]",
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let points: Vec<(u32, f32)> = app.session.project().tracks[0]
        .automation
        .pan
        .iter()
        .map(|point| (point.tick, point.value))
        .collect();
    assert_eq!(points, [(0, -1.0), (64, 1.0), (256, 0.0)]);
    // The knob keeps its value; the state lists the lane beside it.
    let state = agent(&context, &mut app, &port, &["state"]);
    assert_eq!(state["song"]["tracks"][0]["pan"], 0.0, "{state}");
    assert_eq!(
        state["song"]["tracks"][0]["automation"]["pan"],
        serde_json::json!([
            {"tick": 0, "value": -1.0},
            {"tick": 64, "value": 1.0},
            {"tick": 256, "value": 0.0}
        ]),
        "{state}"
    );
    // A value beyond hard left or right, or an unknown lane, changes nothing.
    for words in [
        &["automate", "track=1", "lane=pan", "points=[[1,-1.5]]"][..],
        &["automate", "track=1", "lane=balance", "points=[[1,0]]"],
    ] {
        let reply = agent(&context, &mut app, &port, words);
        assert_eq!(reply["ok"], false, "{words:?}: {reply}");
    }
    assert_eq!(app.session.project().tracks[0].automation.pan.len(), 3);
    // An empty list clears the lane, and the knob applies again.
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["automate", "track=1", "lane=pan", "points=[]"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().tracks[0].automation.pan.is_empty());
    let state = agent(&context, &mut app, &port, &["state"]);
    let automation = &state["song"]["tracks"][0]["automation"];
    assert!(automation.get("pan").is_none(), "{state}");
}

#[test]
fn agent_automates_the_repeat_lane_and_reports_it_in_the_state() {
    let (context, mut app, port) = agent_fixture();
    let reply = agent(
        &context,
        &mut app,
        &port,
        &[
            "automate",
            "track=1",
            "lane=repeat",
            "points=[[1,8],[1.25,16],[1.375,32],[1.5,0]]",
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let points: Vec<(u32, f32)> = app.session.project().tracks[0]
        .automation
        .repeat
        .iter()
        .map(|point| (point.tick, point.value))
        .collect();
    assert_eq!(points, [(0, 8.0), (64, 16.0), (96, 32.0), (128, 0.0)]);
    let state = agent(&context, &mut app, &port, &["state"]);
    assert_eq!(
        state["song"]["tracks"][0]["automation"]["repeat"],
        serde_json::json!([
            {"tick": 0, "value": 8.0},
            {"tick": 64, "value": 16.0},
            {"tick": 96, "value": 32.0},
            {"tick": 128, "value": 0.0}
        ]),
        "{state}"
    );
    // A value that is no division changes nothing.
    for value in ["12", "128", "-8", "0.5"] {
        let points = format!("points=[[1,{value}]]");
        let reply = agent(
            &context,
            &mut app,
            &port,
            &["automate", "track=1", "lane=repeat", &points],
        );
        assert_eq!(reply["ok"], false, "{value}: {reply}");
    }
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["automate", "track=1", "lane=ratchet", "points=[[1,8]]"],
    );
    let error = reply["error"].as_str().unwrap_or_default();
    assert!(error.contains("repeat"), "{reply}");
    assert_eq!(app.session.project().tracks[0].automation.repeat.len(), 4);
    // An empty list clears the lane.
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["automate", "track=1", "lane=repeat", "points=[]"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let state = agent(&context, &mut app, &port, &["state"]);
    let automation = &state["song"]["tracks"][0]["automation"];
    assert!(automation.get("repeat").is_none(), "{state}");
}

#[test]
fn the_lane_menu_offers_the_repeat_gate_and_each_fx_return() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    choose_automation_lane(&ctx, &mut app, "Repeat gate");
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (Some(Lane::RepeatGate), None)
    );
    choose_automation_lane(&ctx, &mut app, "FX 3 return");
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (None, Some(2))
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "FX 3 return (100% dashed)");
    choose_automation_lane(&ctx, &mut app, "Off");
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (None, None)
    );
}

#[test]
fn the_lane_menu_groups_all_fourteen_lanes_and_shows_what_each_holds() {
    use mixjam_techno::application::automation::{Lane, Point};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let point = |tick| Point { tick, value: 0.5 };
    for track in [0, 1] {
        app.session
            .edit(EditCommand::SetAutomation {
                track,
                lane: Lane::Filter,
                points: vec![point(0)],
            })
            .unwrap();
    }
    app.session
        .edit(EditCommand::SetAutomation {
            track: 0,
            lane: Lane::Send(0),
            points: vec![point(0)],
        })
        .unwrap();
    app.session
        .edit(EditCommand::SetReturnAutomation {
            bus: 1,
            points: (0..4).map(|bar| point(bar * TICKS_PER_BAR)).collect(),
        })
        .unwrap();
    app.score_tools.automation_lane = Some(Lane::Filter);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (selector, _) = accessible_node(&output, "Automation lane");
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target: selector,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // Three headed groups, in the order of the concept.
    let headings = [
        "ON EVERY TRACK",
        "SEND AMOUNT, ON EVERY TRACK",
        "RETURN LEVEL, ONE LINE ACROSS THE SONG",
    ]
    .map(|heading| text_rect(&output, heading).top());
    assert!(headings.is_sorted(), "{headings:?}");
    assert!(text_rect(&output, "Pan").top() < headings[1]);
    assert!(text_rect(&output, "FX 1 send · empty").top() > headings[1]);
    assert!(text_rect(&output, "FX 1 return · empty").top() > headings[2]);
    // Each lane says how much it holds.
    text_rect(&output, "2 tracks");
    text_rect(&output, "1 track");
    text_rect(&output, "4 points");
    // The current lane is marked for assistive technology too.
    let toggled = |label| accessible_node(&output, label).1.toggled();
    assert_eq!(toggled("Filter"), Some(egui::accesskit::Toggled::True));
    assert_eq!(toggled("Volume"), Some(egui::accesskit::Toggled::False));
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );

    // Every lane is one click away.
    type Choice = (Option<Lane>, Option<usize>);
    let mut lanes: Vec<(String, Choice)> = Lane::ALL
        .iter()
        .map(|lane| (lane.label(), (Some(*lane), None)))
        .collect();
    lanes.extend(
        (0..mixjam_techno::application::fx::BUSES)
            .map(|bus| (format!("FX {} return", bus + 1), (None, Some(bus)))),
    );
    assert_eq!(lanes.len(), 14);
    for (label, lane) in lanes {
        choose_automation_lane(&ctx, &mut app, &label);
        assert_eq!(
            (app.score_tools.automation_lane, app.score_tools.return_lane),
            lane,
            "{label}"
        );
    }
    choose_automation_lane(&ctx, &mut app, "Off");
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (None, None)
    );
}

#[test]
fn the_agent_automate_request_reaches_all_fourteen_lanes() {
    use mixjam_techno::application::automation::Lane;
    let (context, mut app, port) = agent_fixture();
    for lane in Lane::ALL {
        let value = match lane {
            Lane::Repeat => "8",
            Lane::RepeatGate => "50",
            Lane::Send(_) => "0.5",
            _ => "0",
        };
        let key = lane.key();
        let reply = agent(
            &context,
            &mut app,
            &port,
            &[
                "automate",
                "track=1",
                &format!("lane={key}"),
                &format!("points=[[2,{value}]]"),
            ],
        );
        assert_eq!(reply["ok"], true, "{key}: {reply}");
        assert_eq!(
            app.session.project().tracks[0].automation.lane(lane).len(),
            1,
            "{key}"
        );
    }
    for bus in 1..=mixjam_techno::application::fx::BUSES {
        let reply = agent(
            &context,
            &mut app,
            &port,
            &[
                "automate",
                &format!("bus={bus}"),
                "lane=return",
                "points=[[2,1]]",
            ],
        );
        assert_eq!(reply["ok"], true, "bus {bus}: {reply}");
        assert_eq!(app.session.project().return_automation[bus - 1].len(), 1);
    }
}

#[test]
fn the_keyboard_opens_the_lane_menu_and_chooses_a_lane() {
    use mixjam_techno::application::automation::Lane;
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
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // Tab walks into the menu, past Off, to Volume; Enter chooses it.
    let focused = |output: &egui::FullOutput| {
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == update.focus)
            .and_then(|(_, node)| node.label().map(str::to_owned))
    };
    let mut walked = Vec::new();
    for _ in 0..6 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        walked.extend(focused(&output));
        if walked.last().map(String::as_str) == Some("Volume") {
            break;
        }
    }
    assert_eq!(walked, ["Off", "Volume"], "{walked:?}");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (Some(Lane::Volume), None)
    );
}

#[test]
fn agent_automates_the_pitch_lane_and_reports_it_in_the_state() {
    let (context, mut app, port) = agent_fixture();
    let reply = agent(
        &context,
        &mut app,
        &port,
        &[
            "automate",
            "track=1",
            "lane=pitch",
            "points=[[1,0],[1.5,-4800],[2,350.5]]",
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let points: Vec<(u32, f32)> = app.session.project().tracks[0]
        .automation
        .pitch
        .iter()
        .map(|point| (point.tick, point.value))
        .collect();
    assert_eq!(points, [(0, 0.0), (128, -4800.0), (256, 350.5)]);
    let state = agent(&context, &mut app, &port, &["state"]);
    assert_eq!(
        state["song"]["tracks"][0]["automation"]["pitch"],
        serde_json::json!([
            {"tick": 0, "value": 0.0},
            {"tick": 128, "value": -4800.0},
            {"tick": 256, "value": 350.5}
        ]),
        "{state}"
    );
    // A value outside the range changes nothing.
    for value in ["-4801", "2401"] {
        let points = format!("points=[[1,{value}]]");
        let reply = agent(
            &context,
            &mut app,
            &port,
            &["automate", "track=1", "lane=pitch", &points],
        );
        assert_eq!(reply["ok"], false, "{value}: {reply}");
    }
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["automate", "track=1", "lane=tapestop", "points=[[1,0]]"],
    );
    let error = reply["error"].as_str().unwrap_or_default();
    assert!(error.contains("pitch"), "{reply}");
    assert_eq!(app.session.project().tracks[0].automation.pitch.len(), 3);
    // An empty list clears the lane.
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["automate", "track=1", "lane=pitch", "points=[]"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let state = agent(&context, &mut app, &port, &["state"]);
    let automation = &state["song"]["tracks"][0]["automation"];
    assert!(automation.get("pitch").is_none(), "{state}");
}

#[test]
fn agent_automates_the_gate_and_a_return_and_reverses_a_clip() {
    let (context, mut app, port) = agent_fixture();
    for (words, ok) in [
        (
            &[
                "automate",
                "track=1",
                "lane=repeat",
                "points=[[1,16],[2,0]]",
            ][..],
            true,
        ),
        (
            &[
                "automate",
                "track=1",
                "lane=repeatgate",
                "points=[[1,25],[1.5,60]]",
            ],
            true,
        ),
        (
            &[
                "automate",
                "bus=2",
                "lane=return",
                "points=[[1,0.25],[1.75,2],[2,0.25]]",
            ],
            true,
        ),
        (
            &["automate", "track=1", "lane=repeatgate", "points=[[1,4]]"],
            false,
        ),
        (
            &["automate", "track=1", "lane=repeatgate", "points=[[1,101]]"],
            false,
        ),
        (
            &["automate", "bus=2", "lane=return", "points=[[1,2.5]]"],
            false,
        ),
        (
            &["automate", "bus=5", "lane=return", "points=[[1,1]]"],
            false,
        ),
        (
            &["automate", "bus=2", "lane=volume", "points=[[1,1]]"],
            false,
        ),
        (
            &["automate", "bus=2", "track=1", "lane=return", "points=[]"],
            false,
        ),
        (&["automate", "lane=return", "points=[]"], false),
    ] {
        let reply = agent(&context, &mut app, &port, words);
        assert_eq!(reply["ok"], ok, "{words:?}: {reply}");
    }
    let id = app.session.project().clips[0].id;
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["clip", &format!("clip={id}"), "reverse=true"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().clips[0].reverse);
    let state = agent(&context, &mut app, &port, &["state"]);
    let song = &state["song"];
    assert_eq!(
        song["tracks"][0]["automation"]["repeatGate"],
        serde_json::json!([{"tick": 0, "value": 25.0}, {"tick": 128, "value": 60.0}]),
        "{state}"
    );
    assert_eq!(song["fx_buses"].as_array().unwrap().len(), 4, "{state}");
    assert_eq!(song["fx_buses"][1]["bus"], 2);
    assert_eq!(
        song["fx_buses"][1]["return_automation"],
        serde_json::json!([
            {"tick": 0, "value": 0.25},
            {"tick": 192, "value": 2.0},
            {"tick": 256, "value": 0.25}
        ]),
        "{state}"
    );
    assert_eq!(
        song["fx_buses"][0]["return_automation"],
        serde_json::json!([])
    );
    let clip = song["clips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|clip| clip["id"] == id)
        .unwrap();
    assert_eq!(clip["reverse"], true, "{state}");
    // An empty list clears the return lane; reverse=false plays forward.
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["automate", "bus=2", "lane=return", "points=[]"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().return_automation[1].is_empty());
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["clip", &format!("clip={id}"), "reverse=false"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(!app.session.project().clips[0].reverse);
}
