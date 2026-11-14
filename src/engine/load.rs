//! Reading the earlier JSON project format, and migrating imported and
//! older projects to the current one.
use super::{
    ImportNotice, PROJECT_SCHEMA_VERSION, Project, TRACK_COUNT, Track, nearest_supported_bpm,
    validate::valid_gain,
};
use std::{fs, io::Read, path::Path};

/// Guards against a file that is not a project, as the song reader's limit
/// does. A project holds no audio, and at most `MAX_CLIPS` clips of a few
/// hundred bytes each, so a real one stays far below this.
pub(super) const MAX_PROJECT_BYTES: usize = 1024 * 1024 * 1024;

impl Project {
    /// Write the earlier JSON project format. The application saves `.mixjam`
    /// songs; tests use this to make JSON projects to open.
    #[cfg(test)]
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        crate::atomic_file::write(path, &bytes)
    }
    /// Read the earlier JSON project format.
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_PROJECT_BYTES as u64 {
            return Err("The project file is too large.".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_PROJECT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::parse_json(&bytes)
    }

    pub(super) fn parse_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err("The project file is too large.".into());
        }
        let project: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if matches!(project.schema_version, 0..=3) && !valid_gain(project.master_gain) {
            return Err("Legacy master amplitude must be from 0 to 2.".into());
        }
        match project.schema_version {
            PROJECT_SCHEMA_VERSION => {
                project.validate()?;
                Ok(project)
            }
            2 | 3 => {
                let mut migrated = project;
                migrated.schema_version = PROJECT_SCHEMA_VERSION;
                migrated.validate()?;
                Ok(migrated)
            }
            1 => {
                // Schema 1 already uses the common 256-tick clock and fixed tempos.
                // Only expand its capacity; do not reinterpret its musical data.
                if project.tracks.len() != 32 || project.clips.iter().any(|clip| clip.track >= 32) {
                    return Err("A version 1 project must have 32 track slots.".into());
                }
                let mut migrated = project;
                migrated.schema_version = PROJECT_SCHEMA_VERSION;
                migrated.tracks.resize(TRACK_COUNT, Track::default());
                migrated.validate()?;
                Ok(migrated)
            }
            0 => {
                if project.tracks.len() != 16 {
                    return Err("A legacy JSON project must have 16 tracks.".into());
                }
                if !project.bpm.is_finite()
                    || !(30.0..=300.0).contains(&project.bpm)
                    || !project.pitch_ratio.is_finite()
                    || !(0.25..=4.0).contains(&project.pitch_ratio)
                {
                    return Err("The legacy project has invalid tempo or pitch data.".into());
                }
                normalize_import(project)
            }
            _ => Err("This project version is not supported.".into()),
        }
    }
}

/// Preserve valid imported tracks and clips, then apply the current project contract.
pub fn normalize_import(mut project: Project) -> Result<Project, String> {
    if project.schema_version != 0 && project.schema_version != PROJECT_SCHEMA_VERSION {
        return Err("This project version is not supported.".into());
    }
    let source_track_count = project.tracks.len();
    if source_track_count > TRACK_COUNT {
        return Err("This song has more than 64 tracks. The editor supports 64 tracks.".into());
    }
    if source_track_count == 0 {
        return Err("An imported project must have at least one track.".into());
    }
    if project
        .clips
        .iter()
        .any(|clip| clip.track >= source_track_count)
    {
        return Err("An imported clip refers to a track outside its source project.".into());
    }
    if !project.bpm.is_finite()
        || project.bpm <= 0.0
        || !project.pitch_ratio.is_finite()
        || project.pitch_ratio <= 0.0
    {
        return Err("The imported project has invalid tempo or pitch data.".into());
    }
    let source_bpm = project.bpm * project.pitch_ratio;
    let target_bpm = nearest_supported_bpm(source_bpm)?;
    project.schema_version = PROJECT_SCHEMA_VERSION;
    project.bpm = target_bpm;
    project.pitch_ratio = 1.0;
    project.tracks.resize(TRACK_COUNT, Track::default());
    if source_bpm != target_bpm {
        project.import_notices.push(ImportNotice {
            source_bpm,
            target_bpm,
        });
    }
    project.validate()?;
    Ok(project)
}
