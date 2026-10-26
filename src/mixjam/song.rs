//! The application's own `.mixjam` song format, format version 12.
//!
//! It adapts the preserved version-7 document to this editor. The envelope is
//! the same: strict camelCase JSON with `formatVersion`, `appVersion`, two UTC
//! timestamps, `song`, and lanes that own their placements in visible order.
//! Versions 8 to 10 differ from version 7 where the editor does:
//! - Ticks are the editor's 256 per bar, not MixJam's 32.
//! - `lanes` always has 64 entries with gain, pan, and two channel switches.
//!   Master processors, return buses, and sends do not exist here.
//! - Version 9 separates nonnegative lane amplitude from polarity. Version 8
//!   signed lane gains migrate without changing their effective amplitude.
//! - Version 10 adds lane Mute and Solo. Earlier songs open with both off.
//! - Version 11 adds `master`, the master pedalboard with the eJay pedals.
//!   Earlier songs open with every pedal off, so they sound as they did.
//! - Version 12's `master` adds the input trim and the mastering pedals.
//!   Version 11 songs open with their eJay pedals first, in their saved
//!   order, and the mastering pedals off after them.
//! - A placement names its audio by `sampleId`. Product catalog IDs resolve
//!   through `libraries`; other IDs resolve through `samples`, which link WAV
//!   files by path. Placements also keep their trim offset and gain.
//! - Missing sources, conflict choices, import notices, and the settings that
//!   the original product saved are kept, so an imported song reopens without
//!   its original file.
use super::{Header, MAX_SONG_BYTES, text};
use crate::{
    engine::{
        Clip, ImportNotice, ImportedSample, MissingReference, MissingSample,
        PROJECT_SCHEMA_VERSION, Project, SampleConflict, TRACK_COUNT, Track,
    },
    master::{EjayCompressor, EjayEq, EjayWide, MasterChain, PedalKind},
    products::{LibraryBinding, Product},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) const FORMAT_VERSION: u32 = 12;

/// The `master` record. Its shape depends on the format version, so a
/// record that is not the current board is kept as JSON until the version
/// is known.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SavedMaster {
    Board(MasterChain),
    Other(serde_json::Value),
}
impl SavedMaster {
    fn json(self) -> serde_json::Value {
        match self {
            Self::Board(chain) => serde_json::to_value(chain).unwrap_or_default(),
            Self::Other(value) => value,
        }
    }
}

