//! Application preferences. These values are independent of songs and exports.
pub use crate::playback_quality::Oversampling;
use crate::playback_quality::PlaybackQuality;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackBufferTarget {
    #[serde(rename = "automatic")]
    Automatic,
    #[serde(rename = "20ms")]
    Ms20,
    #[default]
    #[serde(rename = "40ms")]
    Ms40,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeterDisplayRate {
    #[default]
    Normal,
    Reduced,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartingModule {
    #[default]
    Sounds,
    Mixer,
    Mastering,
    Effects,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SongFolderMode {
    #[default]
    LastUsed,
    Fixed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppOptions {
    pub schema_version: u32,
    pub drive_playback_oversampling: Oversampling,
    pub master_playback_oversampling: Oversampling,
    pub preview_level_db: f64,
    pub preview_muted: bool,
    pub playback_buffer_target: PlaybackBufferTarget,
    pub meter_display_rate: MeterDisplayRate,
    pub follow_on_startup: bool,
    pub show_section_bookmarks: bool,
    pub starting_module: StartingModule,
    pub show_tooltips: bool,
    pub song_folder_mode: SongFolderMode,
    pub fixed_song_folder: Option<PathBuf>,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            drive_playback_oversampling: Oversampling::FourX,
            master_playback_oversampling: Oversampling::FourX,
            preview_level_db: 0.0,
            preview_muted: false,
            playback_buffer_target: PlaybackBufferTarget::Ms40,
            meter_display_rate: MeterDisplayRate::Normal,
            follow_on_startup: true,
            show_section_bookmarks: true,
            starting_module: StartingModule::Sounds,
            show_tooltips: true,
            song_folder_mode: SongFolderMode::LastUsed,
            fixed_song_folder: None,
        }
    }
}

impl AppOptions {
    pub fn playback_quality(&self) -> PlaybackQuality {
        PlaybackQuality {
            drive: self.drive_playback_oversampling,
            master: self.master_playback_oversampling,
        }
    }

    pub fn quality_reduced(&self) -> bool {
        self.drive_playback_oversampling == Oversampling::Off
            || self.master_playback_oversampling == Oversampling::Off
    }

    pub fn preview_gain(&self) -> f32 {
        if self.preview_muted {
            0.0
        } else {
            crate::engine::db_to_amplitude(self.preview_level_db as f32)
        }
    }

    /// Only fields that affect sound or the output stream require a stopped transport.
    pub fn audio_changed(&self, other: &Self) -> bool {
        self.playback_quality() != other.playback_quality()
            || self.preview_level_db != other.preview_level_db
            || self.preview_muted != other.preview_muted
            || self.playback_buffer_target != other.playback_buffer_target
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err("The options schema version is not supported.".into());
        }
        if !self.preview_level_db.is_finite() || !(-30.0..=0.0).contains(&self.preview_level_db) {
            return Err("Preview level must be from -30 to 0 dB.".into());
        }
        if self
            .fixed_song_folder
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err("Select an absolute fixed song folder.".into());
        }
        if self.song_folder_mode == SongFolderMode::Fixed && self.fixed_song_folder.is_none() {
            return Err("Select a fixed song folder.".into());
        }
        Ok(())
    }

    pub fn validate_apply(&self) -> Result<(), String> {
        self.validate()?;
        if self.song_folder_mode == SongFolderMode::Fixed
            && !self
                .fixed_song_folder
                .as_ref()
                .is_some_and(|path| path.is_dir())
        {
            return Err("The fixed song folder is unavailable. Select an existing folder.".into());
        }
        Ok(())
    }
}

pub(crate) struct LoadedOptions {
    pub values: AppOptions,
    pub error: Option<String>,
    pub invalid: bool,
    pub future: bool,
}

