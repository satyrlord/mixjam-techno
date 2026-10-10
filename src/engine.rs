use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const PROJECT_SCHEMA_VERSION: u32 = 2;
pub const TRACK_COUNT: usize = 64;
pub const SUPPORTED_BPMS: [f64; 4] = [120.0, 140.0, 160.0, 180.0];
pub const TICKS_PER_BAR: u32 = 256;
pub const SOURCE_BPM: f64 = 140.0;
pub const MAX_TICKS: u32 = 4096 * TICKS_PER_BAR;
const MAX_PROJECT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub gain: f32,
    #[serde(default)]
    pub pan: f32,
    pub left_enabled: bool,
    pub right_enabled: bool,
}
impl Default for Track {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            left_enabled: true,
            right_enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub sample_id: u32,
    pub track: usize,
    pub start_tick: u32,
    pub length_ticks: u32,
    pub offset_frames: u32,
    pub gain: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportedSample {
    pub id: u32,
    pub name: String,
    pub group_name: String,
    pub path: PathBuf,
    #[serde(default = "default_source_bpm")]
    pub source_bpm: f64,
    #[serde(default)]
    pub origin: Option<crate::products::Product>,
}
fn default_source_bpm() -> f64 {
    SOURCE_BPM
}

/// Preserve both bounded original wrapped labels, including non-ASCII text.
pub fn validate_import_metadata(sample: &ImportedSample) -> Result<(), String> {
    let text_limit = legacy_format::imports::MAX_STRING_BYTES;
    if sample.id < crate::products::IMPORTED_ID_START || sample.path.as_os_str().is_empty() {
        return Err("The imported sample needs a valid ID and file path.".into());
    }
    if sample.name.trim().is_empty()
        || sample.name.chars().count() > text_limit * 2 + 1
        || sample.group_name.trim().is_empty()
        || sample.group_name.chars().count() > text_limit
        || sample.name.contains('\0')
        || sample.group_name.contains('\0')
    {
        return Err(
            "The imported sample labels are empty, too long, or contain a null character.".into(),
        );
    }
    if !sample.source_bpm.is_finite() || sample.source_bpm <= 0.0 {
        return Err("The imported sample needs a finite positive source tempo.".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportNotice {
    pub source_bpm: f64,
    pub target_bpm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default, deserialize_with = "deserialize_schema_version")]
    pub schema_version: u32,
    pub title: String,
    pub author: String,
    pub bpm: f64,
    pub master_gain: f32,
    pub pitch_ratio: f64,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    pub loop_region: Option<[u32; 2]>,
    #[serde(default)]
    pub imported_samples: Vec<ImportedSample>,
    #[serde(default)]
    pub import_notices: Vec<ImportNotice>,
    #[serde(default)]
    pub source_notices: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<crate::products::LibraryBinding>,
}
/// An absent version marks a legacy project. Zero is reserved for that case,
/// so an explicit zero is not a supported version.
fn deserialize_schema_version<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u32, D::Error> {
    match u32::deserialize(deserializer)? {
        0 => Err(serde::de::Error::custom(
            "This project version is not supported.",
        )),
        version => Ok(version),
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            title: "Untitled".into(),
            author: String::new(),
            bpm: SOURCE_BPM,
            master_gain: 0.75,
            pitch_ratio: 1.0,
            tracks: vec![Track::default(); TRACK_COUNT],
            clips: Vec::new(),
            loop_region: None,
            imported_samples: Vec::new(),
            import_notices: Vec::new(),
            source_notices: Vec::new(),
            libraries: Vec::new(),
        }
    }
}
impl Project {
    pub fn end_tick(&self) -> u32 {
        self.clips
            .iter()
            .map(|c| c.start_tick.saturating_add(c.length_ticks))
            .max()
            .unwrap_or(TICKS_PER_BAR * 4)
            .max(self.loop_region.map_or(0, |[_, b]| b))
    }
    pub fn frames_per_tick(&self, sample_rate: u32) -> f64 {
        sample_rate as f64 * 240.0 / self.bpm / TICKS_PER_BAR as f64
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err("This project version is not supported.".into());
        }
        if self.tracks.len() != TRACK_COUNT {
            return Err("The project must have 64 track slots.".into());
        }
        if !SUPPORTED_BPMS.contains(&self.bpm) {
            return Err("Tempo must be 120, 140, 160, or 180 BPM.".into());
        }
        if self.pitch_ratio != 1.0 {
            return Err("Project pitch must be 1.0.".into());
        }
        if self.import_notices.iter().any(|notice| {
            !notice.source_bpm.is_finite()
                || notice.source_bpm <= 0.0
                || !SUPPORTED_BPMS.contains(&notice.target_bpm)
        }) {
            return Err("The project has invalid import tempo data.".into());
        }
        let mut library_products = HashSet::new();
        if self.libraries.iter().any(|binding| {
            binding.root.as_os_str().is_empty() || !library_products.insert(binding.product)
        }) {
            return Err("Each product needs one library binding with a nonempty root.".into());
        }
        if !valid_gain(self.master_gain) {
            return Err("Master volume must be from 0 to 2.".into());
        }
        if self
            .tracks
            .iter()
            .any(|track| !track.gain.is_finite() || !(-2.0..=4.0).contains(&track.gain))
        {
            return Err("Track gain must be from -2 to 4.".into());
        }
        if self
            .tracks
            .iter()
            .any(|track| !track.pan.is_finite() || !(-1.0..=1.0).contains(&track.pan))
        {
            return Err("Track pan must be from -1 to 1.".into());
        }
        if self.clips.len() > 100_000 {
            return Err("The project has too many clips.".into());
        }
        let mut ids = HashSet::new();
        let mut imported_ids = HashSet::new();
        if self.imported_samples.len() > 100_000 {
            return Err("The project has too many imported samples.".into());
        }
        for sample in &self.imported_samples {
            validate_import_metadata(sample)?;
            if !imported_ids.insert(sample.id) {
                return Err("Imported sample IDs must be unique.".into());
            }
        }
        for c in &self.clips {
            if c.sample_id >= 1_000_000 && !imported_ids.contains(&c.sample_id) {
                return Err(format!("Clip {} has no imported sample reference.", c.id));
            }
            if !ids.insert(c.id) {
                return Err("Clip IDs must be unique.".into());
            }
            if c.track >= TRACK_COUNT
                || c.length_ticks == 0
                || c.start_tick
                    .checked_add(c.length_ticks)
                    .is_none_or(|end| end > MAX_TICKS)
                || !valid_gain(c.gain)
            {
                return Err(format!(
                    "Clip {} has invalid track, time, or volume data.",
                    c.id
                ));
            }
        }
        if let Some([a, b]) = self.loop_region
            && (a >= b || b > MAX_TICKS)
        {
            return Err("The loop must be within the timeline and have a positive length.".into());
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err("The project file exceeds the 16 MiB limit. Use a smaller project.".into());
        }
        let filename = path
            .file_name()
            .ok_or("Select a project file.")?
            .to_string_lossy();
        let temp = path.with_file_name(format!(".{filename}.{}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let result = (|| -> Result<(), String> {
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            drop(file);
            fs::rename(&temp, path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
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

    fn parse_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err("The project file is too large.".into());
        }
        let project: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        match project.schema_version {
            PROJECT_SCHEMA_VERSION => {
                project.validate()?;
                Ok(project)
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

/// Select the nearest supported project tempo. Midpoint ties select the lower tempo.
pub fn nearest_supported_bpm(source_bpm: f64) -> Result<f64, String> {
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    for pair in SUPPORTED_BPMS.windows(2) {
        if source_bpm <= (pair[0] + pair[1]) / 2.0 {
            return Ok(pair[0]);
        }
    }
    Ok(SUPPORTED_BPMS[SUPPORTED_BPMS.len() - 1])
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

fn valid_gain(g: f32) -> bool {
    g.is_finite() && (0.0..=2.0).contains(&g)
}

pub struct Editor {
    pub project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
}
impl Editor {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn transact(&mut self, change: impl FnOnce(&mut Project)) -> Result<(), String> {
        let previous = self.project.clone();
        change(&mut self.project);
        if let Err(e) = self.project.validate() {
            self.project = previous;
            return Err(e);
        }
        if previous != self.project {
            self.undo.push(previous);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            true
        } else {
            false
        }
    }
    pub fn place(
        &mut self,
        sample_id: u32,
        track: usize,
        start_tick: u32,
        length_ticks: u32,
    ) -> Result<u64, String> {
        ensure_space(&self.project, None, track, start_tick, length_ticks)?;
        let id = self
            .project
            .clips
            .iter()
            .map(|c| c.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Clip ID limit reached.")?;
        self.transact(|p| {
            p.clips.push(Clip {
                id,
                sample_id,
                track,
                start_tick,
                length_ticks,
                offset_frames: 0,
                gain: 1.0,
            })
        })?;
        Ok(id)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        if !self.project.clips.iter().any(|c| c.id == id) {
            return Err("The clip does not exist.".into());
        }
        self.transact(|p| p.clips.retain(|c| c.id != id))
    }
    pub fn move_clip(&mut self, id: u64, track: usize, start_tick: u32) -> Result<(), String> {
        let c = self
            .project
            .clips
            .iter()
            .find(|c| c.id == id)
            .ok_or("The clip does not exist.")?;
        ensure_space(&self.project, Some(id), track, start_tick, c.length_ticks)?;
        self.transact(|p| {
            let c = p.clips.iter_mut().find(|c| c.id == id).unwrap();
            c.track = track;
            c.start_tick = start_tick;
        })
    }
    pub fn duplicate(&mut self, id: u64, track: usize, start_tick: u32) -> Result<(), String> {
        let c = self
            .project
            .clips
            .iter()
            .find(|c| c.id == id)
            .ok_or("The clip does not exist.")?
            .clone();
        let new_id = self.place(c.sample_id, track, start_tick, c.length_ticks)?;
        // One undo entry must cover the complete duplicate, including its trim and volume.
        let new = self
            .project
            .clips
            .iter_mut()
            .find(|c| c.id == new_id)
            .unwrap();
        new.offset_frames = c.offset_frames;
        new.gain = c.gain;
        Ok(())
    }
    pub fn move_clips(
        &mut self,
        ids: &[u64],
        track_delta: i32,
        tick_delta: i64,
        copy: bool,
    ) -> Result<Vec<u64>, String> {
        let selected: HashSet<_> = ids.iter().copied().collect();
        if selected.len() != ids.len() || ids.is_empty() {
            return Err("Select one or more different clips.".into());
        }
        let mut additions = Vec::with_capacity(ids.len());
        let mut next_id = self.project.clips.iter().map(|c| c.id).max().unwrap_or(0);
        for id in ids {
            let mut c = self
                .project
                .clips
                .iter()
                .find(|c| c.id == *id)
                .ok_or("A selected clip does not exist.")?
                .clone();
            c.track = usize::try_from(c.track as i64 + track_delta as i64)
                .map_err(|_| "The selection is outside the timeline.")?;
            c.start_tick = u32::try_from(c.start_tick as i64 + tick_delta)
                .map_err(|_| "The selection is outside the timeline.")?;
            if copy {
                next_id = next_id.checked_add(1).ok_or("Clip ID limit reached.")?;
                c.id = next_id;
            }
            additions.push(c);
        }
        let mut proposed = self.project.clone();
        if !copy {
            proposed.clips.retain(|c| !selected.contains(&c.id));
        }
        for c in &additions {
            ensure_space(&proposed, None, c.track, c.start_tick, c.length_ticks)?;
            proposed.clips.push(c.clone());
        }
        let result = additions.iter().map(|c| c.id).collect();
        self.transact(|p| *p = proposed)?;
        Ok(result)
    }
}
fn ensure_space(
    p: &Project,
    except: Option<u64>,
    track: usize,
    start: u32,
    length: u32,
) -> Result<(), String> {
    let end = start
        .checked_add(length)
        .ok_or("The clip is outside the timeline.")?;
    if track >= TRACK_COUNT || length == 0 || end > MAX_TICKS {
        return Err("The clip is outside the timeline.".into());
    }
    if p.clips.iter().any(|c| {
        Some(c.id) != except
            && c.track == track
            && start < c.start_tick.saturating_add(c.length_ticks)
            && end > c.start_tick
    }) {
        return Err("This track position is occupied.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paired_native_labels_and_signed_track_levels_survive_json() {
        let mut project = Project::default();
        project.tracks[31].gain = -2.0 / 65536.0;
        project.tracks[0].gain = 2.352_630_6;
        project.imported_samples.push(ImportedSample {
            id: 1_000_000,
            name: format!("{} {}", "ă".repeat(200), "Î".repeat(200)),
            group_name: "Wave".into(),
            path: "source.wav".into(),
            source_bpm: 2000.0,
            origin: None,
        });
        project.validate().unwrap();
        let restored: Project =
            serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        assert_eq!(restored, project);
        project.tracks[31].gain = -2.01;
        assert!(project.validate().is_err());
        project.tracks[31].gain = 0.0;
        project.imported_samples[0].group_name.clear();
        assert!(project.validate().is_err());
    }

    #[test]
    fn oversized_save_leaves_the_existing_project_and_directory_unchanged() {
        let directory = std::env::temp_dir().join(format!(
            "mixjam-techno-oversized-save-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("project.json");
        fs::write(&path, b"existing project").unwrap();
        let project = Project {
            title: "x".repeat(MAX_PROJECT_BYTES),
            ..Project::default()
        };
        assert!(project.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing project");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn last_of_sixty_four_tracks_supports_edit_undo_and_redo() {
        let mut editor = Editor::new(Project::default());
        assert_eq!(editor.project.tracks.len(), 64);
        editor.project.validate().unwrap();
        let id = editor.place(1, 63, 17, 31).unwrap();
        let placed = editor.project.clone();
        editor.move_clip(id, 62, 48).unwrap();
        assert!(editor.undo());
        assert_eq!(editor.project, placed);
        assert!(editor.redo());
        assert_eq!(editor.project.clips[0].track, 62);
        let before = editor.project.clone();
        assert!(editor.place(1, 64, 0, 256).is_err());
        assert!(
            editor
                .transact(|project| project.tracks.push(Track::default()))
                .is_err()
        );
        assert_eq!(editor.project, before);
    }

    #[test]
    fn imported_track_shapes_preserve_values_and_pad_to_sixty_four() {
        for source_track_count in [1, 8, 16, 24, 32, 33, 48, 49, 64] {
            let mut project = Project::default();
            project.tracks.truncate(source_track_count);
            project.tracks[0] = Track {
                pan: 0.0,
                gain: 0.3,
                left_enabled: false,
                right_enabled: true,
            };
            project.clips.push(Clip {
                id: 37,
                sample_id: 13_000,
                track: source_track_count - 1,
                start_tick: 17,
                length_ticks: 31,
                offset_frames: 7,
                gain: 0.5,
            });
            project.loop_region = Some([3, 999]);
            let original = project.clone();
            let normalized = normalize_import(project).unwrap();
            assert_eq!(normalized.tracks.len(), 64);
            assert_eq!(normalized.tracks[..source_track_count], original.tracks);
            assert!(
                normalized.tracks[source_track_count..]
                    .iter()
                    .all(|track| *track == Track::default())
            );
            assert_eq!(normalized.clips, original.clips);
            assert_eq!(normalized.loop_region, original.loop_region);
            assert!(normalized.import_notices.is_empty());
            normalized.validate().unwrap();
        }
    }

    #[test]
    fn imported_invalid_shapes_and_missing_source_tracks_are_rejected() {
        for source_track_count in [0, 65, 99] {
            let mut project = Project::default();
            project.tracks.resize(source_track_count, Track::default());
            assert!(normalize_import(project).is_err());
        }
        for source_track_count in [8, 16] {
            let mut project = Project::default();
            project.tracks.truncate(source_track_count);
            project.clips.push(Clip {
                id: 1,
                sample_id: 1,
                track: source_track_count,
                start_tick: 0,
                length_ticks: 256,
                offset_frames: 0,
                gain: 1.0,
            });
            assert!(normalize_import(project).is_err());
        }
        let project = Project {
            schema_version: PROJECT_SCHEMA_VERSION + 1,
            ..Project::default()
        };
        assert!(normalize_import(project).is_err());
    }

    #[test]
    fn nearest_project_tempo_uses_lower_midpoint_ties() {
        for (source, target) in [
            (0.001, 120.0),
            (120.0, 120.0),
            (130.0, 120.0),
            (130.001, 140.0),
            (150.0, 140.0),
            (150.001, 160.0),
            (170.0, 160.0),
            (170.001, 180.0),
            (180.0, 180.0),
            (f64::MAX, 180.0),
        ] {
            assert_eq!(nearest_supported_bpm(source).unwrap(), target);
        }
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(nearest_supported_bpm(invalid).is_err());
        }
    }

    #[test]
    fn current_project_contract_keeps_source_sample_tempo_independent() {
        let mut project = Project::default();
        for bpm in SUPPORTED_BPMS {
            project.bpm = bpm;
            project.validate().unwrap();
        }
        project.bpm = 146.0;
        assert!(project.validate().is_err());
        project.bpm = 140.0;
        project.pitch_ratio = 1.01;
        assert!(project.validate().is_err());
        project.pitch_ratio = 1.0;
        project.imported_samples.push(ImportedSample {
            id: 1_000_000,
            name: "Source tempo".into(),
            group_name: "Imported".into(),
            path: PathBuf::from("source.wav"),
            source_bpm: 0.001,
            origin: None,
        });
        project.validate().unwrap();
        project.imported_samples[0].source_bpm = 2_000.0;
        project.validate().unwrap();
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            project.imported_samples[0].source_bpm = invalid;
            assert!(project.validate().is_err());
        }
    }

    #[test]
    fn import_folds_legacy_pitch_once_and_records_only_tempo_changes() {
        let original = Project {
            pitch_ratio: 146.0 / 140.0,
            ..Project::default()
        };
        let normalized = normalize_import(original).unwrap();
        assert_eq!(normalized.bpm, 140.0);
        assert_eq!(normalized.pitch_ratio, 1.0);
        assert_eq!(normalized.import_notices.len(), 1);
        assert!((normalized.import_notices[0].source_bpm - 146.0).abs() < 1e-12);
        assert_eq!(normalized.import_notices[0].target_bpm, 140.0);
        assert_eq!(normalize_import(normalized.clone()).unwrap(), normalized);
        let supported_effective_tempo = Project {
            bpm: 120.0,
            pitch_ratio: 1.5,
            ..Project::default()
        };
        let normalized = normalize_import(supported_effective_tempo).unwrap();
        assert_eq!(normalized.bpm, 180.0);
        assert_eq!(normalized.pitch_ratio, 1.0);
        assert!(normalized.import_notices.is_empty());
        let overflowed_effective_tempo = Project {
            bpm: f64::MAX,
            pitch_ratio: 2.0,
            ..Project::default()
        };
        assert!(normalize_import(overflowed_effective_tempo).is_err());
    }

    #[test]
    fn json_rejects_unknown_versions_and_malformed_legacy_shapes() {
        let mut value = serde_json::to_value(Project::default()).unwrap();
        for version in [0, PROJECT_SCHEMA_VERSION + 1, u32::MAX] {
            value["schema_version"] = version.into();
            assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        value.as_object_mut().unwrap().remove("schema_version");
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        value["tracks"].as_array_mut().unwrap().truncate(16);
        value["bpm"] = 301.0.into();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        value["bpm"] = 140.0.into();
        value["pitch_ratio"] = 4.01.into();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(Project::parse_json(&vec![b' '; MAX_PROJECT_BYTES + 1]).is_err());
    }

    #[test]
    fn version_one_migration_only_expands_track_capacity() {
        let mut original = Project {
            schema_version: 1,
            ..Project::default()
        };
        original.tracks.truncate(32);
        original.tracks[31].pan = -0.75;
        original.clips.push(Clip {
            id: 55,
            sample_id: 731,
            track: 31,
            start_tick: 17,
            length_ticks: 45,
            offset_frames: 9,
            gain: 0.5,
        });
        original.import_notices.push(ImportNotice {
            source_bpm: 146.0,
            target_bpm: 140.0,
        });
        let mut value = serde_json::to_value(&original).unwrap();
        let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(migrated.schema_version, 2);
        assert_eq!(migrated.tracks.len(), 64);
        assert_eq!(migrated.tracks[..32], original.tracks);
        assert_eq!(migrated.clips, original.clips);
        assert_eq!(migrated.import_notices, original.import_notices);
        assert_eq!(migrated.bpm, original.bpm);
        assert_eq!(migrated.pitch_ratio, original.pitch_ratio);
        assert_eq!(
            Project::parse_json(&serde_json::to_vec(&migrated).unwrap()).unwrap(),
            migrated
        );

        value["clips"][0]["track"] = 32.into();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        value["clips"][0]["track"] = 31.into();
        value["bpm"] = 146.into();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        value["bpm"] = 140.into();
        value["tracks"].as_array_mut().unwrap().pop();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn old_sixteen_track_json_migrates_saves_and_reopens_with_one_notice() {
        let mut old_project = Project {
            title: "Original arrangement".into(),
            author: "Original author".into(),
            master_gain: 0.6,
            pitch_ratio: 146.0 / 140.0,
            loop_region: Some([17, 512]),
            ..Project::default()
        };
        old_project.tracks.truncate(16);
        old_project.tracks[15].gain = 0.25;
        old_project.tracks[15].right_enabled = false;
        old_project.imported_samples.push(ImportedSample {
            id: 1_000_000,
            name: "Imported source".into(),
            group_name: "Imported".into(),
            path: PathBuf::from("original-source.wav"),
            source_bpm: 127.0,
            origin: None,
        });
        old_project.clips.push(Clip {
            id: 19,
            sample_id: 1_000_000,
            track: 15,
            start_tick: 17,
            length_ticks: 31,
            offset_frames: 23,
            gain: 0.4,
        });
        let mut value = serde_json::to_value(&old_project).unwrap();
        value.as_object_mut().unwrap().remove("schema_version");
        value.as_object_mut().unwrap().remove("import_notices");
        value.as_object_mut().unwrap().remove("libraries");
        value["imported_samples"][0]
            .as_object_mut()
            .unwrap()
            .remove("origin");
        let directory = std::env::temp_dir().join(format!(
            "mixjam-techno-legacy-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let old_path = directory.join("old.json");
        let current_path = directory.join("current.json");
        fs::write(&old_path, serde_json::to_vec(&value).unwrap()).unwrap();
        let migrated = Project::load(&old_path).unwrap();
        assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(migrated.tracks.len(), 64);
        assert_eq!(migrated.tracks[..16], old_project.tracks);
        assert_eq!(migrated.clips, old_project.clips);
        assert_eq!(migrated.imported_samples, old_project.imported_samples);
        assert_eq!(migrated.loop_region, old_project.loop_region);
        assert_eq!(migrated.title, old_project.title);
        assert_eq!(migrated.author, old_project.author);
        assert_eq!(migrated.master_gain, old_project.master_gain);
        assert_eq!(migrated.bpm, 140.0);
        assert_eq!(migrated.pitch_ratio, 1.0);
        assert_eq!(migrated.import_notices.len(), 1);
        assert!((migrated.import_notices[0].source_bpm - 146.0).abs() < 1e-12);
        migrated.save(&current_path).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(&current_path).unwrap()).unwrap();
        assert_eq!(saved["schema_version"], PROJECT_SCHEMA_VERSION);
        assert_eq!(Project::load(&current_path).unwrap(), migrated);
        fs::remove_file(&old_path).unwrap();
        fs::remove_file(&current_path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn optional_source_and_notice_fields_keep_deserialization_defaults() {
        let mut value = serde_json::to_value(Project::default()).unwrap();
        value.as_object_mut().unwrap().remove("import_notices");
        value.as_object_mut().unwrap().remove("imported_samples");
        value.as_object_mut().unwrap().remove("libraries");
        assert_eq!(
            Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap(),
            Project::default()
        );
        value.as_object_mut().unwrap().remove("schema_version");
        value["tracks"].as_array_mut().unwrap().truncate(16);
        let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(migrated.imported_samples.is_empty());
        assert!(migrated.import_notices.is_empty());
        assert!(migrated.libraries.is_empty());
    }

    #[test]
    fn library_bindings_require_unique_products_and_nonempty_roots() {
        use crate::products::{LibraryBinding, Product};
        let mut project = Project {
            libraries: vec![
                LibraryBinding {
                    product: Product::T2,
                    root: PathBuf::from("t2-library"),
                },
                LibraryBinding {
                    product: Product::T1,
                    root: PathBuf::from("t1-library"),
                },
            ],
            imported_samples: vec![ImportedSample {
                id: 1_000_000,
                name: "Original WAV".into(),
                group_name: "Imported".into(),
                path: PathBuf::from("original.wav"),
                source_bpm: 180.0,
                origin: Some(Product::T1),
            }],
            ..Project::default()
        };
        project.validate().unwrap();
        let reopened = Project::parse_json(&serde_json::to_vec(&project).unwrap()).unwrap();
        assert_eq!(reopened, project);
        for origin in Product::ALL {
            project.imported_samples[0].origin = Some(origin);
            project.validate().unwrap();
        }
        project.libraries[1].product = Product::T2;
        assert!(project.validate().is_err());
        project.libraries[1].product = Product::T1;
        project.libraries[1].root.clear();
        assert!(project.validate().is_err());
    }

    #[test]
    fn move_is_atomic_and_adjacent_clips_fit() {
        let mut e = Editor::new(Project::default());
        let a = e.place(1, 0, 0, 256).unwrap();
        e.place(2, 0, 256, 256).unwrap();
        let before = e.project.clone();
        assert!(e.move_clip(a, 0, 128).is_err());
        assert_eq!(e.project, before);
        e.move_clip(a, 1, 128).unwrap();
        assert!(e.undo());
        assert_eq!(e.project, before);
        assert!(e.redo());
        assert_eq!(e.project.clips[0].track, 1);
    }
    #[test]
    fn invalid_edit_rolls_back_and_redo_branch_is_cleared() {
        let mut e = Editor::new(Project::default());
        assert!(e.transact(|p| p.bpm = f64::NAN).is_err());
        assert_eq!(e.project.bpm, 140.0);
        e.place(1, 0, 0, 256).unwrap();
        e.undo();
        e.place(2, 1, 0, 256).unwrap();
        assert!(!e.redo());
    }
    #[test]
    fn fractional_original_positions_survive_save() {
        let mut e = Editor::new(Project::default());
        e.place(13000, 0, 17, 31).unwrap();
        let p: Project = serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
        assert_eq!(p, e.project);
        assert!((p.frames_per_tick(44100) * 256.0 - 75600.0).abs() < 1e-8);
    }
    #[test]
    fn save_preserves_an_existing_temporary_file() {
        let directory =
            std::env::temp_dir().join(format!("mixjam-techno-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("mix.json");
        let temp = directory.join(format!(".mix.json.{}.tmp", std::process::id()));
        fs::write(&temp, b"unrelated existing file").unwrap();
        assert!(Project::default().save(&path).is_err());
        assert_eq!(fs::read(&temp).unwrap(), b"unrelated existing file");
        fs::remove_file(&temp).unwrap();
        let p = Project::default();
        p.save(&path).unwrap();
        assert_eq!(Project::load(&path).unwrap(), p);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }
    #[test]
    fn group_moves_validate_final_positions_and_undo_once() {
        let mut e = Editor::new(Project::default());
        let a = e.place(1, 0, 0, 256).unwrap();
        let b = e.place(2, 0, 256, 256).unwrap();
        let before = e.project.clone();
        e.move_clips(&[a, b], 0, 256, false).unwrap();
        assert_eq!(e.project.clips[0].start_tick, 256);
        assert!(e.undo());
        assert_eq!(e.project, before);
        assert!(e.move_clips(&[a, b], -1, 0, false).is_err());
        assert_eq!(e.project, before);
        let copied = e.move_clips(&[a, b], 1, 0, true).unwrap();
        assert_eq!(copied.len(), 2);
        assert_eq!(e.project.clips.len(), 4);
        assert!(e.undo());
        assert_eq!(e.project, before);
    }
}
