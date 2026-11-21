//! The Options draft as a plain controller, with no egui and no Session.
//!
//! The window reads `OptionsFacts` from the Session once per frame, passes
//! each of the person's `Action`s to `OptionsDraft::update`, and carries out
//! the `Effect`s that come back. Tests drive the same calls with facts they
//! write themselves.
use super::labels;
use mixjam_techno::application::options::{
    AppOptions, PlaybackBufferTarget, STALE_DRAFT, SongFolderMode,
};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

/// The pages of the Options window, in their selector's order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Page {
    #[default]
    Audio,
    Workspace,
    Files,
}

impl Page {
    pub(crate) const ALL: [Self; 3] = [Self::Audio, Self::Workspace, Self::Files];

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// What the Session says this frame, as plain values.
#[derive(Clone, Debug, Default)]
pub(crate) struct OptionsFacts {
    /// The active options.
    pub(crate) active: AppOptions,
    /// The active options' revision; a draft from an older one is stale.
    pub(crate) revision: u64,
    /// The options file could not be read, so defaults are in use.
    pub(crate) invalid: bool,
    /// Why the options file could not be read or written, if it could not.
    pub(crate) error: Option<String>,
    /// Why audio settings cannot change now, such as during an export.
    pub(crate) audio_blocked: Option<String>,
    /// Each buffer target that the device cannot use, and why.
    pub(crate) buffer_errors: Vec<(PlaybackBufferTarget, String)>,
    /// The requested and measured buffer, for Device details.
    pub(crate) buffer_status: String,
    /// A problem with the song folder setting that is already active.
    pub(crate) folder_warning: Option<String>,
}

impl OptionsFacts {
    pub(crate) fn buffer_error(&self, target: PlaybackBufferTarget) -> Option<&str> {
        self.buffer_errors
            .iter()
            .find(|(each, _)| *each == target)
            .map(|(_, error)| error.as_str())
    }
}

/// What the person did in the window or its prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Show(Page),
    /// Put every page's defaults in the draft.
    Restore,
    /// Start again from the active options.
    Reload,
    Apply,
    /// Close, Escape, or Cancel.
    Close,
    /// The prompt's Apply changes: apply, then close.
    ApplyAndClose,
    /// The prompt's Discard changes.
    Discard,
    /// The prompt's Keep editing, or Escape in the prompt.
    KeepEditing,
    PickFolder,
    SaveCapture,
    OpenLibraries,
}

/// What the window must do for an action.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Effect {
    /// Ask the Session to apply these values, then close if `close`.
    Apply {
        values: AppOptions,
        revision: u64,
        restored: bool,
        close: bool,
    },
    Close,
    PickFolder,
    SaveCapture,
    OpenLibraries,
}

/// The values being edited, and the state of their window.
#[derive(Clone, Debug)]
pub(crate) struct OptionsDraft {
    pub(crate) values: AppOptions,
    /// The revision that the draft started from.
    pub(crate) revision: u64,
    pub(crate) page: Page,
    /// Restore put defaults in the draft, which replace an invalid file.
    pub(crate) restored: bool,
    /// Why the last Apply failed.
    pub(crate) error: Option<String>,
    /// The last Apply succeeded and nothing has changed since.
    pub(crate) saved: bool,
    /// Close found unapplied edits and asks first.
    pub(crate) confirm_discard: bool,
}

/// Unapplied edits per page, in `Page::ALL` order. The destructuring is
/// exhaustive, so a new option field must be given a page here.
fn page_changes(draft: &AppOptions, active: &AppOptions) -> [usize; 3] {
    let AppOptions {
        schema_version: _,
        drive_playback_oversampling,
        master_playback_oversampling,
        sample_playback_quality,
        preview_level_db,
        preview_muted,
        playback_buffer_target,
        meter_display_rate,
        follow_on_startup,
        show_section_bookmarks,
        starting_module,
        show_tooltips,
        theme,
        lane_height,
        song_folder_mode,
        fixed_song_folder,
    } = draft;
    let count = |changes: &[bool]| changes.iter().filter(|changed| **changed).count();
    [
        count(&[
            *drive_playback_oversampling != active.drive_playback_oversampling,
            *master_playback_oversampling != active.master_playback_oversampling,
            *sample_playback_quality != active.sample_playback_quality,
            *preview_level_db != active.preview_level_db,
            *preview_muted != active.preview_muted,
            *playback_buffer_target != active.playback_buffer_target,
            *meter_display_rate != active.meter_display_rate,
        ]),
        count(&[
            *follow_on_startup != active.follow_on_startup,
            *show_section_bookmarks != active.show_section_bookmarks,
            *starting_module != active.starting_module,
            *show_tooltips != active.show_tooltips,
            *theme != active.theme,
            *lane_height != active.lane_height,
        ]),
        count(&[
            *song_folder_mode != active.song_folder_mode,
            *fixed_song_folder != active.fixed_song_folder,
        ]),
    ]
}

/// How long the answer about a fixed song folder holds. The window asks
/// every frame, and a slow or disconnected folder must not stall each one.
const FOLDER_CHECK_LIFE: Duration = Duration::from_secs(1);

/// The last answer about a fixed song folder.
struct FolderCheck {
    values: AppOptions,
    at: Instant,
    error: Option<String>,
}

thread_local! {
    static LAST_FOLDER_CHECK: RefCell<Option<FolderCheck>> = const { RefCell::new(None) };
}

/// Drop the held answer, so the next check asks the file system.
pub(in super::super) fn forget_folder_check() {
    LAST_FOLDER_CHECK.with_borrow_mut(|last| *last = None);
}

