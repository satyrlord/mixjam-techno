//! EQ window: channel EQ, compressor, ducker, drive and width.

use super::*;

#[test]
fn the_eq_window_edits_its_track_and_each_dial_turn_is_an_undo_step() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    app.boards.module = BottomModule::Mixer;
    let reply = send(&mut app, &["ui", "contains=Track 1 EQ"]);
    let button = &reply["nodes"][0];
    assert_eq!(button["label"], "Track 1 EQ", "{reply}");
    assert_eq!(button["checked"], false, "{reply}");
    let reply = send(&mut app, &["click", "target=Track 1 EQ"]);
    assert_eq!(
        reply["windows"],
        serde_json::json!(["Track 1 EQ"]),
        "{reply}"
    );
    // The dials wait for the power switch.
    let reply = send(&mut app, &["ui", "all=true", "window=Track 1 EQ"]);
    let nodes = reply["nodes"].as_array().unwrap();
    let named = |label: &str| {
        nodes
            .iter()
            .find(|node| node["label"] == label)
            .unwrap_or_else(|| panic!("Missing {label}: {reply}"))
    };
    for param in [
        "low cut",
        "low frequency",
        "low gain",
        "mid frequency",
        "mid gain",
        "mid Q",
        "peak 2 frequency",
        "peak 2 gain",
        "peak 2 Q",
        "high frequency",
        "high gain",
        "high cut",
    ] {
        assert_eq!(named(&format!("Track 1 EQ {param}"))["enabled"], false);
    }
    named("Track 1 EQ response curve");
    named("Reset Track 1 EQ");
    let reply = send(&mut app, &["set", "target=Track 1 EQ power", "value=true"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().tracks[0].eq.on);
    assert!(app.session.unsaved());
    // An accessibility SetValue names a place on the dial.
    let reply = send(
        &mut app,
        &["set", "target=Track 1 EQ high gain", "value=-9"],
    );
    assert_eq!(reply["control"]["number"], -9.0, "{reply}");
    assert_eq!(app.session.project().tracks[0].eq.high_db, -9.0);
    send(
        &mut app,
        &["set", "target=Track 1 EQ high frequency", "value=6500"],
    );
    assert_eq!(app.session.project().tracks[0].eq.high_hz, 6500.0);
    let state = send(&mut app, &["state"]);
    assert_eq!(state["song"]["tracks"][0]["eq_on"], true, "{state}");
    assert_eq!(state["song"]["tracks"][1]["eq_on"], false, "{state}");
    let reply = send(&mut app, &["ui", "contains=Track 1 EQ"]);
    assert_eq!(reply["nodes"][0]["checked"], true, "{reply}");
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].eq.high_hz, 8000.0);
    assert_eq!(app.session.project().tracks[0].eq.high_db, -9.0);
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].eq.high_db, 0.0);
    assert!(app.session.project().tracks[0].eq.on);
    // Reset flattens every band and keeps the switch.
    app.history_step(true);
    send(&mut app, &["click", "target=Reset Track 1 EQ"]);
    let eq = app.session.project().tracks[0].eq;
    assert!(eq.on && eq.high_db == 0.0);
    // Another track's button switches the window to that track.
    let reply = send(&mut app, &["click", "target=Track 2 EQ"]);
    assert_eq!(
        reply["windows"],
        serde_json::json!(["Track 2 EQ"]),
        "{reply}"
    );
    let reply = send(&mut app, &["click", "target=Close Track 2 EQ"]);
    assert_eq!(reply["windows"], serde_json::json!([]), "{reply}");
}

#[test]
fn the_eq_window_default_width_fits_beside_its_vertical_scrollbar() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.eq_track = Some(1);
        for _ in 0..12 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let bounds = accessible_node(&output, "Track 2 EQ response curve")
            .1
            .bounds()
            .unwrap();
        let curve = Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        );
        let horizontal_bar = output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.fill == roles().legend
                    && rect.rect.width() > 100.0
                    && rect.rect.height() < 16.0
                    && rect.rect.top() > curve.bottom()
                    && rect.rect.left() >= curve.left()
                    && rect.rect.right() <= curve.right())
        });
        assert!(
            !horizontal_bar,
            "The EQ content must fit beside its vertical scrollbar at {size:?}."
        );
    }
}

