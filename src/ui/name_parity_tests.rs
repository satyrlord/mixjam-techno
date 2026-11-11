//! Guard for the layout migration: every accessible name that the main window,
//! its lower-panel modules, its tool windows and its modals expose today must
//! still exist after a change, or be listed in `RENAMED` with the name that
//! replaces it. `ui-names.txt` holds the names at the start of the migration.
//!
//! Regenerate the list only after the owner approves a removal:
//! `MIXJAM_TECHNO_WRITE_UI_NAMES=1 cargo test name_parity`.

use super::*;
use mixjam_techno::library::Library;
use std::collections::BTreeSet;

const BASELINE: &str = include_str!("ui-names.txt");
const BASELINE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/ui-names.txt");

/// Album Review buttons named after the bundled reviews: data, not controls.
const ALBUM_DATA: &[&str] = &["Open review: ", "Artist filter: ", "Genre filter: "];

/// Approved renames: an old `role<TAB>name` line and the line that replaces it.
const RENAMED: &[(&str, &str)] = &[];

fn sample(id: u32, name: &str, group: &str) -> Sample {
    Sample {
        id,
        code: format!("PARITY{id}"),
        name: name.into(),
        bars: 1,
        group_name: group.into(),
        offset: 0,
        length: 0,
        frame_count: 75600,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    }
}

fn app(with_clips: bool) -> (egui::Context, MixJamApp) {
    let context = egui::Context::default();
    configure_style(&context);
    context.enable_accesskit();
    context.style_mut(|style| {
        style.animation_time = 0.0;
        style.scroll_animation = egui::style::ScrollAnimation::none();
    });
    let library = Arc::new(Library::fixture(vec![
        sample(1, "parity loop", "Loop"),
        sample(2, "parity drum", "Drum"),
        sample(3, "parity bass", "Bass"),
        sample(4, "parity voice", "Voice"),
    ]));
    let mut editor = crate::engine::Editor::new(Project::default());
    if with_clips {
        for track in 0..4 {
            editor
                .place(track as u32 + 1, track, 0, TICKS_PER_BAR)
                .unwrap();
            editor
                .place(track as u32 + 1, track, 8 * TICKS_PER_BAR, TICKS_PER_BAR)
                .unwrap();
        }
    }
    let mut app = MixJamApp::from_session(
        Session::headless(library, editor.into_project()).unwrap(),
        "Name parity".into(),
        false,
    );
    app.capture_path = None;
    app.options = None;
    app.session.set_theme_fixture(theme::current());
    (context, app)
}

fn run(context: &egui::Context, app: &mut MixJamApp, events: Vec<egui::Event>) -> egui::FullOutput {
    context.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ctx| app.draw(ctx),
    )
}

/// The `role<TAB>name` lines of one state, as the agent `ui` request names them.
fn names(context: &egui::Context, app: &mut MixJamApp, found: &mut BTreeSet<String>) {
    run(context, app, Vec::new());
    let output = run(context, app, Vec::new());
    let update = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled for this test.");
    for node in agent::nodes(update) {
        let label = node.label.trim();
        if !label.is_empty() && !ALBUM_DATA.iter().any(|prefix| label.starts_with(prefix)) {
            let label = label.replace(mixjam_techno::application::APP_VERSION, "{version}");
            found.insert(format!(
                "{}\t{}",
                node.role,
                label.replace(['\n', '\t'], " ")
            ));
        }
    }
}

fn key(key: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]
}

/// Every name of every state that the agent baseline of phase 0 visited.
fn every_name() -> BTreeSet<String> {
    let mut found = BTreeSet::new();

    let (context, mut empty) = app(false);
    names(&context, &mut empty, &mut found);

    let (context, mut app) = app(true);
    let section = |bar: u32, name: &str| crate::engine::Section {
        start_tick: bar * TICKS_PER_BAR,
        name: name.into(),
    };
    app.session
        .edit(EditCommand::SetSections(vec![
            section(0, "Intro"),
            section(8, "Drop"),
        ]))
        .unwrap();
    names(&context, &mut app, &mut found);

    let clip = app.session.project().clips[0].id;
    app.selected_clip = Some(clip);
    app.selected_clips = [clip].into_iter().collect();
    names(&context, &mut app, &mut found);
    // What Shift+F10 sets on a selected clip.
    app.clip_menu_key = true;
    names(&context, &mut app, &mut found);
    run(&context, &mut app, key(egui::Key::Escape));
    app.selected_clip = None;
    app.selected_clips.clear();

    for lane in automation::Lane::ALL {
        app.automation_lane = Some(lane);
        names(&context, &mut app, &mut found);
    }
    app.automation_lane = None;
    for bus in 0..mixjam_techno::application::fx::BUSES {
        app.return_lane = Some(bus);
        names(&context, &mut app, &mut found);
    }
    app.return_lane = None;

    for module in BottomModule::ALL {
        app.bottom_module = module;
        names(&context, &mut app, &mut found);
    }
    app.bottom_module = BottomModule::Mixer;
    app.eq_track = Some(0);
    names(&context, &mut app, &mut found);
    app.eq_track = None;
    app.bottom_module = BottomModule::Sounds;

    app.selected_sample = Some(1);
    let windows: [fn(&mut MixJamApp) -> &mut bool; 11] = [
        |app| &mut app.show_effects,
        |app| &mut app.show_step_synth,
        |app| &mut app.show_sample_edit,
        |app| &mut app.show_stretch,
        |app| &mut app.show_album_review,
        |app| &mut app.show_help,
        |app| &mut app.show_effects_help,
        |app| &mut app.show_step_synth_help,
        |app| &mut app.show_sample_edit_help,
        |app| &mut app.show_stretch_help,
        |app| &mut app.show_master_help,
    ];
    for window in windows {
        *window(&mut app) = true;
        names(&context, &mut app, &mut found);
        *window(&mut app) = false;
    }

    for page in [
        options::Page::Audio,
        options::Page::Workspace,
        options::Page::Files,
        options::Page::Libraries,
    ] {
        app.options = Some(options::OptionsDraft::new(&app.session, page));
        names(&context, &mut app, &mut found);
    }
    app.options = None;

    app.pending_action = Some(PendingAction::New);
    names(&context, &mut app, &mut found);
    app.pending_action = None;
    app.path_prompt = Some(PathPrompt {
        request: FileRequest::ImportSound,
        open_token: None,
        text: String::new(),
        error: None,
        focused: false,
    });
    names(&context, &mut app, &mut found);
    app.path_prompt = None;
    app.import_path = Some(PathBuf::from("parity.wav"));
    names(&context, &mut app, &mut found);
    found
}

#[test]
fn name_parity_every_baseline_name_still_exists_or_has_an_approved_rename() {
    let found = every_name();
    if std::env::var_os("MIXJAM_TECHNO_WRITE_UI_NAMES").is_some() {
        let text: String = found.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(BASELINE_PATH, text).unwrap();
        return;
    }
    let renamed: BTreeMap<&str, &str> = RENAMED.iter().copied().collect();
    let missing: Vec<String> = BASELINE
        .lines()
        .filter(|line| !line.is_empty())
        .filter(|line| match renamed.get(line) {
            Some(new) => !found.contains(*new),
            None => !found.contains(*line),
        })
        .map(str::to_owned)
        .collect();
    assert!(
        missing.is_empty(),
        "{} accessible names from ui-names.txt are gone; give each a new home or an approved \
         rename:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn name_parity_states_are_deterministic() {
    assert_eq!(every_name(), every_name());
}
