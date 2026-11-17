//! The Options pages on a bare context: values and facts in, actions out,
//! with no app and no session, so a page's look can change without a song.

use super::*;
use mixjam_techno::application::options::{AppOptions, SampleQuality, SongFolderMode};
use options::labels::*;
use options::{Action, OptionsDraft, OptionsFacts, Page, pages};

/// A bare context with the app's style and accessibility on.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    configure_style(&ctx);
    ctx
}

/// The bay's top edge.
const TOP: f32 = 20.0;

/// Paint `page` with its notices after `events`, and return the frame and
/// the actions it reported.
fn paint(
    ctx: &egui::Context,
    page: Page,
    values: &mut AppOptions,
    facts: &OptionsFacts,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, Vec<Action>) {
    let draft = OptionsDraft {
        values: values.clone(),
        ..OptionsDraft::new(facts, page)
    };
    let mut actions = Vec::new();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 900.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let bay = Rect::from_min_size(Pos2::new(8.0, TOP), Vec2::new(560.0, 860.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(bay), |ui| {
                    pages::notices(ui, &draft, facts, &mut actions);
                    pages::page(ui, page, values, facts, &mut actions);
                });
            });
        },
    );
    (output, actions)
}

/// Paint twice, so the second frame's layout has settled.
fn settled(
    ctx: &egui::Context,
    page: Page,
    values: &mut AppOptions,
    facts: &OptionsFacts,
) -> egui::FullOutput {
    paint(ctx, page, values, facts, vec![]);
    paint(ctx, page, values, facts, vec![]).0
}

/// Click the control named `name` through accessibility, and return what
/// the page reported.
fn click(
    ctx: &egui::Context,
    page: Page,
    values: &mut AppOptions,
    facts: &OptionsFacts,
    name: &str,
) -> Vec<Action> {
    let output = settled(ctx, page, values, facts);
    let target = screen(&output).node(name).id();
    let request = egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Click,
        target,
        data: None,
    });
    paint(ctx, page, values, facts, vec![request]).1
}

#[test]
fn each_page_opens_with_an_uppercase_section_title_below_the_bay_top() {
    for (page, title) in [
        (Page::Audio, PLAYBACK_QUALITY),
        (Page::Workspace, WORKSPACE),
        (Page::Files, SONG_FOLDERS),
    ] {
        let ctx = context();
        let output = settled(
            &ctx,
            page,
            &mut AppOptions::default(),
            &OptionsFacts::default(),
        );
        let heading = screen(&output).text(&title.to_uppercase());
        assert!(
            heading.rect().top() >= TOP + 3.0,
            "{title}: {:?}",
            heading.rect()
        );
        assert_eq!(heading.color(), roles().legend, "{title}");
    }
}

#[test]
fn files_shows_the_fixed_folder_only_in_fixed_mode_and_checks_it_inline() {
    let ctx = context();
    let facts = OptionsFacts::default();
    let mut values = AppOptions::default();
    let output = settled(&ctx, Page::Files, &mut values, &facts);
    for name in [FIXED_FOLDER, CHOOSE_FOLDER] {
        assert!(
            !has_control(&output, name),
            "{name} is shown in Last used mode"
        );
    }
    assert!(has_control(&output, OPEN_LIBRARIES));
    values.song_folder_mode = SongFolderMode::Fixed;
    // No folder yet: the reason that keeps Apply disabled shows under it.
    let output = settled(&ctx, Page::Files, &mut values, &facts);
    assert!(has_control(&output, FIXED_FOLDER));
    let reason = values.validate().unwrap_err();
    assert_eq!(screen(&output).text(&reason).color(), roles().coral);
    values.fixed_song_folder = Some(PathBuf::from("relative"));
    let output = settled(&ctx, Page::Files, &mut values, &facts);
    let reason = values.validate().unwrap_err();
    assert_eq!(screen(&output).text(&reason).color(), roles().coral);
    // A valid folder shows its help instead.
    values.fixed_song_folder = Some(std::env::temp_dir());
    let output = settled(&ctx, Page::Files, &mut values, &facts);
    assert!(shows_text(
        &output,
        "An existing folder, chosen or typed as a full path."
    ));
}

#[test]
fn files_buttons_report_their_actions() {
    let ctx = context();
    let facts = OptionsFacts::default();
    let mut values = AppOptions {
        song_folder_mode: SongFolderMode::Fixed,
        ..AppOptions::default()
    };
    assert_eq!(
        click(&ctx, Page::Files, &mut values, &facts, CHOOSE_FOLDER),
        [Action::PickFolder]
    );
    assert_eq!(
        click(&ctx, Page::Files, &mut values, &facts, OPEN_LIBRARIES),
        [Action::OpenLibraries]
    );
}

#[test]
fn reduced_quality_is_a_warning_not_an_error() {
    let ctx = context();
    let facts = OptionsFacts::default();
    let mut values = AppOptions::default();
    let output = settled(&ctx, Page::Audio, &mut values, &facts);
    assert!(!shows_text(&output, QUALITY_REDUCED));
    assert_eq!(screen(&output).node(SAMPLE_QUALITY).value(), Some("High"));
    values.sample_playback_quality = SampleQuality::Low;
    let output = settled(&ctx, Page::Audio, &mut values, &facts);
    assert_eq!(screen(&output).node(SAMPLE_QUALITY).value(), Some("Low"));
    assert_eq!(
        screen(&output).text(QUALITY_REDUCED).color(),
        roles().warning
    );
}

