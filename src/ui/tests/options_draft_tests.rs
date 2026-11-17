//! The Options draft as a plain controller: facts written by hand, actions
//! in, effects out. No egui, no Session, no files.

use super::*;
use mixjam_techno::application::options::{
    AppOptions, PlaybackBufferTarget, STALE_DRAFT, SongFolderMode,
};
use options::labels;
use options::{Action, Effect, OptionsDraft, OptionsFacts, Page};

/// Facts for a session whose active options are the defaults.
fn facts() -> OptionsFacts {
    OptionsFacts {
        revision: 3,
        ..OptionsFacts::default()
    }
}

/// A draft on `page` with one Workspace edit.
fn edited(facts: &OptionsFacts) -> OptionsDraft {
    let mut draft = OptionsDraft::new(facts, Page::Workspace);
    draft.values.show_section_bookmarks = !draft.values.show_section_bookmarks;
    draft
}

#[test]
fn a_new_draft_is_clean_and_apply_says_why_it_waits() {
    let facts = facts();
    let draft = OptionsDraft::new(&facts, Page::Files);
    assert_eq!(draft.page, Page::Files);
    assert!(!draft.pending(&facts));
    assert_eq!(draft.changes(&facts), [0, 0, 0]);
    assert_eq!(draft.status(&facts), "");
    assert_eq!(
        draft.apply_blocked(&facts).as_deref(),
        Some(labels::NOTHING_TO_APPLY)
    );
}

#[test]
fn changes_are_counted_on_the_page_that_shows_them() {
    let facts = facts();
    let mut draft = OptionsDraft::new(&facts, Page::Audio);
    draft.values.preview_muted = true;
    draft.values.preview_level_db = -12.0;
    draft.values.show_tooltips = false;
    draft.values.song_folder_mode = SongFolderMode::Fixed;
    draft.values.fixed_song_folder = Some(std::env::temp_dir());
    assert_eq!(draft.changes(&facts), [2, 1, 2]);
    assert_eq!(draft.status(&facts), labels::unapplied(5));
    assert_eq!(labels::unapplied(1), "1 unapplied change");
    assert_eq!(draft.apply_blocked(&facts), None);
}

#[test]
fn apply_is_blocked_by_a_stale_draft_an_invalid_value_and_a_busy_device() {
    let mut facts = facts();
    let mut draft = edited(&facts);
    // An invalid folder blocks Apply with the same words the page shows.
    draft.values.song_folder_mode = SongFolderMode::Fixed;
    draft.values.fixed_song_folder = Some(PathBuf::from("relative"));
    let invalid = draft.values.validate().unwrap_err();
    assert_eq!(draft.invalid().as_ref(), Some(&invalid));
    assert_eq!(draft.apply_blocked(&facts), Some(invalid));
    assert_eq!(draft.update(Action::Apply, &facts), None);
    // Audio settings wait while the device is busy, others do not.
    draft.values = facts.active.clone();
    draft.values.show_tooltips = false;
    facts.audio_blocked = Some("An export is running.".into());
    assert_eq!(draft.audio_blocked(&facts), None);
    assert_eq!(draft.apply_blocked(&facts), None);
    draft.values.preview_muted = true;
    assert_eq!(draft.audio_blocked(&facts), Some("An export is running."));
    assert_eq!(
        draft.apply_blocked(&facts).as_deref(),
        Some("An export is running.")
    );
    // A newer active revision makes the draft stale before anything else.
    facts.revision += 1;
    assert!(draft.stale(&facts));
    assert_eq!(draft.apply_blocked(&facts).as_deref(), Some(STALE_DRAFT));
}

#[test]
fn apply_hands_the_values_to_the_session_and_applied_reloads_them() {
    let mut facts = facts();
    let mut draft = edited(&facts);
    let values = draft.values.clone();
    assert_eq!(
        draft.update(Action::Apply, &facts),
        Some(Effect::Apply {
            values: values.clone(),
            revision: 3,
            restored: false,
            close: false,
        })
    );
    // Nothing changes until the Session says so.
    assert!(draft.pending(&facts));
    facts.active = values;
    facts.revision = 4;
    draft.applied(&facts);
    assert!(!draft.pending(&facts) && !draft.stale(&facts));
    assert_eq!(draft.page, Page::Workspace);
    assert_eq!(draft.status(&facts), labels::APPLIED);
    draft.failed("The disk is full.".into());
    assert_eq!(draft.error.as_deref(), Some("The disk is full."));
}

