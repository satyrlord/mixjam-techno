//! The Options pages as free functions. Each takes the draft's values and
//! the frame's facts, edits the values in place, and reports any other
//! request as an `Action`. None of them reads the Session or the app, so a
//! page can be drawn and tested on a bare context.
use super::super::form::{self, choice, help, section, switch};
use super::super::*;
use super::draft::{Action, OptionsDraft, OptionsFacts, Page, validation_error};
use super::labels::*;
use mixjam_techno::application::options::{AppOptions, SongFolderMode, Theme};

/// The page on show.
pub(crate) fn page(
    ui: &mut egui::Ui,
    page: Page,
    values: &mut AppOptions,
    facts: &OptionsFacts,
    actions: &mut Vec<Action>,
) {
    match page {
        Page::Audio => audio(ui, values, facts, actions),
        Page::Workspace => workspace(ui, values),
        Page::Files => files(ui, values, facts, actions),
    }
}

/// What a page shows above its controls: a stale draft, and why the file or
/// the last Apply failed.
pub(crate) fn notices(
    ui: &mut egui::Ui,
    draft: &OptionsDraft,
    facts: &OptionsFacts,
    actions: &mut Vec<Action>,
) {
    if draft.stale(facts) {
        form::warning(ui, mixjam_techno::application::options::STALE_DRAFT);
        if form::button(ui, RELOAD, controls::ButtonRole::Secondary).clicked() {
            actions.push(Action::Reload);
        }
        ui.separator();
    }
    if let Some(error) = &facts.error {
        form::error(ui, error);
    }
    if let Some(error) = &draft.error {
        form::error(ui, error);
    }
    if let Some(reason) = draft.audio_blocked(facts) {
        form::warning(ui, reason);
    }
}

pub(crate) fn audio(
    ui: &mut egui::Ui,
    values: &mut AppOptions,
    facts: &OptionsFacts,
    actions: &mut Vec<Action>,
) {
    section(ui, PLAYBACK_QUALITY);
    help(
        ui,
        "Lower settings use less CPU during playback. Exports always use full quality.",
    );
    choice(
        ui,
        DRIVE_OVERSAMPLING,
        &mut values.drive_playback_oversampling,
        &OVERSAMPLING,
    );
    help(
        ui,
        "4x keeps track drive saturation clean. Off can sound harsher.",
    );
    choice(
        ui,
        MASTER_OVERSAMPLING,
        &mut values.master_playback_oversampling,
        &OVERSAMPLING,
    );
    help(
        ui,
        "4x keeps Soft clip, Tube, Maximizer, and Tape clean. Off can sound harsher.",
    );
    choice(
        ui,
        SAMPLE_QUALITY,
        &mut values.sample_playback_quality,
        &SAMPLE_QUALITIES,
    );
    help(
        ui,
        "High reduces unwanted tones at changed tempo or rate. Low uses less CPU.",
    );
    if values.quality_reduced() {
        form::warning(ui, QUALITY_REDUCED);
    }
    ui.add_space(8.0);
    buffer_target(ui, values, facts);
    help(
        ui,
        "A larger buffer absorbs processing bursts but adds delay between a control and its sound.",
    );
    // The request and the measured callback, apart from each other (Options
    // spec), out of the way until someone needs them.
    form::disclosure(
        ui,
        controls::disclosure(DEVICE_DETAILS).default_open(false),
        |ui| {
            ui.label(
                egui::RichText::new(&facts.buffer_status)
                    .size(theme::type_size::CLIP)
                    .color(roles().help),
            );
            if form::button(ui, SAVE_CAPTURE, controls::ButtonRole::Secondary).clicked() {
                actions.push(Action::SaveCapture);
            }
            help(
                ui,
                "Saves the last 30 seconds sent to the audio device, with the song's state, \
                 to the diagnostics folder in the data folder. F12 does the same anywhere.",
            );
        },
    );
    ui.separator();
    section(ui, MONITORING);
    ui.horizontal(|ui| {
        ui.label(PREVIEW_LEVEL);
        let level = controls::slider(ui, &mut values.preview_level_db, -30.0..=0.0, PREVIEW_LEVEL);
        form::register(&level);
        ui.ctx().accesskit_node_builder(level.id, |node| {
            node.set_description("Preview output level in dB, from -30 to 0.")
        });
        ui.label(format!("{:.1} dB", values.preview_level_db));
    });
    switch(ui, MUTE_PREVIEWS, &mut values.preview_muted);
    choice(ui, METER_RATE, &mut values.meter_display_rate, &METER_RATES);
    help(
        ui,
        "Reduced draws meters at most 15 times a second. Measurement and peaks stay exact.",
    );
}