/// Time left until a cached folder failure expires, so the window can wake
/// and re-check an idle draft.
pub(super) fn folder_check_retry_in() -> Option<Duration> {
    LAST_FOLDER_CHECK.with_borrow(|last| {
        let check = last.as_ref().filter(|check| check.error.is_some())?;
        FOLDER_CHECK_LIFE.checked_sub(check.at.elapsed())
    })
}

/// `validate_apply` of `values`. A fixed folder costs a file system call, so
/// the answer for the same values holds for `FOLDER_CHECK_LIFE`.
fn apply_error(values: &AppOptions) -> Option<String> {
    if values.song_folder_mode != SongFolderMode::Fixed || values.fixed_song_folder.is_none() {
        return values.validate_apply().err();
    }
    LAST_FOLDER_CHECK.with_borrow_mut(|last| {
        if let Some(check) = last
            .as_ref()
            .filter(|check| check.values == *values && check.at.elapsed() < FOLDER_CHECK_LIFE)
        {
            return check.error.clone();
        }
        let error = values.validate_apply().err();
        *last = Some(FolderCheck {
            values: values.clone(),
            at: Instant::now(),
            error: error.clone(),
        });
        error
    })
}

/// Check a draft as Apply does. A newly selected folder must exist; an
/// unchanged accepted folder keeps its fallback if it later disappears.
pub(super) fn validation_error(values: &AppOptions, active: &AppOptions) -> Option<String> {
    if values.song_folder_mode != active.song_folder_mode
        || values.fixed_song_folder != active.fixed_song_folder
    {
        apply_error(values)
    } else {
        values.validate().err()
    }
}

impl OptionsDraft {
    pub(crate) fn new(facts: &OptionsFacts, page: Page) -> Self {
        Self {
            values: facts.active.clone(),
            revision: facts.revision,
            page,
            restored: false,
            error: None,
            saved: false,
            confirm_discard: false,
        }
    }

    /// Apply has work: an edited value, or defaults that replace an invalid
    /// file.
    pub(crate) fn pending(&self, facts: &OptionsFacts) -> bool {
        self.values != facts.active || (self.restored && facts.invalid)
    }

    pub(crate) fn changes(&self, facts: &OptionsFacts) -> [usize; 3] {
        page_changes(&self.values, &facts.active)
    }

    /// The active options changed since the draft started.
    pub(crate) fn stale(&self, facts: &OptionsFacts) -> bool {
        self.revision != facts.revision
    }

    /// Why the edited audio settings cannot apply now, if they cannot.
    pub(crate) fn audio_blocked<'a>(&self, facts: &'a OptionsFacts) -> Option<&'a str> {
        self.values
            .audio_changed(&facts.active)
            .then_some(facts.audio_blocked.as_deref())
            .flatten()
    }

    /// Why the values cannot be saved as they are, such as a relative fixed
    /// folder.
    pub(crate) fn invalid(&self, facts: &OptionsFacts) -> Option<String> {
        validation_error(&self.values, &facts.active)
    }

    /// Why Apply is disabled, if it is.
    pub(crate) fn apply_blocked(&self, facts: &OptionsFacts) -> Option<String> {
        if self.stale(facts) {
            Some(STALE_DRAFT.to_owned())
        } else if !self.pending(facts) {
            Some(labels::NOTHING_TO_APPLY.to_owned())
        } else if let Some(reason) = self.invalid(facts) {
            Some(reason)
        } else {
            self.audio_blocked(facts).map(str::to_owned)
        }
    }

    /// The footer's account of the draft, empty when there is nothing to say.
    pub(crate) fn status(&self, facts: &OptionsFacts) -> String {
        let count: usize = self.changes(facts).iter().sum();
        match count {
            0 if self.pending(facts) => labels::DEFAULTS_PENDING.into(),
            0 if self.saved => labels::APPLIED.into(),
            0 => String::new(),
            _ => labels::unapplied(count),
        }
    }

    pub(crate) fn update(&mut self, action: Action, facts: &OptionsFacts) -> Option<Effect> {
        match action {
            Action::Show(page) => self.page = page,
            Action::Restore => {
                self.values = AppOptions::default();
                self.restored = true;
                self.error = None;
                self.saved = false;
            }
            Action::Reload => self.reload(facts),
            Action::Apply | Action::ApplyAndClose => {
                forget_folder_check();
                if let Some(reason) = self.apply_blocked(facts) {
                    self.failed(reason);
                    self.confirm_discard = false;
                    return None;
                }
                self.error = None;
                let close = action == Action::ApplyAndClose;
                if close {
                    self.confirm_discard = false;
                }
                return Some(Effect::Apply {
                    values: self.values.clone(),
                    revision: self.revision,
                    restored: self.restored,
                    close,
                });
            }
            Action::Close if self.pending(facts) => self.confirm_discard = true,
            Action::Close => return Some(Effect::Close),
            Action::Discard => {
                self.confirm_discard = false;
                return Some(Effect::Close);
            }
            Action::KeepEditing => self.confirm_discard = false,
            Action::PickFolder => return Some(Effect::PickFolder),
            Action::SaveCapture => return Some(Effect::SaveCapture),
            Action::OpenLibraries => return Some(Effect::OpenLibraries),
        }
        None
    }

    /// The Session applied the draft; `facts` are read after it.
    pub(crate) fn applied(&mut self, facts: &OptionsFacts) {
        self.reload(facts);
        self.saved = true;
    }

    /// The Session refused the draft.
    pub(crate) fn failed(&mut self, error: String) {
        self.error = Some(error);
    }

    /// A clean draft has nothing to discard, so its prompt goes.
    pub(crate) fn settle(&mut self, facts: &OptionsFacts) {
        if !self.pending(facts) {
            self.confirm_discard = false;
        }
    }

    fn reload(&mut self, facts: &OptionsFacts) {
        let page = self.page;
        *self = Self::new(facts, page);
    }
}