#[test]
fn compressor_and_ducker_labels_are_visible_at_the_bottom_of_the_eq_window() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (ctx, mut app) = fixture();
        app.eq_track = Some(1);
        for _ in 0..4 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let point = text_rect(&output, "Mid peak").center();
        frame_at_size(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -20_000.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
            size,
        );
        for _ in 0..60 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let q = text_rect(&output, "Q");
        let ducker = text_rect(&output, "Ducker");
        let separator = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if (500.0..540.0).contains(&rect.rect.width())
                        && rect.rect.height() <= 2.0
                        && rect.rect.top() > q.top()
                        && rect.rect.bottom() < ducker.top() =>
                {
                    Some(rect.rect.top())
                }
                _ => None,
            })
            .min_by(f32::total_cmp)
            .expect("The compressor section needs its separator.");
        assert!(
            separator >= q.bottom() + 2.0,
            "The compressor separator crosses the EQ Q label at {size:?}: {q:?}, {separator}"
        );
        for label in [
            "Compressor",
            "Ratio",
            "Makeup",
            "Stereo",
            "Width",
            "Key",
            "Thresh",
            "Depth",
            "Attack",
            "Release",
        ] {
            assert!(
                output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text)
                        if text.galley.job.text == label
                        && shape.clip_rect.contains_rect(text.visual_bounding_rect()))
                }),
                "{label} is clipped at {size:?}"
            );
        }
    }
}

#[test]
fn opening_eq_preserves_an_off_ducker_with_its_own_track_as_key() {
    let (context, mut app, port) = agent_fixture();
    let mut project = Project::default();
    project.tracks[1].ducker.key = 2;
    app.session = Session::headless(Arc::new(Library::empty()), project.clone()).unwrap();
    app.eq_track = Some(1);
    agent(&context, &mut app, &port, &["ui", "all=true"]);
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.unsaved());
    assert!(!app.session.has_history_step(false));
}

#[test]
fn the_eq_window_holds_the_ducker_whose_key_steps_over_its_own_track() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    app.boards.module = BottomModule::Mixer;
    send(&mut app, &["click", "target=Track 1 EQ"]);
    // The ducker's dials wait for its switch.
    let reply = send(&mut app, &["ui", "all=true", "window=Track 1 EQ"]);
    let nodes = reply["nodes"].as_array().unwrap();
    for control in ["key", "threshold", "depth", "attack", "release"] {
        let label = format!("Track 1 ducker {control}");
        let node = nodes
            .iter()
            .find(|node| node["label"] == label.as_str())
            .unwrap_or_else(|| panic!("Missing {label}: {reply}"));
        assert_eq!(node["enabled"], false, "{label}");
    }
    let reply = send(
        &mut app,
        &["set", "target=Track 1 ducker power", "value=true"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    // On, it keys from another track than its own.
    let ducker = app.session.project().tracks[0].ducker;
    assert!(ducker.on);
    assert_eq!(ducker.key, 2);
    send(&mut app, &["set", "target=Track 1 ducker depth", "value=9"]);
    assert_eq!(app.session.project().tracks[0].ducker.depth, 9.0);
    send(&mut app, &["set", "target=Track 1 ducker key", "value=1"]);
    assert_eq!(app.session.project().tracks[0].ducker.key, 2);
    send(&mut app, &["set", "target=Track 1 ducker key", "value=5"]);
    assert_eq!(app.session.project().tracks[0].ducker.key, 5);
    // The strip's EQ button lights for the ducker as for the EQ.
    assert!(!app.session.project().tracks[0].eq.on);
    let reply = send(&mut app, &["ui", "contains=Track 1 EQ"]);
    assert_eq!(reply["nodes"][0]["checked"], true, "{reply}");
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].ducker.key, 2);
}