/// The version 11 `master` record: the eJay pedals alone.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Booster {
    order: [BoosterPedal; 3],
    ejay_eq: EjayEq,
    ejay_compressor: EjayCompressor,
    ejay_wide: EjayWide,
}
#[derive(Clone, Copy, Deserialize)]
enum BoosterPedal {
    #[serde(rename = "ejayEq")]
    Eq,
    #[serde(rename = "ejayCompressor")]
    Compressor,
    #[serde(rename = "ejayWide")]
    Wide,
}
impl Booster {
    /// The eJay pedals lead the board in their saved order; the mastering
    /// pedals follow, off.
    fn into_board(self) -> MasterChain {
        let mut chain = MasterChain {
            ejay_eq: self.ejay_eq,
            ejay_compressor: self.ejay_compressor,
            ejay_wide: self.ejay_wide,
            ..MasterChain::default()
        };
        chain.order[..3].copy_from_slice(&self.order.map(|pedal| match pedal {
            BoosterPedal::Eq => PedalKind::EjayEq,
            BoosterPedal::Compressor => PedalKind::EjayCompressor,
            BoosterPedal::Wide => PedalKind::EjayWide,
        }));
        chain
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    format_version: u32,
    app_version: String,
    created_at: String,
    modified_at: String,
    song: Song,
    // Required from version 11; earlier versions have no master pedalboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    master: Option<SavedMaster>,
    lanes: Vec<Lane>,
    samples: Vec<Sample>,
    missing_samples: Vec<Missing>,
    sample_conflicts: Vec<Conflict>,
    import_notices: Vec<Notice>,
    source_notices: Vec<String>,
    // Songs saved before this field existed, and songs without settings, omit it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    source_settings: BTreeMap<String, String>,
    libraries: Vec<LibraryBinding>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Song {
    title: String,
    author: String,
    bpm: f64,
    master_gain: f32,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    loop_region: Option<[u32; 2]>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lane {
    gain: f32,
    #[serde(
        default,
        deserialize_with = "crate::engine::optional_bool",
        skip_serializing_if = "Option::is_none"
    )]
    polarity_inverted: Option<bool>,
    pan: f32,
    left_enabled: bool,
    right_enabled: bool,
    #[serde(
        default,
        deserialize_with = "crate::engine::optional_bool",
        skip_serializing_if = "Option::is_none"
    )]
    muted: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::engine::optional_bool",
        skip_serializing_if = "Option::is_none"
    )]
    solo: Option<bool>,
    placements: Vec<Placement>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    id: u64,
    sample_id: u32,
    start_tick: u32,
    duration_ticks: u32,
    offset_frames: u32,
    gain: f32,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sample {
    id: u32,
    name: String,
    group_name: String,
    path: PathBuf,
    source_bpm: f64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    source_sample_rate: Option<u32>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    origin: Option<Product>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Missing {
    id: u32,
    name: String,
    source_bpm: f64,
    sample_rate: u32,
    nominal_ticks: u32,
    length_inferred: bool,
    reference: Reference,
    reason: String,
    metadata: BTreeMap<String, String>,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum Reference {
    Native {
        product: Product,
        #[serde(rename = "nativeId")]
        native_id: u32,
    },
    External {
        path: PathBuf,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        origin: Option<Product>,
    },
    Generated {
        product: Product,
        kind: String,
        data: serde_json::Value,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Conflict {
    reference: String,
    sample_ids: Vec<u32>,
    candidates: Vec<PathBuf>,
    selected: usize,
    confirmed: bool,
    source_spans: BTreeMap<u32, u32>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Notice {
    source_bpm: f64,
    target_bpm: f64,
}

impl Document {
    fn new(project: &Project, created_at: String, modified_at: String) -> Self {
        let mut lanes: Vec<Lane> = project
            .tracks
            .iter()
            .map(|track| Lane {
                gain: track.gain,
                polarity_inverted: Some(track.polarity_inverted),
                pan: track.pan,
                left_enabled: track.left_enabled,
                right_enabled: track.right_enabled,
                muted: Some(track.muted),
                solo: Some(track.solo),
                placements: Vec::new(),
            })
            .collect();
        for clip in &project.clips {
            lanes[clip.track].placements.push(Placement {
                id: clip.id,
                sample_id: clip.sample_id,
                start_tick: clip.start_tick,
                duration_ticks: clip.length_ticks,
                offset_frames: clip.offset_frames,
                gain: clip.gain,
            });
        }
        Self {
            format_version: FORMAT_VERSION,
            app_version: crate::APP_VERSION.into(),
            created_at,
            modified_at,
            song: Song {
                title: project.title.clone(),
                author: project.author.clone(),
                bpm: project.bpm,
                master_gain: project.master_gain,
                loop_region: project.loop_region,
            },
            master: Some(SavedMaster::Board(project.master)),
            lanes,
            samples: project
                .imported_samples
                .iter()
                .map(|sample| Sample {
                    id: sample.id,
                    name: sample.name.clone(),
                    group_name: sample.group_name.clone(),
                    path: sample.path.clone(),
                    source_bpm: sample.source_bpm,
                    source_sample_rate: sample.source_sample_rate,
                    origin: sample.origin,
                })
                .collect(),
            missing_samples: project
                .missing_samples
                .iter()
                .map(|sample| Missing {
                    id: sample.id,
                    name: sample.name.clone(),
                    source_bpm: sample.source_bpm,
                    sample_rate: sample.sample_rate,
                    nominal_ticks: sample.nominal_ticks,
                    length_inferred: sample.length_inferred,
                    reference: match sample.reference.clone() {
                        MissingReference::Native { product, native_id } => {
                            Reference::Native { product, native_id }
                        }
                        MissingReference::External { path, origin } => {
                            Reference::External { path, origin }
                        }
                        MissingReference::Generated {
                            product,
                            kind,
                            data,
                        } => Reference::Generated {
                            product,
                            kind,
                            data,
                        },
                    },
                    reason: sample.reason.clone(),
                    metadata: sample.metadata.clone(),
                })
                .collect(),
            sample_conflicts: project
                .sample_conflicts
                .iter()
                .map(|conflict| Conflict {
                    reference: conflict.reference.clone(),
                    sample_ids: conflict.sample_ids.clone(),
                    candidates: conflict.candidates.clone(),
                    selected: conflict.selected,
                    confirmed: conflict.confirmed,
                    source_spans: conflict.source_spans.clone(),
                })
                .collect(),
            import_notices: project
                .import_notices
                .iter()
                .map(|notice| Notice {
                    source_bpm: notice.source_bpm,
                    target_bpm: notice.target_bpm,
                })
                .collect(),
            source_notices: project.source_notices.clone(),
            source_settings: project.source_settings.clone(),
            libraries: project.libraries.clone(),
        }
    }

    fn into_project(self) -> Result<Project, String> {
        if !(8..=FORMAT_VERSION).contains(&self.format_version) {
            return Err("This MixJam format version is not supported.".into());
        }
        if !text(&self.app_version) || !text(&self.created_at) || !text(&self.modified_at) {
            return Err("The MixJam document metadata is invalid.".into());
        }
        if self.lanes.len() != TRACK_COUNT {
            return Err("The MixJam song must have 64 lanes.".into());
        }
        if self.format_version == 8
            && (!self.song.master_gain.is_finite() || !(0.0..=2.0).contains(&self.song.master_gain))
        {
            return Err("Legacy master amplitude must be from 0 to 2.".into());
        }
        let invalid =
            |error: serde_json::Error| format!("The master pedalboard is invalid: {error}");
        let master = match (self.format_version, self.master) {
            (FORMAT_VERSION, Some(SavedMaster::Board(master))) => master,
            // Reading the record again gives the reason it is not a board.
            (FORMAT_VERSION, Some(master)) => {
                serde_json::from_value(master.json()).map_err(invalid)?
            }
            (11, Some(master)) => serde_json::from_value::<Booster>(master.json())
                .map_err(invalid)?
                .into_board(),
            (version @ (11 | FORMAT_VERSION), None) => {
                return Err(format!("Version {version} songs require master."));
            }
            (_, None) => MasterChain::default(),
            (version, Some(_)) => {
                return Err(format!("Version {version} songs cannot contain master."));
            }
        };
        let mut tracks = Vec::with_capacity(TRACK_COUNT);
        let mut clips = Vec::new();
        for (track, lane) in self.lanes.into_iter().enumerate() {
            let mut settings = Track {
                gain: lane.gain,
                polarity_inverted: lane.polarity_inverted.unwrap_or(false),
                pan: lane.pan,
                left_enabled: lane.left_enabled,
                right_enabled: lane.right_enabled,
                muted: lane.muted.unwrap_or(false),
                solo: lane.solo.unwrap_or(false),
            };
            if self.format_version == 8 {
                if lane.polarity_inverted.is_some() {
                    return Err("Version 8 lanes cannot contain polarityInverted.".into());
                }
                if !lane.gain.is_finite() || !(-2.0..=4.0).contains(&lane.gain) {
                    return Err("Legacy track gain must be from -2 to 4.".into());
                }
                settings.set_legacy_gain(lane.gain);
            } else if lane.polarity_inverted.is_none() {
                return Err(format!(
                    "Version {} lanes require polarityInverted.",
                    self.format_version
                ));
            }
            if self.format_version < 10 {
                if lane.muted.is_some() || lane.solo.is_some() {
                    return Err(format!(
                        "Version {} lanes cannot contain muted or solo.",
                        self.format_version
                    ));
                }
            } else if lane.muted.is_none() || lane.solo.is_none() {
                return Err(format!(
                    "Version {} lanes require muted and solo.",
                    self.format_version
                ));
            }
            tracks.push(settings);
            clips.extend(lane.placements.into_iter().map(|placement| Clip {
                id: placement.id,
                sample_id: placement.sample_id,
                track,
                start_tick: placement.start_tick,
                length_ticks: placement.duration_ticks,
                offset_frames: placement.offset_frames,
                gain: placement.gain,
            }));
        }
        // Lanes group the placements. Identity order restores one arrangement
        // order for every lane layout, which is also the order of an import.
        clips.sort_by_key(|clip| clip.id);
        let project = Project {
            schema_version: PROJECT_SCHEMA_VERSION,
            title: self.song.title,
            author: self.song.author,
            bpm: self.song.bpm,
            master_gain: self.song.master_gain,
            pitch_ratio: 1.0,
            tracks,
            clips,
            loop_region: self.song.loop_region,
            imported_samples: self
                .samples
                .into_iter()
                .map(|sample| ImportedSample {
                    id: sample.id,
                    name: sample.name,
                    group_name: sample.group_name,
                    path: sample.path,
                    source_bpm: sample.source_bpm,
                    source_sample_rate: sample.source_sample_rate,
                    origin: sample.origin,
                })
                .collect(),
            missing_samples: self
                .missing_samples
                .into_iter()
                .map(|sample| MissingSample {
                    id: sample.id,
                    name: sample.name,
                    source_bpm: sample.source_bpm,
                    sample_rate: sample.sample_rate,
                    nominal_ticks: sample.nominal_ticks,
                    length_inferred: sample.length_inferred,
                    reference: match sample.reference {
                        Reference::Native { product, native_id } => {
                            MissingReference::Native { product, native_id }
                        }
                        Reference::External { path, origin } => {
                            MissingReference::External { path, origin }
                        }
                        Reference::Generated {
                            product,
                            kind,
                            data,
                        } => MissingReference::Generated {
                            product,
                            kind,
                            data,
                        },
                    },
                    reason: sample.reason,
                    metadata: sample.metadata,
                })
                .collect(),
            sample_conflicts: self
                .sample_conflicts
                .into_iter()
                .map(|conflict| SampleConflict {
                    reference: conflict.reference,
                    sample_ids: conflict.sample_ids,
                    candidates: conflict.candidates,
                    selected: conflict.selected,
                    confirmed: conflict.confirmed,
                    source_spans: conflict.source_spans,
                })
                .collect(),
            import_notices: self
                .import_notices
                .into_iter()
                .map(|notice| ImportNotice {
                    source_bpm: notice.source_bpm,
                    target_bpm: notice.target_bpm,
                })
                .collect(),
            source_notices: self.source_notices,
            source_settings: self.source_settings,
            libraries: self.libraries,
            master,
        };
        project.validate()?;
        Ok(project)
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Project, String> {
    serde_json::from_slice::<Document>(bytes)
        .map_err(|e| format!("Cannot read the MixJam song: {e}"))?
        .into_project()
}

pub(super) fn save(project: &Project, path: &Path) -> Result<(), String> {
    save_with_limit(project, path, MAX_SONG_BYTES)
}

fn save_with_limit(project: &Project, path: &Path, byte_limit: u64) -> Result<(), String> {
    project.validate()?;
    let now = timestamp(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs()),
    );
    // A song saved again keeps the creation time of the file that it replaces.
    let created_at = existing_creation_time(path).unwrap_or_else(|| now.clone());
    let mut bytes = serde_json::to_vec_pretty(&Document::new(project, created_at, now))
        .map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    if bytes.len() as u64 > byte_limit {
        return Err("The MixJam project exceeds the 1 GiB limit.".into());
    }
    crate::atomic_file::write(path, &bytes)
}

fn existing_creation_time(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_SONG_BYTES {
        return None;
    }
    // Skip unrelated fields as a stream. Save can replace a large destination
    // without allocating its entire contents just to keep its creation time.
    let header: Header =
        serde_json::from_reader(std::io::BufReader::new(file.take(MAX_SONG_BYTES + 1))).ok()?;
    ((8..=FORMAT_VERSION).contains(&header.format_version) && text(&header.created_at))
        .then_some(header.created_at)
}

/// ISO 8601 UTC time for seconds since the Unix epoch.
fn timestamp(seconds: u64) -> String {
    let (days, rest) = ((seconds / 86_400) as i64, seconds % 86_400);
    // Days to a Gregorian date, counted in 400-year eras that start on 1 March.
    let shifted = days + 719_468;
    let (era, day_of_era) = (shifted.div_euclid(146_097), shifted.rem_euclid(146_097));
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;

    fn project() -> Project {
        let imported = ImportedSample {
            id: crate::products::IMPORTED_ID_START,
            name: "Lead".into(),
            group_name: "Wave".into(),
            path: PathBuf::from("waves/lead.wav"),
            source_bpm: 140.0,
            source_sample_rate: Some(22_050),
            origin: Some(Product::T2),
        };
        let mut lost = imported.clone();
        lost.id += 1;
        let mut missing = MissingSample::external(&lost, 512, "Missing file.".into());
        missing
            .metadata
            .insert("Playback timing policy".into(), "mixjam-span".into());
        let clip = |id, sample_id, track| Clip {
            id,
            sample_id,
            track,
            start_tick: 256,
            length_ticks: 512,
            offset_frames: 4_410,
            gain: 0.75,
        };
        let mut project = Project {
            title: "Night drive".into(),
            author: "Owner".into(),
            bpm: 160.0,
            master_gain: 0.5,
            loop_region: Some([256, 1_024]),
            imported_samples: vec![imported.clone()].into(),
            missing_samples: vec![
                missing,
                MissingSample::native(
                    Product::T2,
                    7,
                    "Kick".into(),
                    140.0,
                    256,
                    false,
                    "No library.".into(),
                )
                .unwrap(),
            ],
            // Arrangement order differs from lane order.
            clips: vec![
                clip(1, imported.id, 63),
                clip(2, lost.id, 0),
                clip(3, Product::T2.global_sample_id(7).unwrap(), 5),
            ],
            sample_conflicts: vec![SampleConflict {
                reference: "lead.wav".into(),
                sample_ids: vec![imported.id],
                candidates: vec!["a/lead.wav".into(), "b/lead.wav".into()],
                selected: 1,
                confirmed: true,
                source_spans: BTreeMap::from([(imported.id, 512)]),
            }],
            import_notices: vec![ImportNotice {
                source_bpm: 155.0,
                target_bpm: 160.0,
            }],
            source_notices: vec!["Imported.".into()],
            source_settings: BTreeMap::from([
                ("BoostCompressorLED".into(), "1".into()),
                ("DrumU1_0".into(), String::new()),
            ]),
            libraries: vec![LibraryBinding {
                product: Product::T2,
                root: "library".into(),
            }],
            ..Project::default()
        };
        project.tracks[5] = Track {
            gain: 0.25,
            polarity_inverted: true,
            pan: -0.5,
            left_enabled: false,
            right_enabled: true,
            muted: true,
            solo: false,
        };
        project.tracks[6].solo = true;
        project.master.ejay_compressor.on = true;
        project.master.ejay_compressor.drive = 368;
        project.master.ejay_eq.bands[9] = 0;
        project.master.move_pedal(2, 0);
        project.master.trim = -2.5;
        project.master.limiter.on = true;
        project.master.limiter.ceiling = -0.3;
        project.master.tape.speed = 15;
        project.master.bus_compressor.attack = 0.1;
        project
    }

    /// The same song as an earlier format version saves it: no master pedalboard.
    fn earlier_document(expected: &mut Project, version: u32) -> serde_json::Value {
        expected.master = MasterChain::default();
        let mut document = serde_json::to_value(Document::new(
            expected,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap();
        document["formatVersion"] = version.into();
        document.as_object_mut().unwrap().remove("master");
        document
    }

    #[test]
    fn timestamps_follow_the_gregorian_calendar() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(951_782_400 + 86_399), "2000-02-29T23:59:59Z");
        assert_eq!(timestamp(1_791_676_800), "2026-10-11T00:00:00Z");
    }

    #[test]
    fn saved_song_is_a_version_12_document_that_reopens_unchanged() {
        let root = std::env::temp_dir().join(format!("mixjam-song-format-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("song.mixjam");
        let project = project();
        save(&project, &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["formatVersion"], 12);
        assert_eq!(document["appVersion"], crate::APP_VERSION);
        assert_eq!(document["master"]["ejayCompressor"]["on"], true);
        assert_eq!(document["master"]["ejayCompressor"]["drive"], 368);
        assert_eq!(document["master"]["ejayEq"]["bands"][9], 0);
        assert_eq!(document["master"]["ejayWide"]["on"], false);
        assert_eq!(
            document["master"]["order"],
            serde_json::json!([
                "ejayWide",
                "ejayEq",
                "ejayCompressor",
                "softClip",
                "tube",
                "trimEq",
                "busCompressor",
                "maximizer",
                "liftEq",
                "tape",
                "imaging",
                "multiband",
                "limiter"
            ])
        );
        // Settings keep their written form: decibels, percent, Hz, and ms.
        assert!(text.contains(r#""ceiling": -0.3"#), "{text}");
        assert!(text.contains(r#""attack": 0.1,"#));
        assert_eq!(document["master"]["trim"], -2.5);
        assert_eq!(document["master"]["limiter"]["on"], true);
        assert_eq!(document["master"]["tape"]["speed"], 15);
        assert_eq!(document["master"]["imaging"]["monoBelow"], 120.0);
        assert_eq!(document["song"]["masterGain"], 0.5);
        assert_eq!(document["lanes"].as_array().unwrap().len(), 64);
        assert_eq!(document["lanes"][63]["placements"][0]["durationTicks"], 512);
        assert_eq!(document["lanes"][5]["leftEnabled"], false);
        assert_eq!(document["lanes"][5]["polarityInverted"], true);
        assert_eq!(document["lanes"][5]["muted"], true);
        assert_eq!(document["lanes"][5]["solo"], false);
        assert_eq!(document["lanes"][6]["solo"], true);
        assert_eq!(document["missingSamples"][1]["reference"]["nativeId"], 7);
        assert_eq!(
            document.as_object().unwrap().keys().collect::<Vec<_>>(),
            [
                "appVersion",
                "createdAt",
                "formatVersion",
                "importNotices",
                "lanes",
                "libraries",
                "master",
                "missingSamples",
                "modifiedAt",
                "sampleConflicts",
                "samples",
                "song",
                "sourceNotices",
                "sourceSettings"
            ]
        );
        assert_eq!(document["sourceSettings"]["BoostCompressorLED"], "1");
        assert!(text.ends_with(
            "}
"
        ));
        assert_eq!(
            crate::mixjam::load_song(&path, &Library::empty())
                .unwrap()
                .0,
            project
        );

        // The creation time belongs to the file; later saves change only modifiedAt.
        document["createdAt"] = "2020-01-02T03:04:05Z".into();
        document["modifiedAt"] = "2020-01-02T03:04:05Z".into();
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        save(&project, &path).unwrap();
        let resaved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(resaved["createdAt"], "2020-01-02T03:04:05Z");
        assert_ne!(resaved["modifiedAt"], "2020-01-02T03:04:05Z");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn version_8_signed_lanes_migrate_and_save_as_11_without_losing_polarity() {
        let root = std::env::temp_dir().join(format!("mixjam-polarity-v8-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("song.mixjam");
        let mut expected = project();
        for track in &mut expected.tracks {
            (track.muted, track.solo) = (false, false);
        }
        expected.tracks[0].gain = 4.0;
        expected.tracks[1].gain = 0.0;
        expected.tracks[1].polarity_inverted = true;
        expected.tracks[2].set_legacy_gain(-2.0 / 65536.0);
        let mut legacy = earlier_document(&mut expected, 8);
        for (lane, track) in legacy["lanes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(&expected.tracks)
        {
            let lane = lane.as_object_mut().unwrap();
            for key in ["polarityInverted", "muted", "solo"] {
                lane.remove(key);
            }
            lane.insert("gain".into(), track.effective_gain().into());
        }
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let (migrated, replaceable) = crate::mixjam::load_song(&path, &Library::empty()).unwrap();
        assert!(replaceable);
        assert_eq!(migrated, expected);
        assert!(migrated.tracks[1].polarity_inverted);
        save(&migrated, &path).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["formatVersion"], 12);
        assert_eq!(saved["createdAt"], "2020-01-02T03:04:05Z");
        assert_eq!(saved["lanes"][1]["gain"], 0.0);
        assert_eq!(saved["lanes"][1]["polarityInverted"], true);
        assert_eq!(
            crate::mixjam::load_song(&path, &Library::empty())
                .unwrap()
                .0,
            expected
        );
        legacy["lanes"][0]["gain"] = (-2.01).into();
        assert!(parse(&serde_json::to_vec(&legacy).unwrap()).is_err());
        legacy["lanes"][0]["gain"] = 1.0.into();
        legacy["lanes"][0]["polarityInverted"] = false.into();
        assert!(parse(&serde_json::to_vec(&legacy).unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn version_9_songs_open_unmuted_with_their_outputs_and_save_as_11() {
        let root = std::env::temp_dir().join(format!("mixjam-switches-v9-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("song.mixjam");
        let mut expected = project();
        for track in &mut expected.tracks {
            (track.muted, track.solo) = (false, false);
        }
        let mut earlier = earlier_document(&mut expected, 9);
        for lane in earlier["lanes"].as_array_mut().unwrap() {
            let lane = lane.as_object_mut().unwrap();
            lane.remove("muted");
            lane.remove("solo");
        }
        fs::write(&path, serde_json::to_vec(&earlier).unwrap()).unwrap();
        let (opened, replaceable) = crate::mixjam::load_song(&path, &Library::empty()).unwrap();
        assert!(replaceable);
        // L/R states stay as saved, including outputs an earlier import disabled.
        assert_eq!(opened, expected);
        assert!(!opened.tracks[5].left_enabled);
        save(&opened, &path).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["formatVersion"], 12);
        assert_eq!(saved["createdAt"], "2020-01-02T03:04:05Z");
        assert_eq!(saved["lanes"][5]["muted"], false);
        for key in ["muted", "solo"] {
            let mut changed = earlier.clone();
            changed["lanes"][0][key] = false.into();
            assert!(
                parse(&serde_json::to_vec(&changed).unwrap())
                    .unwrap_err()
                    .contains("cannot contain muted or solo")
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn version_10_songs_open_with_every_pedal_off_and_version_12_requires_master() {
        let mut expected = project();
        let earlier = earlier_document(&mut expected, 10);
        let opened = parse(&serde_json::to_vec(&earlier).unwrap()).unwrap();
        assert_eq!(opened, expected);
        assert!(!opened.master.is_active());

        let mut with_master = earlier.clone();
        with_master["master"] = serde_json::to_value(MasterChain::default()).unwrap();
        assert!(
            parse(&serde_json::to_vec(&with_master).unwrap())
                .unwrap_err()
                .contains("Version 10 songs cannot contain master")
        );
        let current =
            serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
        let mut without = current.clone();
        without.as_object_mut().unwrap().remove("master");
        assert!(
            parse(&serde_json::to_vec(&without).unwrap())
                .unwrap_err()
                .contains("require master")
        );
        for (pointer, value) in [
            ("/master/ejayEq/bands/0", serde_json::Value::from(101)),
            ("/master/ejayCompressor/speed", 1001.into()),
            ("/master/ejayWide/spread", (-1).into()),
            ("/master/ejayWide/on", serde_json::Value::Null),
            ("/master/order/0", "ejayEq".into()),
            ("/master/order", serde_json::json!(["ejayEq", "ejayWide"])),
            ("/master/order/2", "limiter".into()),
            ("/master/order/12", "tape".into()),
            ("/master/order/5", "reverb".into()),
            ("/master/limiter/ceiling", 0.5.into()),
            ("/master/tape/speed", 20.into()),
            ("/master/trim", "loud".into()),
            ("/master/busCompressor/ratio", 1.into()),
        ] {
            let mut changed = current.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(
                parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{pointer}"
            );
        }
        let mut unknown = current;
        unknown["master"]["ejayWide"]["width"] = 3.into();
        assert!(
            parse(&serde_json::to_vec(&unknown).unwrap())
                .unwrap_err()
                .contains("unknown field `width`")
        );
    }

    #[test]
    fn version_11_songs_open_with_their_ejay_pedals_first_and_the_rest_off() {
        let mut expected = project();
        let mut document = earlier_document(&mut expected, 11);
        document["master"] = serde_json::json!({
            "order": ["ejayWide", "ejayCompressor", "ejayEq"],
            "ejayEq": {"on": false, "bands": [50, 50, 50, 50, 50, 50, 50, 50, 50, 0]},
            "ejayCompressor": {"on": true, "drive": 368, "speed": 500, "gain": 500},
            "ejayWide": {"on": true, "spread": 700},
        });
        let opened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
        let master = opened.master;
        assert_eq!(
            master.order[..3],
            [
                PedalKind::EjayWide,
                PedalKind::EjayCompressor,
                PedalKind::EjayEq
            ]
        );
        assert_eq!(master.order[3..], PedalKind::ALL[3..]);
        assert!(master.ejay_compressor.on && master.ejay_wide.on && !master.ejay_eq.on);
        assert_eq!((master.ejay_wide.spread, master.ejay_eq.bands[9]), (700, 0));
        assert!(
            PedalKind::ALL
                .iter()
                .filter(|kind| !kind.is_ejay())
                .all(|kind| !master.is_on(*kind))
        );
        assert_eq!(master.trim, 0.0);
        assert_eq!(opened, Project { master, ..expected });

        // A version 11 song keeps the eJay record's strict shape.
        for (pointer, value) in [
            ("/master/order", serde_json::json!(["ejayEq", "ejayWide"])),
            ("/master/order/0", "ejayCompressor".into()),
            ("/master/ejayWide/spread", 1001.into()),
        ] {
            let mut changed = document.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(
                parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{pointer}"
            );
        }
        let mut later = document.clone();
        later["master"]["trim"] = 0.into();
        assert!(parse(&serde_json::to_vec(&later).unwrap()).is_err());
        let mut without = document;
        without.as_object_mut().unwrap().remove("master");
        assert!(
            parse(&serde_json::to_vec(&without).unwrap())
                .unwrap_err()
                .contains("Version 11 songs require master")
        );
    }

    #[test]
    fn version_10_requires_explicit_mute_and_solo() {
        let document = earlier_document(&mut project(), 10);
        for key in ["muted", "solo"] {
            let mut changed = document.clone();
            changed["lanes"][0].as_object_mut().unwrap().remove(key);
            assert!(
                parse(&serde_json::to_vec(&changed).unwrap())
                    .unwrap_err()
                    .contains("require muted and solo")
            );
            for value in [serde_json::Value::Null, 0.into(), "false".into()] {
                changed = document.clone();
                changed["lanes"][0][key] = value;
                assert!(parse(&serde_json::to_vec(&changed).unwrap()).is_err());
            }
        }
    }

    #[test]
    fn version_9_requires_explicit_polarity_and_nonnegative_amplitude() {
        let document =
            serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
        let mut changed = document.clone();
        changed["lanes"][0]
            .as_object_mut()
            .unwrap()
            .remove("polarityInverted");
        assert!(
            parse(&serde_json::to_vec(&changed).unwrap())
                .unwrap_err()
                .contains("polarityInverted")
        );
        for value in [serde_json::Value::Null, 0.into(), "false".into()] {
            changed = document.clone();
            changed["lanes"][0]["polarityInverted"] = value;
            assert!(parse(&serde_json::to_vec(&changed).unwrap()).is_err());
        }
        for amplitude in [-0.5, 4.01] {
            changed = document.clone();
            changed["lanes"][0]["gain"] = amplitude.into();
            assert!(parse(&serde_json::to_vec(&changed).unwrap()).is_err());
        }
    }

    #[test]
    fn rejects_unknown_keys_missing_lanes_and_invalid_arrangement_data() {
        let document =
            serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
        for (pointer, value, expected) in [
            ("/song/pitchRatio", serde_json::json!(1.0), "unknown field"),
            (
                "/lanes/0/sends",
                serde_json::json!([0, 0, 0, 0]),
                "unknown field",
            ),
            ("/lanes", serde_json::json!([]), "64 lanes"),
            ("/song/bpm", serde_json::json!(127), "Tempo"),
            ("/lanes/0/placements/0/id", serde_json::json!(1), "unique"),
            ("/createdAt", serde_json::json!(""), "metadata"),
        ] {
            let mut changed = document.clone();
            match changed.pointer_mut(pointer) {
                Some(target) => *target = value,
                None => {
                    let (parent, key) = pointer.rsplit_once('/').unwrap();
                    changed.pointer_mut(parent).unwrap()[key] = value;
                }
            }
            let error = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
            assert!(error.contains(expected), "{pointer}: {error}");
        }
    }

    #[test]
    fn nullable_fields_are_required_but_accept_null() {
        let document =
            serde_json::to_value(Document::new(&project(), timestamp(0), timestamp(0))).unwrap();
        for (parent, key) in [
            ("/song", "loopRegion"),
            ("/samples/0", "sourceSampleRate"),
            ("/samples/0", "origin"),
            ("/missingSamples/0/reference", "origin"),
        ] {
            let mut changed = document.clone();
            changed.pointer_mut(parent).unwrap()[key] = serde_json::Value::Null;
            parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
            changed
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let error = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
            assert!(error.contains("missing field"), "{parent}/{key}: {error}");
        }
    }

    #[test]
    fn saving_over_an_oversized_destination_does_not_read_its_contents() {
        let path = std::env::temp_dir().join(format!(
            "mixjam-oversized-save-{}.mixjam",
            std::process::id()
        ));
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_SONG_BYTES + 1).unwrap();
        drop(file);
        assert!(existing_creation_time(&path).is_none());
        save(&Project::default(), &path).unwrap();
        assert_eq!(
            crate::mixjam::load_song(&path, &Library::empty())
                .unwrap()
                .0,
            Project {
                // An untitled song opens under its file name.
                title: path.file_stem().unwrap().to_string_lossy().into_owned(),
                ..Project::default()
            }
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn serialized_song_limit_includes_formatting_and_preserves_the_destination() {
        let path =
            std::env::temp_dir().join(format!("mixjam-save-limit-{}.mixjam", std::process::id()));
        let project = Project::default();
        save(&project, &path).unwrap();
        let saved_length = fs::metadata(&path).unwrap().len();
        let compact =
            serde_json::to_vec(&Document::new(&project, timestamp(0), timestamp(0))).unwrap();
        assert!((compact.len() as u64) < saved_length);

        fs::write(&path, b"previous song").unwrap();
        for limit in [compact.len() as u64, saved_length - 1] {
            assert!(
                save_with_limit(&project, &path, limit)
                    .unwrap_err()
                    .contains("limit")
            );
            assert_eq!(fs::read(&path).unwrap(), b"previous song");
        }
        save_with_limit(&project, &path, saved_length).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), saved_length);
        assert_eq!(parse(&fs::read(&path).unwrap()).unwrap(), project);
        fs::remove_file(&path).unwrap();
        assert!(save_with_limit(&project, &path, saved_length - 1).is_err());
        assert!(!path.exists());
    }
}
