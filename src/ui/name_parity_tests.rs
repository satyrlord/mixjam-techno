//! Guard for the layout migration: every accessible name that the main window,
//! its lower-panel modules, its tool windows and its modals expose today must
//! still exist after a change, or be listed in `RENAMED` with the name that
//! replaces it. `ui-names.txt` holds the names at the start of the migration.
//!
//! Regenerate the list only after the owner approves a removal:
//! `MIXJAM_TECHNO_WRITE_UI_NAMES=1 cargo test name_parity`.

use super::*;
use mixjam_techno::application::album_review::{
    Shelf,
    covers::{CACHE_LIMIT_BYTES, CoverCache},
};
use mixjam_techno::library::Library;

const BASELINE: &str = include_str!("ui-names.txt");
const BASELINE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/ui-names.txt");

/// Album Review buttons named after the bundled reviews: data, not controls.
const ALBUM_DATA: &[&str] = &["Open review: ", "Artist filter: ", "Genre filter: "];

/// Approved renames: an old `role<TAB>name` line and the line that replaces it.
const RENAMED: &[(&str, &str)] = &[
    // Phase 6, decision B: the board has no window; the rail button folds it.
    ("button\tClose Step synth", "button\tStep synth"),
    ("button\tCollapse Step synth", "button\tStep synth"),
    // Phase 6: the rail buttons replace the module selector; the board tabs
    // that once sat beside them are gone (owner, 2026-11-15).
    ("combo_box\tBottom module", "button\tMixer"),
    (
        "label\tMastering processes the whole mix after the tracks are summed. Choose it in the lower panel's module selector, or with its button in the tool rail. The signal flows from left to right: the mixer's output enters the Input unit, passes each pedal that is on, and leaves through the Output unit. Playback and export both use it, so what you hear is what you export.",
        "label\tMastering processes the whole mix after the tracks are summed. Choose it with its button in the tool rail. The signal flows from left to right: the mixer's output enters the Input unit, passes each pedal that is on, and leaves through the Output unit. Playback and export both use it, so what you hear is what you export.",
    ),
    // Decision L: Effects becomes FX returns.
    ("other\tEffects board", "other\tFX returns board"),
    // The window title becomes the board title in the lower panel's top row.
    ("other\tSTEP SYNTH", "label\tSTEP SYNTH"),
    // Step synth's help describes its board and rail action.
    // The source keys' names are their visible text.
    ("button\tUse palette sound", "button\tPalette sound"),
    ("button\tChoose source file", "button\tSource file…"),
    (
        "label\tThe upper-left triangle collapses the tool without changing its draft. Tab reaches each control and the note grid. In the grid, arrow keys move between cells, Home and End jump to the first and last step, and Space or Enter sets or clears the note; the focused cell scrolls into view. Esc first cancels an active value or list, or goes to a window above the tool.",
        "label\tThe Step synth button in the tool rail folds the lower panel without changing the draft; choose it again to open the board. Tab reaches each control and the note grid. In the grid, arrow keys move between cells, Home and End jump to the first step in view and the last step in view that plays, Ctrl with Left or Right turns the bar page when another page is there, Page Up and Page Down shift the octave, and Space or Enter places or clears the note; the focused cell scrolls into view. On the ruler and value lanes, Left and Right move the shared step cursor, and Home and End jump to the same steps as in the grid. On the ruler, Shift with Left or Right selects a range; Space or Enter selects the current step. On Accent and Slide, Space or Enter toggles the flag, and Up or Down sets or clears it. On Velocity, Up or Down changes the level by one. Lane edits also apply to the selected notes. Esc first cancels an active value or list, or goes to a window above the board.",
    ),
    // Phase 6c: the Sounds panel's count replaces the Sounds module's.
    ("label\t4 sounds", "label\t4 of 4"),
    // Phase 10: the Step synth pane switch no longer shares the rail's name;
    // Notes and Effects in the top row replace it.
    (
        "label\tEffects replaces the note grid with sequence effects. Note grid returns to editing. Save pattern and Load pattern keep the editable four-bar draft in JSON; they do not replace the source file or viewed bar.",
        "label\tNotes and Effects, at the left of the top row, switch between the note grid and the pattern effects; while Listen plays, the strip beside it lights the playing step. The pattern plays as a loop, so the echo and filter tails at its end come back at its start. The undo and redo keys after them take back and put back the board's last edit, and the status line names it. Load and Save, under Pattern, keep the editable four-bar draft in JSON with its source file; Load switches to that source, or to the palette sound when the file is missing, and keeps the viewed bar. A loaded tempo that is not one of the choices moves to the nearest one, and the status line says so.",
    ),
    // The step model: Length replaces the Active bar switches and Gate the
    // 1/16 cut (owner decision 1, 2026-11-15).
    ("checkbox\t1/16 cut", "slider\tGate"),
    ("checkbox\tActive 1", "slider\tLength"),
    ("checkbox\tActive 2", "slider\tLength"),
    ("checkbox\tActive 3", "slider\tLength"),
    ("checkbox\tActive 4", "slider\tLength"),
    (
        "label\tChoose a WAV or PXD source file, or use the selected palette sound or clip. Listen sequence previews the active bars. Save sequence writes a WAV to Step synth at the sequence tempo.",
        "label\tSource file… chooses a WAV or PXD source file, and Palette sound uses the selected palette sound or clip. Listen plays the pattern and Stop ends it; L does the same while the board shows. Save sound writes a WAV to Step synth at the pattern's tempo. Start is where each note starts to read the source, in ms; Find start sets it to the source's first hit, so the notes play on the beat. A new source sets Start to its first hit and turns Loop source off; a loaded pattern keeps its own. A pattern that would clip plays and saves turned down as a whole, and the status line says by how much.",
    ),
    (
        "label\tView bar changes the bar you edit. Active switches choose which bars render, in ascending order. Clear bar clears only the viewed bar. At least one bar must be active to render or save a pattern.",
        "label\tThe grid shows four, two, or one bar, as many as the board's width allows; the bar buttons bring a bar into view. Click a cell to place a note and click a note to clear it. Drag a note's last step sideways to hold it longer. Shift-click a note to accent it and Alt-click it to slide into the next note, or use the Accent and Slide lanes. Drag on the Velocity lane to set levels. Drag along the step ruler to select steps; lane edits then apply to all of them. Length, under Pattern, sets how many steps the pattern repeats over; drag its end marker on the ruler. The steps after it keep their notes. ×2 and ÷2 double and halve it; ×2 repeats the loop into the new steps when they are empty. Copy to copies the viewed bar, the one picked last, onto the next bar, Fill copies it onto every bar, and Clear bar empties it; Clear, under Pattern, empties every bar. With the pointer over the board, or the keyboard focus in it, Ctrl+Z takes back the board's last edit, of a note, setting, effect, source, or load, and Ctrl+Y or Ctrl+Shift+Z puts it back; elsewhere they act on the song.",
    ),
    (
        "label\t1/16 cut ends a trigger after one sixteenth. Without it, the source continues until the next trigger or sequence end. Loop source repeats audio during that interval. The tempo choices are 120, 125, 130, 135, 140, 160, and 180 BPM.",
        "label\tSwing delays every second sixteenth, from 50% (straight) to 75%. Gate sets how much of its last step a note sounds for; a note that slides into the next ignores it. With swing, the step is the swung one, so a note on the beat sounds longer than one off the beat. Accent sets how much louder an accented note plays, up to 12 dB, and Slide time how long a slide takes to reach the next pitch, from 1 to 1000 ms. Every note ends with a short fade, so a cut does not click. Loop source, in the Source section, repeats audio while a note sounds. The tempo choices are 120, 125, 130, 135, 140, 160, and 180 BPM.",
    ),
    (
        "label\tEach of four bars has sixteen steps. Click a cell to set its pitch, replace that step's note, or click the same note again to clear it. Scroll the pitch grid to reach all notes from +12 to -12 semitones. A 0 keeps the source pitch.",
        "label\tThe pattern has four bars of sixteen steps. The grid shows 25 pitch rows, +12 to -12 semitones around the octave in view, and the olive 0 row keeps the source pitch. The grid opens on the octave nearest the notes. The − and + keys beside the step ruler, with the octave between them, or Page Up and Page Down, shift the rows by up to three octaves; a coral edge marks a note above or below them. Scale and its root, A to G# with A as the source pitch, in the grid's top row, shade the rows outside the scale but never block a pitch. Randomise, beside them, writes a new line in the scale over the pattern's length.",
    ),
    (
        "label\tClick a note to set it. Click it again to clear it.",
        "label\tClick places or clears a note. Drag a note's end to hold it.",
    ),
    // One noun each: the pattern is the notes, the sound the WAV. Listen
    // plays and stops the pattern from the grid's top row (owner,
    // 2026-11-15).
    ("button\tListen sequence", "button\tListen"),
    ("button\tSave sequence", "button\tSave sound"),
];

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