#[test]
fn device_details_stay_closed_until_opened() {
    let ctx = context();
    let facts = OptionsFacts {
        buffer_status: "Requested automatic; measured 10.7 ms.".into(),
        ..OptionsFacts::default()
    };
    let mut values = AppOptions::default();
    let output = settled(&ctx, Page::Audio, &mut values, &facts);
    assert!(has_control(&output, DEVICE_DETAILS));
    assert!(!shows_text(&output, &facts.buffer_status));
    assert!(!has_control(&output, SAVE_CAPTURE));
    // Opening the disclosure reports nothing and changes no value.
    assert_eq!(
        click(&ctx, Page::Audio, &mut values, &facts, DEVICE_DETAILS),
        []
    );
    let output = settled(&ctx, Page::Audio, &mut values, &facts);
    assert!(shows_text(&output, &facts.buffer_status));
    assert_eq!(values, AppOptions::default());
    assert_eq!(
        click(&ctx, Page::Audio, &mut values, &facts, SAVE_CAPTURE),
        [Action::SaveCapture]
    );
}

#[test]
fn a_buffer_target_the_device_refuses_shows_why() {
    let ctx = context();
    let mut values = AppOptions::default();
    let facts = OptionsFacts {
        buffer_errors: vec![(
            values.playback_buffer_target,
            "The device refused it.".into(),
        )],
        ..OptionsFacts::default()
    };
    let output = settled(&ctx, Page::Audio, &mut values, &facts);
    assert_eq!(
        screen(&output).text("The device refused it.").color(),
        roles().coral
    );
}

#[test]
fn a_stale_draft_offers_reload_above_the_page() {
    let ctx = context();
    let mut values = AppOptions::default();
    let mut facts = OptionsFacts::default();
    let draft_facts = facts.clone();
    facts.revision = 1;
    // `paint` starts its draft from the facts, so make it stale by hand.
    let draft = OptionsDraft::new(&draft_facts, Page::Workspace);
    let mut actions = Vec::new();
    let run = |events: Vec<egui::Event>, actions: &mut Vec<Action>, values: &mut AppOptions| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 900.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    pages::notices(ui, &draft, &facts, actions);
                    pages::page(ui, Page::Workspace, values, &facts, actions);
                });
            },
        )
    };
    run(vec![], &mut actions, &mut values);
    let output = run(vec![], &mut actions, &mut values);
    let stale = screen(&output).text(mixjam_techno::application::options::STALE_DRAFT);
    assert_eq!(stale.color(), roles().warning);
    assert!(stale.rect().top() < screen(&output).text("WORKSPACE").rect().top());
    let target = screen(&output).node(RELOAD).id();
    run(
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
        &mut actions,
        &mut values,
    );
    assert_eq!(actions, [Action::Reload]);
}

#[test]
fn workspace_choices_name_their_values() {
    let ctx = context();
    let mut values = AppOptions::default();
    let output = settled(&ctx, Page::Workspace, &mut values, &OptionsFacts::default());
    assert_eq!(screen(&output).node(THEME).value(), Some("Studio"));
    assert_eq!(screen(&output).node(LANE_HEIGHT).value(), Some("30 px"));
    assert_eq!(
        screen(&output).node(STARTING_MODULE).value(),
        Some("Sounds")
    );
}

#[test]
fn hidden_pointer_hints_keep_help_without_opening_tooltips() {
    for show in [true, false] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        ctx.style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("show-hover-hints"), show));
        let mut output = egui::FullOutput::default();
        for frame in 0..4 {
            output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
                    time: Some(frame as f64),
                    events: vec![egui::Event::PointerMoved(Pos2::new(20.0, 20.0))],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.add_sized([200.0, 50.0], egui::Button::new("Pointer control"))
                            .hover_hint_at_pointer("Pointer help text");
                    });
                },
            );
        }
        assert_eq!(
            screen(&output).node("Pointer control").node().description(),
            Some("Pointer help text")
        );
        let tooltip = screen(&output).nodes().any(|node| {
            node.role() == egui::accesskit::Role::Label && node.value() == Some("Pointer help text")
        });
        assert_eq!(tooltip, show);
    }
}

#[test]
fn hover_hints_preserve_explicit_accessible_state() {
    for show in [false, true] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("show-hover-hints"), show));
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = ui.button("State control");
                ctx.accesskit_node_builder(response.id, |node| {
                    node.set_description("OVER: detected.")
                });
                response.hover_hint("Long hover instructions.");
                let pointer = ui.button("Pointer state control");
                ctx.accesskit_node_builder(pointer.id, |node| {
                    node.set_description("Current value.")
                });
                pointer.hover_hint_at_pointer("Long pointer instructions.");
            });
        });
        assert_eq!(
            screen(&output).node("State control").node().description(),
            Some("OVER: detected.")
        );
        assert_eq!(
            screen(&output)
                .node("Pointer state control")
                .node()
                .description(),
            Some("Current value.")
        );
    }
}