pub(crate) fn load(root: &Path) -> LoadedOptions {
    let mut loaded = LoadedOptions {
        values: AppOptions::default(),
        error: None,
        invalid: false,
        future: false,
    };
    let file = match fs::File::open(root.join("options.json")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            loaded.values.show_section_bookmarks = crate::project_io::load_section_bookmarks(root);
            return loaded;
        }
        Err(error) => {
            loaded.invalid = true;
            loaded.error = Some(format!(
                "Cannot read options: {error}. Safe defaults are active."
            ));
            return loaded;
        }
    };
    let parsed: Result<AppOptions, String> = (|| {
        let mut bytes = Vec::new();
        file.take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("The options file is too large.".into());
        }
        let raw: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if raw
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .is_some_and(|v| v > u64::from(SCHEMA_VERSION))
        {
            loaded.future = true;
            return Err(
                "This options file needs a newer application. Persistent writes are blocked."
                    .into(),
            );
        }
        // Parse the original bytes so duplicate keys are rejected by the typed reader.
        let values: AppOptions = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        values.validate()?;
        Ok(values)
    })();
    match parsed {
        Ok(values) => {
            if values.song_folder_mode == SongFolderMode::Fixed
                && !values
                    .fixed_song_folder
                    .as_ref()
                    .is_some_and(|path| path.is_dir())
            {
                loaded.error = Some(
                    "The fixed song folder is unavailable. Song dialogs use the last used folder."
                        .into(),
                );
            }
            loaded.values = values;
        }
        Err(error) => {
            loaded.invalid = !loaded.future;
            loaded.error = Some(format!(
                "Cannot load options: {error} Safe defaults are active."
            ));
        }
    }
    loaded
}

pub(crate) fn save(root: &Path, values: &AppOptions) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(values).map_err(|e| e.to_string())?;
    fs::create_dir_all(root).map_err(|e| format!("Cannot create the settings folder: {e}"))?;
    crate::atomic_file::write(&root.join("options.json"), &bytes)
        .map_err(|e| format!("Cannot save options: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mixjam-options-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn migrates_sections_only_until_options_exist() {
        let root = root();
        crate::project_io::save_section_bookmarks(&root, false).unwrap();
        assert!(!load(&root).values.show_section_bookmarks);
        save(&root, &AppOptions::default()).unwrap();
        assert!(load(&root).values.show_section_bookmarks);
        assert_eq!(
            fs::read(root.join("section-bookmarks.json")).unwrap(),
            b"false"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupt_and_future_files_remain_intact() {
        let root = root();
        for bytes in [
            b"broken".as_slice(),
            b"{\"schemaVersion\":999,\"futureField\":true}".as_slice(),
        ] {
            fs::write(root.join("options.json"), bytes).unwrap();
            let result = load(&root);
            assert_eq!(result.values, AppOptions::default());
            assert!(result.error.is_some());
            assert_eq!(fs::read(root.join("options.json")).unwrap(), bytes);
        }
        assert!(load(&root).future);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_invalid_types_ranges_versions_and_duplicate_keys() {
        let root = root();
        for (key, value) in [
            ("previewLevelDb", serde_json::json!(-31)),
            ("previewLevelDb", serde_json::json!(-30.00000001)),
            ("previewLevelDb", serde_json::json!(1)),
            ("previewMuted", serde_json::json!("false")),
            ("drivePlaybackOversampling", serde_json::json!("8x")),
            ("schemaVersion", serde_json::json!(0)),
        ] {
            let mut raw = serde_json::to_value(AppOptions::default()).unwrap();
            raw[key] = value;
            fs::write(root.join("options.json"), serde_json::to_vec(&raw).unwrap()).unwrap();
            assert!(load(&root).invalid);
        }
        let text = serde_json::to_string(&AppOptions::default())
            .unwrap()
            .replacen("{", "{\"previewMuted\":true,", 1);
        fs::write(root.join("options.json"), text).unwrap();
        assert!(load(&root).invalid);
        let options = AppOptions {
            preview_level_db: f64::NAN,
            ..AppOptions::default()
        };
        assert!(options.validate().is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