/// Keep the baseline's cache summary independent of the owner's downloaded covers.
struct ParityCache(CoverCache);

impl ParityCache {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(format!(
                "name-parity-covers-{}-{serial}",
                std::process::id()
            ));
        let cache = CoverCache::new(folder, CACHE_LIMIT_BYTES);
        let mut png = include_bytes!("../../assets/rail/album-review.png").to_vec();
        png.resize(200 * 1024, 0);
        for id in ["parity-first", "parity-second"] {
            cache.store(id, &png).unwrap();
        }
        assert_eq!(cache.usage(), (2, 400 * 1024));
        Self(cache)
    }
}

impl Drop for ParityCache {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.folder());
    }
}

fn app(with_clips: bool, cache: &CoverCache) -> (egui::Context, MixJamApp) {
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
    app.capture.path = None;
    app.options = None;
    app.album_review.state.use_folder(
        Shelf::open_default().folder().map(Path::to_path_buf),
        cache.clone(),
    );
    app.session.set_theme_fixture(theme::current());
    // A default launch folds the lower panel (Starting module Sounds); the
    // walk opens it to reach the boards.
    app.set_layout(|layout| layout.boards = true);
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
    let cache = ParityCache::new();

    let (context, mut empty) = app(false, &cache.0);
    names(&context, &mut empty, &mut found);

    let (context, mut app) = app(true, &cache.0);
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
    app.selection.clip = Some(clip);
    app.selection.clips = [clip].into_iter().collect();
    names(&context, &mut app, &mut found);
    // What Shift+F10 sets on a selected clip.
    app.clip_menu.by_key = true;
    names(&context, &mut app, &mut found);
    run(&context, &mut app, key(egui::Key::Escape));
    app.selection.clip = None;
    app.selection.clips.clear();

    for lane in automation::Lane::ALL {
        app.score_tools.automation_lane = Some(lane);
        names(&context, &mut app, &mut found);
    }
    app.score_tools.automation_lane = None;
    for bus in 0..mixjam_techno::application::fx::BUSES {
        app.score_tools.return_lane = Some(bus);
        names(&context, &mut app, &mut found);
    }
    app.score_tools.return_lane = None;

    for module in BottomModule::ALL {
        app.boards.module = module;
        names(&context, &mut app, &mut found);
    }
    // Step synth with a source, as its former window was visited.
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    names(&context, &mut app, &mut found);
    app.selection.sample = None;
    app.boards.module = BottomModule::Mixer;
    app.eq_track = Some(0);
    names(&context, &mut app, &mut found);
    app.eq_track = None;

    app.selection.sample = Some(1);
    let windows: [fn(&mut MixJamApp) -> &mut bool; 10] = [
        |app| &mut app.effects.open,
        |app| &mut app.sample_edit.open,
        |app| &mut app.stretch.open,
        |app| &mut app.album_review.open,
        |app| &mut app.help.main,
        |app| &mut app.help.effects,
        |app| &mut app.help.step_synth,
        |app| &mut app.help.sample_edit,
        |app| &mut app.help.stretch,
        |app| &mut app.help.master,
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

    app.files.pending_action = Some(PendingAction::New);
    names(&context, &mut app, &mut found);
    app.files.pending_action = None;
    app.files.path_prompt = Some(PathPrompt {
        request: FileRequest::ImportSound,
        open_token: None,
        text: String::new(),
        error: None,
        focused: false,
    });
    names(&context, &mut app, &mut found);
    app.files.path_prompt = None;
    app.sample_import.path = Some(PathBuf::from("parity.wav"));
    names(&context, &mut app, &mut found);
    found
}

/// A pitch grid cell one semitone from the source says "semitone".
fn singular_semitone(line: &str) -> String {
    for (plural, singular) in [
        ("+1 semitones", "+1 semitone"),
        ("-1 semitones", "-1 semitone"),
    ] {
        if let Some(head) = line.strip_suffix(plural) {
            return format!("{head}{singular}");
        }
    }
    line.to_owned()
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
            None => !found.contains(&singular_semitone(line)),
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