/// The buffer target's choice box. A target that the device cannot use is
/// disabled, with its reason under it.
fn buffer_target(ui: &mut egui::Ui, values: &mut AppOptions, facts: &OptionsFacts) {
    ui.label(BUFFER_TARGET);
    let selected = BUFFER_TARGETS
        .iter()
        .find(|(target, _)| *target == values.playback_buffer_target)
        .map_or("", |(_, label)| *label);
    let buffer = egui::ComboBox::from_id_salt("playback-buffer-target")
        .width(form::CHOICE_WIDTH)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (target, label) in BUFFER_TARGETS {
                let reason = facts.buffer_error(target);
                let response = ui.add_enabled(
                    reason.is_none(),
                    egui::Button::selectable(values.playback_buffer_target == target, label),
                );
                if response.clicked() {
                    values.playback_buffer_target = target;
                }
                if let Some(reason) = reason {
                    response.disabled_hint(reason);
                    ui.small(reason);
                }
            }
        });
    form::choice_named(&buffer.response, BUFFER_TARGET, selected);
    if let Some(reason) = facts.buffer_error(values.playback_buffer_target) {
        form::error(ui, reason);
    }
}

pub(crate) fn workspace(ui: &mut egui::Ui, values: &mut AppOptions) {
    section(ui, WORKSPACE);
    choice(
        ui,
        THEME,
        &mut values.theme,
        &Theme::ALL.map(|each| (each, theme(each))),
    );
    help(
        ui,
        "Studio is plain and flat, Aero is colorful plastic, and Vintage is worn painted \
         metal. Only the look changes. Apply switches it, even while a song plays.",
    );
    choice(
        ui,
        LANE_HEIGHT,
        &mut values.lane_height,
        &mixjam_techno::application::options::LaneHeight::ALL
            .map(|height| (height, lane_height(height))),
    );
    help(
        ui,
        "The height of each arrangement row. Apply resizes the rows, even while a song \
         plays.",
    );
    switch(ui, FOLLOW_ON_STARTUP, &mut values.follow_on_startup);
    help(
        ui,
        "From the next launch. The Follow button sets this session.",
    );
    switch(ui, SECTION_BOOKMARKS, &mut values.show_section_bookmarks);
    help(ui, "The same setting as the Sections button.");
    choice(
        ui,
        STARTING_MODULE,
        &mut values.starting_module,
        &STARTING_MODULES,
    );
    help(ui, "At the next launch. Opening a song keeps the module.");
    switch(ui, SHOW_TOOLTIPS, &mut values.show_tooltips);
    help(
        ui,
        "Hover hints only. Names, focus, and errors stay available.",
    );
}

pub(crate) fn files(
    ui: &mut egui::Ui,
    values: &mut AppOptions,
    facts: &OptionsFacts,
    actions: &mut Vec<Action>,
) {
    section(ui, SONG_FOLDERS);
    choice(ui, FOLDER_MODE, &mut values.song_folder_mode, &FOLDER_MODES);
    help(
        ui,
        "Where Open and Save as start. Existing songs stay in place.",
    );
    if values.song_folder_mode == SongFolderMode::Fixed {
        ui.label(FIXED_FOLDER);
        let mut path = values
            .fixed_song_folder
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        let response =
            ui.add(egui::TextEdit::singleline(&mut path).desired_width(ui.available_width()));
        named(&response, egui::WidgetType::TextEdit, FIXED_FOLDER);
        form::register(&response);
        if response.changed() {
            values.fixed_song_folder = (!path.is_empty()).then(|| PathBuf::from(path));
        }
        if form::button(ui, CHOOSE_FOLDER, controls::ButtonRole::Secondary).clicked() {
            actions.push(Action::PickFolder);
        }
        // The same check that keeps Apply disabled, where the folder is typed.
        match validation_error(values, &facts.active) {
            Some(reason) => form::error(ui, reason),
            None => help(ui, "An existing folder, chosen or typed as a full path."),
        }
    }
    if let Some(warning) = &facts.folder_warning {
        form::error(ui, warning);
    }
    ui.separator();
    section(ui, LIBRARIES);
    help(
        ui,
        "Library folders fill the Sounds panel. Changes there take effect right away.",
    );
    if form::button(ui, OPEN_LIBRARIES, controls::ButtonRole::Secondary).clicked() {
        actions.push(Action::OpenLibraries);
    }
}
