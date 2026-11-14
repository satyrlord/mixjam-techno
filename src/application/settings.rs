//! Application options, the workspace layout, and the folders that a session remembers.
use super::Session;
use crate::audio::Player;
use std::path::{Path, PathBuf};

impl Session {
    /// Keep settings and the scan cache in `root` instead of the user's data folder.
    #[cfg(test)]
    pub(crate) fn with_settings_root(mut self, root: PathBuf) -> Self {
        self.settings_root = root;
        self.load_options();
        self
    }
    /// Give UI tests a theme without an options file. `draw` adopts the
    /// session's theme, so a fixture must carry the theme it paints.
    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn set_theme_fixture(&mut self, theme: crate::options::Theme) {
        self.options.theme = theme;
    }
    /// Where a song file dialog starts: the folder of the song last chosen in one.
    pub fn song_folder(&self) -> Option<PathBuf> {
        if self.options.song_folder_mode == crate::options::SongFolderMode::Fixed
            && let Some(folder) = self
                .options
                .fixed_song_folder
                .as_ref()
                .filter(|p| p.is_dir())
        {
            return Some(folder.clone());
        }
        crate::project_io::load_song_folder(&self.settings_root)
    }

    pub fn song_folder_warning(&self) -> Option<String> {
        if self.options.song_folder_mode == crate::options::SongFolderMode::Fixed
            && let Some(folder) = &self.options.fixed_song_folder
            && !folder.is_dir()
        {
            return Some(format!(
                "The fixed song folder {} is unavailable. Song dialogs use the last used folder.",
                folder.display()
            ));
        }
        None
    }
    /// Start later song file dialogs in the folder of `song`. A failed write
    /// only loses that convenience.
    pub fn remember_song_folder(&self, song: &Path) {
        if let Some(folder) = song.parent().filter(|folder| folder.is_dir()) {
            let _ = crate::project_io::save_song_folder(&self.settings_root, folder);
        }
    }
    /// Whether the user wants section bookmarks shown on the arrangement.
    pub fn section_bookmarks_setting(&self) -> bool {
        self.options.show_section_bookmarks
    }
    /// Keep the bookmark choice for later sessions. A failed write only loses that convenience.
    pub fn remember_section_bookmarks(&mut self, visible: bool) -> Result<(), String> {
        let mut options = self.options.clone();
        options.show_section_bookmarks = visible;
        self.apply_options(options, self.options_revision, false)
            .map(|_| ())
    }

    pub fn layout(&self) -> crate::options::WorkspaceLayout {
        self.layout
    }
    /// Adopt the layout at once; a failed write only loses it for later sessions.
    pub fn set_layout(&mut self, layout: crate::options::WorkspaceLayout) -> Result<(), String> {
        if layout == self.layout {
            return Ok(());
        }
        self.layout = layout;
        if self.layout_saves {
            crate::options::save_layout(&self.settings_root, layout)?;
        }
        Ok(())
    }

    pub(super) fn load_options(&mut self) {
        self.layout = crate::options::load_layout(&self.settings_root);
        self.layout_saves = true;
        let loaded = crate::options::load(&self.settings_root);
        self.options = loaded.values;
        self.options_error = loaded.error;
        self.options_invalid = loaded.invalid;
        self.options_future = loaded.future;
        if let Some(error) = self.options_error.clone() {
            self.error(error);
        }
    }

    pub fn options(&self) -> &crate::options::AppOptions {
        &self.options
    }
    pub fn options_revision(&self) -> u64 {
        self.options_revision
    }
    pub fn options_error(&self) -> Option<&str> {
        self.options_error.as_deref()
    }
    /// The loaded options file was invalid; a restored draft may replace it.
    pub fn options_invalid(&self) -> bool {
        self.options_invalid
    }

    pub fn audio_options_blocked_reason(&self) -> Option<&'static str> {
        (self.wants_playback || self.playing_mix() || self.is_previewing() || self.is_recording())
            .then_some("Stop playback, previews, and recording to apply audio options.")
    }

    pub fn buffer_status(&self) -> String {
        self.player.as_ref().map_or_else(
            || {
                "Audio output is unavailable. Buffer capacity and callback period are unavailable."
                    .into()
            },
            Player::output_status,
        )
    }

    pub fn buffer_target_error(
        &self,
        target: crate::options::PlaybackBufferTarget,
    ) -> Option<String> {
        self.player
            .as_ref()
            .and_then(|player| player.buffer_target_error(target))
    }

    /// Validate and prepare first. Publish the file before adopting active values.
    /// A no-op neither writes settings nor invalidates playback preparation.
    pub fn apply_options(
        &mut self,
        draft: crate::options::AppOptions,
        expected_revision: u64,
        restore_invalid: bool,
    ) -> Result<bool, String> {
        if expected_revision != self.options_revision {
            return Err("Options changed outside this window. Select Reload before Apply.".into());
        }
        draft.validate()?;
        if self.options_future && (draft != self.options || restore_invalid) {
            return Err(
                "This options file needs a newer application. Persistent writes are blocked."
                    .into(),
            );
        }
        if draft == self.options && !(self.options_invalid && restore_invalid) {
            return Ok(false);
        }
        if self.options_invalid && !restore_invalid {
            return Err(
                "The options file is invalid. Select Restore options defaults before Apply.".into(),
            );
        }
        // An already accepted folder can disappear after Apply. Keep its
        // fallback usable for unrelated preferences, including Sections.
        if draft.song_folder_mode != self.options.song_folder_mode
            || draft.fixed_song_folder != self.options.fixed_song_folder
        {
            draft.validate_apply()?;
        }
        if self.options.audio_changed(&draft)
            && let Some(reason) = self.audio_options_blocked_reason()
        {
            return Err(reason.into());
        }
        let replacement = if self.options.playback_buffer_target != draft.playback_buffer_target {
            Some(
                self.player
                    .as_ref()
                    .ok_or("Audio output is unavailable. The buffer target was not changed.")?
                    .prepare_output(draft.playback_buffer_target)?,
            )
        } else {
            None
        };
        crate::options::save(&self.settings_root, &draft)?;
        let quality_changed = self.options.playback_quality() != draft.playback_quality();
        self.options = draft;
        self.options_revision = self.options_revision.wrapping_add(1);
        self.options_error = None;
        self.options_invalid = false;
        if let Some(output) = replacement {
            self.player.as_mut().unwrap().adopt_output(output);
        }
        if let Some(player) = &mut self.player {
            player.set_preview_gain(self.options.preview_gain());
        }
        if quality_changed {
            // The render revision also rejects an old job which completes after Apply.
            self.revision = self.revision.wrapping_add(1);
            self.render_dirty = true;
            self.render = None;
            self.failed_revision = None;
            self.cancel_playback_request();
            self.prepare_ahead();
        }
        Ok(true)
    }
}