#[test]
fn the_eq_window_holds_the_compressor_and_the_width() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    app.boards.module = BottomModule::Mixer;
    send(&mut app, &["click", "target=Track 1 EQ"]);
    // The compressor's dials wait for its switch; the width has none.
    let reply = send(&mut app, &["ui", "all=true", "window=Track 1 EQ"]);
    let nodes = reply["nodes"].as_array().unwrap();
    let named = |label: &str| {
        nodes
            .iter()
            .find(|node| node["label"] == label)
            .unwrap_or_else(|| panic!("Missing {label}: {reply}"))
    };
    for control in ["threshold", "ratio", "attack", "release", "makeup"] {
        let label = format!("Track 1 compressor {control}");
        assert_eq!(named(&label)["enabled"], false, "{label}");
    }
    assert_eq!(named("Track 1 width")["enabled"], true);
    named("Track 1 compressor power");
    let reply = send(
        &mut app,
        &["set", "target=Track 1 compressor power", "value=true"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().tracks[0].compressor.on);
    let reply = send(
        &mut app,
        &["set", "target=Track 1 compressor ratio", "value=8"],
    );
    assert_eq!(reply["control"]["number"], 8.0, "{reply}");
    assert_eq!(app.session.project().tracks[0].compressor.ratio, 8.0);
    send(&mut app, &["set", "target=Track 1 width", "value=40"]);
    assert_eq!(app.session.project().tracks[0].width, 40.0);
    let state = send(&mut app, &["state"]);
    assert_eq!(state["song"]["tracks"][0]["compressor_on"], true, "{state}");
    assert_eq!(state["song"]["tracks"][0]["width"], 40.0, "{state}");
    // The strip's EQ button lights for the compressor and the width.
    assert!(!app.session.project().tracks[0].eq.on);
    let reply = send(&mut app, &["ui", "contains=Track 1 EQ"]);
    assert_eq!(reply["nodes"][0]["checked"], true, "{reply}");
    // Each control is an undo step of its own.
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].width, 100.0);
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].compressor.ratio, 4.0);
    app.history_step(false);
    assert!(!app.session.project().tracks[0].compressor.on);
}

#[test]
fn the_eq_window_holds_the_drive() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    app.boards.module = BottomModule::Mixer;
    send(&mut app, &["click", "target=Track 1 EQ"]);
    // The drive's dials wait for its switch.
    let reply = send(&mut app, &["ui", "all=true", "window=Track 1 EQ"]);
    let nodes = reply["nodes"].as_array().unwrap();
    let named = |label: &str| {
        nodes
            .iter()
            .find(|node| node["label"] == label)
            .unwrap_or_else(|| panic!("Missing {label}: {reply}"))
    };
    for control in ["amount", "character", "mix", "focus", "output"] {
        let label = format!("Track 1 drive {control}");
        assert_eq!(named(&label)["enabled"], false, "{label}");
    }
    named("Track 1 drive power");
    let reply = send(
        &mut app,
        &["set", "target=Track 1 drive power", "value=true"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().tracks[0].drive.on);
    let reply = send(
        &mut app,
        &["set", "target=Track 1 drive amount", "value=18"],
    );
    assert_eq!(reply["control"]["number"], 18.0, "{reply}");
    send(&mut app, &["set", "target=Track 1 drive focus", "value=98"]);
    send(&mut app, &["set", "target=Track 1 drive mix", "value=40"]);
    let drive = app.session.project().tracks[0].drive;
    assert_eq!((drive.amount, drive.focus, drive.mix), (18.0, 98.0, 40.0));
    // The state reports the drive with its settings.
    let state = send(&mut app, &["state"]);
    assert_eq!(
        state["song"]["tracks"][0]["drive"],
        serde_json::json!({
            "on": true, "amount": 18.0, "character": 25.0, "mix": 40.0, "focus": 98.0,
            "output": 0.0
        }),
        "{state}"
    );
    // The strip's EQ button lights for the drive.
    assert!(!app.session.project().tracks[0].eq.on);
    let reply = send(&mut app, &["ui", "contains=Track 1 EQ"]);
    assert_eq!(reply["nodes"][0]["checked"], true, "{reply}");
    // Each control is an undo step of its own; the switch is one too.
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].drive.mix, 50.0);
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].drive.focus, 100.0);
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].drive.amount, 12.0);
    assert!(app.session.project().tracks[0].drive.on);
    app.history_step(false);
    assert!(!app.session.project().tracks[0].drive.on);
}

#[test]
fn the_strip_eq_button_names_channel_processing_not_the_eq_alone() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    app.boards.module = BottomModule::Mixer;
    send(&mut app, &["click", "target=Track 1 EQ"]);
    send(
        &mut app,
        &["set", "target=Track 1 compressor power", "value=true"],
    );
    app.eq_track = None;
    assert!(!app.session.project().tracks[0].eq.on);
    context.enable_accesskit();
    context.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let center = effects_control_rect(&output, "Track 1 EQ").center();
    frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(center)],
        egui::Modifiers::NONE,
    );
    let mut tooltip = None;
    for _ in 0..4 {
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        tooltip = tooltip.or_else(|| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text.contains("Open the channel EQ window") =>
                {
                    Some(text.galley.job.text.clone())
                }
                _ => None,
            })
        });
    }
    let tooltip = tooltip.expect("The EQ button must explain itself on hover.");
    // The button lights for the compressor, so the hover must not say the EQ is on.
    assert!(
        tooltip.starts_with("Track 1 channel processing: on."),
        "{tooltip}"
    );
}