#[test]
fn close_asks_first_only_when_there_is_something_to_lose() {
    let facts = facts();
    let mut clean = OptionsDraft::new(&facts, Page::Audio);
    assert_eq!(clean.update(Action::Close, &facts), Some(Effect::Close));
    let mut draft = edited(&facts);
    assert_eq!(draft.update(Action::Close, &facts), None);
    assert!(draft.confirm_discard);
    assert_eq!(draft.update(Action::KeepEditing, &facts), None);
    assert!(!draft.confirm_discard && draft.pending(&facts));
    draft.update(Action::Close, &facts);
    assert_eq!(draft.update(Action::Discard, &facts), Some(Effect::Close));
    assert!(!draft.confirm_discard);
    // Apply changes from the prompt applies, then closes.
    draft.update(Action::Close, &facts);
    assert!(matches!(
        draft.update(Action::ApplyAndClose, &facts),
        Some(Effect::Apply { close: true, .. })
    ));
    assert!(!draft.confirm_discard);
}

#[test]
fn a_prompt_goes_when_its_draft_becomes_clean() {
    let facts = facts();
    let mut draft = edited(&facts);
    draft.update(Action::Close, &facts);
    draft.settle(&facts);
    assert!(draft.confirm_discard, "a dirty draft keeps its prompt");
    draft.values = facts.active.clone();
    draft.settle(&facts);
    assert!(!draft.confirm_discard);
}

#[test]
fn restore_reload_and_page_actions_stay_in_the_draft() {
    let mut facts = facts();
    facts.active.show_tooltips = false;
    let mut draft = OptionsDraft::new(&facts, Page::Audio);
    assert_eq!(draft.update(Action::Show(Page::Files), &facts), None);
    assert_eq!(draft.page, Page::Files);
    draft.update(Action::Restore, &facts);
    assert_eq!(draft.values, AppOptions::default());
    assert!(draft.restored && draft.pending(&facts));
    assert_eq!(draft.changes(&facts), [0, 1, 0]);
    draft.update(Action::Reload, &facts);
    assert!(!draft.restored && !draft.pending(&facts));
    assert_eq!(draft.page, Page::Files, "Reload keeps the page");
    for (action, effect) in [
        (Action::PickFolder, Effect::PickFolder),
        (Action::SaveCapture, Effect::SaveCapture),
        (Action::OpenLibraries, Effect::OpenLibraries),
    ] {
        assert_eq!(draft.update(action, &facts), Some(effect));
    }
    assert!(!draft.pending(&facts));
}

#[test]
fn restored_defaults_stay_pending_while_they_replace_an_invalid_file() {
    let facts = OptionsFacts {
        invalid: true,
        ..facts()
    };
    let mut draft = OptionsDraft::new(&facts, Page::Audio);
    assert!(!draft.pending(&facts));
    draft.update(Action::Restore, &facts);
    assert!(draft.pending(&facts));
    assert_eq!(draft.changes(&facts), [0; 3]);
    assert_eq!(draft.status(&facts), labels::DEFAULTS_PENDING);
    assert!(matches!(
        draft.update(Action::Apply, &facts),
        Some(Effect::Apply { restored: true, .. })
    ));
}

#[test]
fn a_buffer_target_error_belongs_to_its_target() {
    let facts = OptionsFacts {
        buffer_errors: vec![(PlaybackBufferTarget::Ms20, "Too short.".into())],
        ..facts()
    };
    assert_eq!(
        facts.buffer_error(PlaybackBufferTarget::Ms20),
        Some("Too short.")
    );
    assert_eq!(facts.buffer_error(PlaybackBufferTarget::Ms40), None);
}
