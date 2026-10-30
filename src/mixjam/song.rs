//! The application's own `.mixjam` song format, format version 18.
//!
//! It adapts the preserved version-7 document to this editor. The envelope is
//! the same: strict camelCase JSON with `formatVersion`, `appVersion`, two UTC
//! timestamps, `song`, and lanes that own their placements in visible order.
//! Versions 8 to 16 differ from version 7 where the editor does:
//! - Ticks are the editor's 256 per bar, not MixJam's 32.
//! - `lanes` always has 64 entries with gain, pan, and two channel switches.
//! - Version 9 separates nonnegative lane amplitude from polarity. Version 8
//!   signed lane gains migrate without changing their effective amplitude.
//! - Version 10 adds lane Mute and Solo. Earlier songs open with both off.
//! - Version 11 adds `master`, the master pedalboard with the Booster pedals.
//!   Earlier songs open with every pedal off, so they sound as they did.
//! - Version 12's `master` adds the input trim and the mastering pedals.
//!   Version 11 songs open with their Booster pedals first, in their saved
//!   order, and the mastering pedals off after them.
//! - Version 13 adds `fxBuses`, the four FX buses with their effects, and
//!   each lane's `sends` to them, as version 7 saved them. Earlier songs
//!   open with every bus empty and every send at 0.
//! - Version 14 adds clip fades, clip pitch, and lane automation: a
//!   placement may hold `fades` and `pitchCents`, and a lane `automation`.
//!   Without them a clip has no fades and keeps its pitch, and a lane has no
//!   automation.
//! - Version 15 adds the channel EQ, a lane's `eq`, and the master board's
//!   `bypass`. A lane without `eq` has its EQ off at the default settings,
//!   and a board without `bypass` is not bypassed.
//! - Version 16 renames the Booster pedals' keys in `master` and its
//!   `order` (`BOOSTER_KEYS`). Versions 11 to 15 keep their earlier keys
//!   and open with the same settings; only version 16 accepts the new keys.
//! - Version 17 adds the lane ducker, a lane's `ducker`, and the keys of the
//!   dynamics pedals (`key`) and of the FX duckers (`duckKey`). A lane
//!   without `ducker` has its ducker off at the default settings, and a
//!   pedal or effect without its key listens to its own input.
//! - Version 18 adds the channel EQ's second peak (`peak2Hz`, `peak2Db`,
//!   `peak2Q` in a lane's `eq`), the lane compressor, a lane's
//!   `compressor`, and the lane's stereo `width`. Without them the second
//!   peak is flat, the compressor is off at the default settings, and the
//!   lane is at 100% width. A lane ducker's key leaves out the master gain
//!   from version 18, so a version 17 lane ducker's threshold opens moved
//!   by `-20 * log10(masterGain)` dB, held to its range, and ducks as it did.
//! - A placement names its audio by `sampleId`. Product catalog IDs resolve
//!   through `libraries`; other IDs resolve through `samples`, which link WAV
//!   files by path. Placements also keep their trim offset and gain.
//! - Missing sources, conflict choices, import notices, and the settings that
//!   the original product saved are kept, so an imported song reopens without
//!   its original file.
use super::{Header, MAX_SONG_BYTES, text};
use crate::{
    engine::{
        Clip, Fades, ImportNotice, ImportedSample, MissingReference, MissingSample,
        PROJECT_SCHEMA_VERSION, Project, SampleConflict, TRACK_COUNT, TRACK_WIDTH, Track,
    },
    master::{BoosterCompressor, BoosterEq, BoosterWide, MasterChain, PedalKind},
    products::{LibraryBinding, Product},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) const FORMAT_VERSION: u32 = 18;
/// The channel EQ's keys that version 18 added.
const PEAK2_KEYS: [&str; 3] = ["peak2Hz", "peak2Db", "peak2Q"];
/// The Booster pedals' keys in a saved board and its `order`: the key of
/// versions 11 to 15, then the key of version 16 and later. The earlier
/// keys must match the saved files byte for byte.
const BOOSTER_KEYS: [(&str, &str); 3] = [
    ("ejayEq", "boosterEq"),
    ("ejayCompressor", "boosterCompressor"),
    ("ejayWide", "boosterWide"),
];

/// A lane ducker's threshold for a key without the master gain, from a
/// version 17 song, whose key held the master gain. The key now reads
/// `-20 * log10(masterGain)` dB hotter, so the threshold moves by as much
/// and the lane ducks as it did, as far as the threshold's range allows. A
/// master gain of 0 or less left the key silent, as it still is: the
/// threshold stays. So does a threshold outside its range, which validation
/// then rejects, as it did before.
fn threshold_without_master_gain(threshold: f32, master_gain: f32) -> f32 {
    let range = crate::ducker::DuckerParam::Threshold.range();
    if !(master_gain.is_finite() && master_gain > 0.0)
        || !(range.low..=range.high).contains(&threshold)
    {
        return threshold;
    }
    let moved = f64::from(threshold) - 20.0 * f64::from(master_gain).log10();
    (moved as f32).clamp(range.low, range.high)
}

/// Delay settings admission until the document version is known. Retain raw
/// input so explicit zero keys and duplicate fields are not lost. Save typed
/// settings directly to preserve their f32 precision and normal formatting.
#[derive(Serialize)]
#[serde(untagged)]
enum Saved<T> {
    Known(T),
    Raw(Box<serde_json::value::RawValue>),
}

impl<'de, T> Deserialize<'de> for Saved<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Box::<serde_json::value::RawValue>::deserialize(deserializer).map(Self::Raw)
    }
}

impl<T: Serialize + serde::de::DeserializeOwned> Saved<T> {
    fn json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn settings(self) -> Result<T, serde_json::Error> {
        match self {
            Self::Known(settings) => Ok(settings),
            Self::Raw(raw) => serde_json::from_str(raw.get()),
        }
    }
}

/// The version 11 `master` record: the Booster pedals alone, with the
/// keys of version 16.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Booster {
    order: [BoosterPedal; 3],
    booster_eq: BoosterEq,
    booster_compressor: BoosterCompressor,
    booster_wide: BoosterWide,
}
#[derive(Clone, Copy, Deserialize)]
enum BoosterPedal {
    #[serde(rename = "boosterEq")]
    Eq,
    #[serde(rename = "boosterCompressor")]
    Compressor,
    #[serde(rename = "boosterWide")]
    Wide,
}
impl Booster {
    /// The Booster pedals lead the board in their saved order; the mastering
    /// pedals follow, off.
    fn into_board(self) -> MasterChain {
        let mut chain = MasterChain {
            booster_eq: self.booster_eq,
            booster_compressor: self.booster_compressor,
            booster_wide: self.booster_wide,
            ..MasterChain::default()
        };
        chain.order[..3].copy_from_slice(&self.order.map(|pedal| match pedal {
            BoosterPedal::Eq => PedalKind::BoosterEq,
            BoosterPedal::Compressor => PedalKind::BoosterCompressor,
            BoosterPedal::Wide => PedalKind::BoosterWide,
        }));
        chain
    }
}

/// A version 11 to 15 board with the Booster pedals' earlier keys, in
/// the record and in its `order`, changed to the current keys. These
/// versions reject the current keys.
fn current_booster_keys(
    mut master: serde_json::Value,
    version: u32,
) -> Result<serde_json::Value, String> {
    let rejected = |key: &str| format!("Version {version} songs cannot contain master {key}.");
    let Some(record) = master.as_object_mut() else {
        return Ok(master);
    };
    for (earlier, current) in BOOSTER_KEYS {
        if record.contains_key(current) {
            return Err(rejected(current));
        }
        if let Some(pedal) = record.remove(earlier) {
            record.insert(current.into(), pedal);
        }
    }
    if let Some(order) = record
        .get_mut("order")
        .and_then(serde_json::Value::as_array_mut)
    {
        for name in order {
            for (earlier, current) in BOOSTER_KEYS {
                if name.as_str() == Some(current) {
                    return Err(rejected(current));
                }
                if name.as_str() == Some(earlier) {
                    *name = current.into();
                }
            }
        }
    }
    Ok(master)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    format_version: u32,
    app_version: String,
    created_at: String,
    modified_at: String,
    song: Song,
    // Keep the raw settings until the version is checked. Deserializing the
    // current types first loses the presence of optional keys set to zero.
    // Required from version 11; earlier versions have no master pedalboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    master: Option<Saved<MasterChain>>,
    // Required from version 13; earlier versions have no FX buses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fx_buses: Option<Saved<crate::fx::FxBuses>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sends: Option<[f32; crate::fx::BUSES]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    automation: Option<crate::automation::Automation>,
    // Raw until the version is checked, as the second peak's keys are
    // optional in the EQ record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    eq: Option<Saved<crate::eq::ChannelEq>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compressor: Option<crate::compressor::Compressor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ducker: Option<crate::ducker::Ducker>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fades: Option<Fades>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pitch_cents: Option<i32>,
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
                sends: Some(track.sends),
                automation: (!track.automation.is_empty()).then(|| track.automation.clone()),
                eq: (!track.eq.is_default()).then_some(Saved::Known(track.eq)),
                compressor: (!track.compressor.is_default()).then_some(track.compressor),
                width: (track.width != TRACK_WIDTH.default).then_some(track.width),
                ducker: (!track.ducker.is_default()).then_some(track.ducker),
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
                fades: (clip.fades != Fades::default()).then_some(clip.fades),
                pitch_cents: (clip.pitch_cents != 0).then_some(clip.pitch_cents),
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
            master: Some(Saved::Known(project.master)),
            fx_buses: Some(Saved::Known(project.fx)),
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
        if self.format_version < 17 {
            let master_key = self.master.as_ref().is_some_and(|master| {
                let master = master.json();
                [
                    "ejayCompressor",
                    "boosterCompressor",
                    "busCompressor",
                    "multiband",
                    "limiter",
                ]
                .into_iter()
                .any(|name| {
                    master
                        .get(name)
                        .is_some_and(|pedal| pedal.get("key").is_some())
                })
            });
            let fx_key = self
                .fx_buses
                .as_ref()
                .map(Saved::json)
                .as_ref()
                .and_then(serde_json::Value::as_array)
                .is_some_and(|buses| {
                    buses.iter().any(|bus| {
                        bus.get("module")
                            .is_some_and(|module| module.get("duckKey").is_some())
                    })
                });
            if master_key || fx_key {
                return Err(format!(
                    "Version {} songs cannot contain key or duckKey.",
                    self.format_version
                ));
            }
        }
        let invalid =
            |error: serde_json::Error| format!("The master pedalboard is invalid: {error}");
        let master = match (self.format_version, self.master) {
            (16.., Some(master)) => master.settings().map_err(invalid)?,
            (version @ 12.., Some(master)) => {
                serde_json::from_value(current_booster_keys(master.json(), version)?)
                    .map_err(invalid)?
            }
            (11, Some(master)) => {
                serde_json::from_value::<Booster>(current_booster_keys(master.json(), 11)?)
                    .map_err(invalid)?
                    .into_board()
            }
            (version @ 11.., None) => {
                return Err(format!("Version {version} songs require master."));
            }
            (_, None) => MasterChain::default(),
            (version, Some(_)) => {
                return Err(format!("Version {version} songs cannot contain master."));
            }
        };
        if self.format_version < 15 && master.bypass {
            return Err(format!(
                "Version {} songs cannot contain master bypass.",
                self.format_version
            ));
        }
        let fx = match (self.format_version, self.fx_buses) {
            (13.., Some(buses)) => buses
                .settings()
                .map_err(|error| format!("The FX buses are invalid: {error}"))?,
            (version @ 13.., None) => {
                return Err(format!("Version {version} songs require fxBuses."));
            }
            (_, None) => crate::fx::FxBuses::default(),
            (version, Some(_)) => {
                return Err(format!("Version {version} songs cannot contain fxBuses."));
            }
        };
        let mut tracks = Vec::with_capacity(TRACK_COUNT);
        let mut clips = Vec::new();
        for (track, lane) in self.lanes.into_iter().enumerate() {
            let sends = match (self.format_version, lane.sends) {
                (13.., Some(sends)) => sends,
                (version @ 13.., None) => {
                    return Err(format!("Version {version} lanes require sends."));
                }
                (_, None) => [0.0; crate::fx::BUSES],
                (version, Some(_)) => {
                    return Err(format!("Version {version} lanes cannot contain sends."));
                }
            };
            if self.format_version < 14 && lane.automation.is_some() {
                return Err(format!(
                    "Version {} lanes cannot contain automation.",
                    self.format_version
                ));
            }
            if self.format_version < 17 && lane.ducker.is_some() {
                return Err(format!(
                    "Version {} lanes cannot contain ducker.",
                    self.format_version
                ));
            }
            if self.format_version < 18 && (lane.compressor.is_some() || lane.width.is_some()) {
                return Err(format!(
                    "Version {} lanes cannot contain compressor or width.",
                    self.format_version
                ));
            }
            if self.format_version < 15 && lane.eq.is_some() {
                return Err(format!(
                    "Version {} lanes cannot contain eq.",
                    self.format_version
                ));
            }
            let eq = match lane.eq {
                Some(eq) => {
                    if self.format_version < 18
                        && eq
                            .json()
                            .as_object()
                            .is_some_and(|eq| PEAK2_KEYS.iter().any(|key| eq.contains_key(*key)))
                    {
                        return Err(format!(
                            "Version {} lanes cannot contain eq peak2Hz, peak2Db, or peak2Q.",
                            self.format_version
                        ));
                    }
                    eq.settings()
                        .map_err(|error| format!("The channel EQ is invalid: {error}"))?
                }
                None => Default::default(),
            };
            let mut settings = Track {
                gain: lane.gain,
                polarity_inverted: lane.polarity_inverted.unwrap_or(false),
                pan: lane.pan,
                left_enabled: lane.left_enabled,
                right_enabled: lane.right_enabled,
                muted: lane.muted.unwrap_or(false),
                solo: lane.solo.unwrap_or(false),
                sends,
                automation: lane.automation.unwrap_or_default(),
                eq,
                compressor: lane.compressor.unwrap_or_default(),
                width: lane.width.unwrap_or(TRACK_WIDTH.default),
                ducker: lane.ducker.unwrap_or_default(),
            };
            if self.format_version < 18 && lane.ducker.is_some() {
                settings.ducker.threshold =
                    threshold_without_master_gain(settings.ducker.threshold, self.song.master_gain);
            }
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
            for placement in lane.placements {
                if self.format_version < 14
                    && (placement.fades.is_some() || placement.pitch_cents.is_some())
                {
                    return Err(format!(
                        "Version {} placements cannot contain fades or pitchCents.",
                        self.format_version
                    ));
                }
                clips.push(Clip {
                    id: placement.id,
                    sample_id: placement.sample_id,
                    track,
                    start_tick: placement.start_tick,
                    length_ticks: placement.duration_ticks,
                    offset_frames: placement.offset_frames,
                    gain: placement.gain,
                    fades: placement.fades.unwrap_or_default(),
                    pitch_cents: placement.pitch_cents.unwrap_or(0),
                });
            }
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
            fx,
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
            fades: Default::default(),
            pitch_cents: 0,
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
            sends: [0.25, 0.0, 0.5, 1.0],
            automation: Default::default(),
            eq: crate::eq::ChannelEq {
                on: true,
                low_cut: 80.0,
                high_db: -7.5,
                ..Default::default()
            },
            compressor: crate::compressor::Compressor {
                on: true,
                threshold: -24.0,
                makeup: 3.0,
                ..Default::default()
            },
            width: 150.0,
            ducker: crate::ducker::Ducker {
                on: true,
                key: 1,
                depth: 9.5,
                ..Default::default()
            },
        };
        project.tracks[6].solo = true;
        // An EQ that is off keeps its settings.
        project.tracks[7].eq.mid_db = 3.0;
        project.tracks[7].eq.peak2_db = -4.5;
        project.tracks[9].width = 0.0;
        project.clips[0].fades = Fades {
            fade_in: crate::engine::Fade {
                ticks: 64,
                curve: crate::engine::FadeCurve::EqualPower,
            },
            fade_out: crate::engine::Fade {
                ticks: 128,
                curve: crate::engine::FadeCurve::Exponential,
            },
        };
        project.clips[0].pitch_cents = -150;
        project.tracks[5].automation.volume = vec![
            crate::automation::Point {
                tick: 0,
                value: -60.0,
            },
            crate::automation::Point {
                tick: 512,
                value: 0.0,
            },
        ];
        project.tracks[5].automation.sends[1] = vec![crate::automation::Point {
            tick: 256,
            value: 0.75,
        }];
        project.tracks[5].automation.filter = vec![crate::automation::Point {
            tick: 1024,
            value: -0.5,
        }];
        project.fx.buses[0].module = crate::fx::Module::AetherformReverb(
            crate::fx::Aetherform::preset("Dark Hall").unwrap(),
        );
        project.fx.buses[0].return_level = 0.92;
        project.fx.buses[2].module =
            crate::fx::Module::EchoformDelay(crate::fx::Echoform::preset("Dub Feedback").unwrap());
        project.fx.buses[2].powered = false;
        project.fx.buses[3].limiter = false;
        project.master.booster_compressor.on = true;
        project.master.booster_compressor.drive = 368;
        project.master.booster_eq.bands[9] = 0;
        project.master.move_pedal(2, 0);
        project.master.trim = -2.5;
        project.master.limiter.on = true;
        project.master.limiter.ceiling = -0.3;
        project.master.tape.speed = 15;
        project.master.bus_compressor.attack = 0.1;
        project.master.bypass = true;
        project
    }

    /// A track as a version 17 song saves it: no second EQ peak, no
    /// compressor, and 100% width.
    fn before_version_18(track: &mut Track) {
        use crate::eq::EqParam;
        for param in [EqParam::Peak2Hz, EqParam::Peak2Db, EqParam::Peak2Q] {
            param.set(&mut track.eq, param.range().default);
        }
        track.compressor = Default::default();
        track.width = TRACK_WIDTH.default;
    }

    /// The same song as an earlier format version saves it: no master
    /// pedalboard, no FX buses, no sends, no clip fades or pitch, no
    /// automation, and no channel EQ.
    fn earlier_document(expected: &mut Project, version: u32) -> serde_json::Value {
        expected.master = MasterChain::default();
        expected.fx = crate::fx::FxBuses::default();
        for track in &mut expected.tracks {
            track.sends = [0.0; crate::fx::BUSES];
            track.automation = Default::default();
            track.eq = Default::default();
            track.ducker = Default::default();
            before_version_18(track);
        }
        for clip in &mut expected.clips {
            clip.fades = Fades::default();
            clip.pitch_cents = 0;
        }
        let mut document = serde_json::to_value(Document::new(
            expected,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap();
        document["formatVersion"] = version.into();
        document.as_object_mut().unwrap().remove("master");
        document.as_object_mut().unwrap().remove("fxBuses");
        for lane in document["lanes"].as_array_mut().unwrap() {
            lane.as_object_mut().unwrap().remove("sends");
        }
        document
    }

    /// The board of a version 11 to 15 document: the Booster pedals'
    /// current keys, in the record and its order, changed to the earlier ones.
    fn with_earlier_booster_keys(document: &mut serde_json::Value) {
        let record = document["master"].as_object_mut().unwrap();
        for (earlier, current) in BOOSTER_KEYS {
            if let Some(pedal) = record.remove(current) {
                record.insert(earlier.into(), pedal);
            }
        }
        for name in record["order"].as_array_mut().unwrap() {
            if let Some((earlier, _)) = BOOSTER_KEYS
                .iter()
                .find(|(_, current)| name.as_str() == Some(*current))
            {
                *name = (*earlier).into();
            }
        }
    }

    #[test]
    fn timestamps_follow_the_gregorian_calendar() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(951_782_400 + 86_399), "2000-02-29T23:59:59Z");
        assert_eq!(timestamp(1_791_676_800), "2026-10-11T00:00:00Z");
    }

    #[test]
    fn zero_length_fades_keep_their_selected_curves_on_roundtrip() {
        let mut project = project();
        project.clips[0].fades = Fades {
            fade_in: crate::engine::Fade {
                ticks: 0,
                curve: crate::engine::FadeCurve::EqualPower,
            },
            fade_out: crate::engine::Fade {
                ticks: 0,
                curve: crate::engine::FadeCurve::Exponential,
            },
        };
        let document = Document::new(
            &project,
            "2020-01-02T03:04:05Z".into(),
            "2020-01-02T03:04:05Z".into(),
        );
        let saved = serde_json::to_vec(&document).unwrap();
        let reopened: Document = serde_json::from_slice(&saved).unwrap();
        assert_eq!(reopened.into_project().unwrap(), project);
    }

    #[test]
    fn saved_song_is_a_version_18_document_that_reopens_unchanged() {
        let root = std::env::temp_dir().join(format!("mixjam-song-format-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("song.mixjam");
        let project = project();
        save(&project, &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["formatVersion"], 18);
        assert_eq!(document["appVersion"], crate::APP_VERSION);
        assert_eq!(
            document["lanes"][5]["eq"],
            serde_json::json!({
                "on": true, "lowCut": 80.0, "lowHz": 100.0, "lowDb": 0.0, "midHz": 1000.0,
                "midDb": 0.0, "midQ": 1.0, "highHz": 8000.0, "highDb": -7.5, "highCut": 20000.0
            })
        );
        assert_eq!(document["lanes"][7]["eq"]["on"], false);
        assert_eq!(document["lanes"][7]["eq"]["midDb"], 3.0);
        // The second peak saves only what differs from a new track's.
        assert_eq!(document["lanes"][7]["eq"]["peak2Db"], -4.5);
        assert!(document["lanes"][7]["eq"].get("peak2Hz").is_none());
        assert_eq!(
            document["lanes"][5]["compressor"],
            serde_json::json!({
                "on": true, "threshold": -24.0, "ratio": 4.0, "attack": 10.0, "release": 100.0,
                "makeup": 3.0
            })
        );
        assert_eq!(document["lanes"][5]["width"], 150.0);
        assert_eq!(document["lanes"][9]["width"], 0.0);
        assert!(document["lanes"][4].get("compressor").is_none());
        assert!(document["lanes"][4].get("width").is_none());
        // A lane whose EQ is as a new track has it saves none.
        assert!(document["lanes"][4].get("eq").is_none());
        assert_eq!(
            document["lanes"][5]["ducker"],
            serde_json::json!({
                "on": true, "key": 1, "threshold": -30.0, "depth": 9.5, "attack": 2.0, "release": 120.0
            })
        );
        assert!(document["lanes"][4].get("ducker").is_none());
        assert_eq!(document["master"]["bypass"], true);
        assert_eq!(
            document["lanes"][63]["placements"][0]["fades"],
            serde_json::json!({
                "in": {"ticks": 64, "curve": "equal-power"},
                "out": {"ticks": 128, "curve": "exponential"}
            })
        );
        assert_eq!(document["lanes"][63]["placements"][0]["pitchCents"], -150);
        assert_eq!(
            document["lanes"][5]["automation"],
            serde_json::json!({
                "volume": [{"tick": 0, "value": -60.0}, {"tick": 512, "value": 0.0}],
                "sends": [[], [{"tick": 256, "value": 0.75}], [], []],
                "filter": [{"tick": 1024, "value": -0.5}]
            })
        );
        assert!(document["lanes"][4].get("automation").is_none());
        // A placement without fades or pitch saves neither.
        let plain = document["lanes"][0]["placements"][0].as_object().unwrap();
        assert!(!plain.contains_key("fades") && !plain.contains_key("pitchCents"));
        assert_eq!(
            document["lanes"][5]["sends"],
            serde_json::json!([0.25, 0.0, 0.5, 1.0])
        );
        assert_eq!(
            document["fxBuses"][0]["module"]["type"],
            "aetherform-reverb"
        );
        assert_eq!(document["fxBuses"][0]["module"]["spaceModel"], "hall");
        assert_eq!(document["fxBuses"][2]["module"]["divisionL"], "1/4.");
        assert_eq!(document["fxBuses"][2]["powered"], false);
        assert_eq!(
            document["fxBuses"][1],
            serde_json::json!({
                "module": {"type": "empty"}, "powered": true, "returnLevel": 1.0, "limiter": true
            })
        );
        assert_eq!(document["master"]["boosterCompressor"]["on"], true);
        assert_eq!(document["master"]["boosterCompressor"]["drive"], 368);
        assert_eq!(document["master"]["boosterEq"]["bands"][9], 0);
        assert_eq!(document["master"]["boosterWide"]["on"], false);
        assert_eq!(
            document["master"]["order"],
            serde_json::json!([
                "boosterWide",
                "boosterEq",
                "boosterCompressor",
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
                "fxBuses",
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
    fn versions_before_17_reject_a_lane_ducker() {
        let mut project = project();
        project.tracks.iter_mut().for_each(before_version_18);
        let mut document = serde_json::to_value(Document::new(
            &project,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap();
        document["formatVersion"] = 16.into();
        let reopened: Document = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(
            reopened.into_project().unwrap_err(),
            "Version 16 lanes cannot contain ducker."
        );
        document["lanes"][5]
            .as_object_mut()
            .unwrap()
            .remove("ducker");
        let reopened: Document = serde_json::from_value(document.clone()).unwrap();
        let mut expected = project;
        expected.tracks[5].ducker = Default::default();
        assert_eq!(reopened.into_project().unwrap(), expected);
        // The pedals' keys are new in version 17 too.
        document["master"]["limiter"]["key"] = 3.into();
        let reopened: Document = serde_json::from_value(document).unwrap();
        assert_eq!(
            reopened.into_project().unwrap_err(),
            "Version 16 songs cannot contain key or duckKey."
        );
    }

    #[test]
    fn version_17_ducker_thresholds_move_by_the_master_gain() {
        // A version 17 key held the master gain. From version 18 it leaves
        // it out, 6.02 dB hotter at a master gain of 0.5, so the threshold
        // rises as much and the lane ducks as it did.
        let mut project = project();
        project.tracks.iter_mut().for_each(before_version_18);
        assert_eq!(project.master_gain, 0.5);
        let mut document = serde_json::to_value(Document::new(
            &project,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap();
        let open =
            |document: &serde_json::Value| parse(&serde_json::to_vec(document).unwrap()).unwrap();
        // Version 18 keeps the saved threshold.
        assert_eq!(open(&document), project);
        document["formatVersion"] = 17.into();
        let opened = open(&document);
        let raised = opened.tracks[5].ducker.threshold - project.tracks[5].ducker.threshold;
        assert!((raised - 6.0206).abs() < 1e-3, "{raised} dB");
        // Nothing else changes, and lanes without a ducker keep the default.
        let mut expected = project.clone();
        expected.tracks[5].ducker.threshold = opened.tracks[5].ducker.threshold;
        assert_eq!(opened, expected);
        // A ducker that is off moves too. The range holds the threshold, and
        // a master gain of 0, which left the key silent, leaves it.
        document["lanes"][5]["ducker"]["on"] = false.into();
        for (master_gain, threshold, moved) in [
            (0.5, -3.0, 0.0),
            (2.0, -58.0, -60.0),
            (2.0, -18.0, -24.0206),
            (1.0, -18.0, -18.0),
            (0.0, -18.0, -18.0),
        ] {
            document["song"]["masterGain"] = master_gain.into();
            document["lanes"][5]["ducker"]["threshold"] = threshold.into();
            let ducker = open(&document).tracks[5].ducker;
            assert!(!ducker.on);
            assert!(
                (ducker.threshold - moved).abs() < 1e-3,
                "{master_gain} {threshold}: {}",
                ducker.threshold
            );
        }
        // A threshold outside its range stays, so the song is rejected, as
        // before the move existed, even where the move would bring it back.
        for (master_gain, threshold) in [(1.0, 5.0), (0.5, -61.0), (2.0, 3.0)] {
            document["song"]["masterGain"] = master_gain.into();
            document["lanes"][5]["ducker"]["threshold"] = threshold.into();
            let found = parse(&serde_json::to_vec(&document).unwrap()).unwrap_err();
            assert!(
                found.contains("ducker setting is out of range"),
                "{master_gain} {threshold}: {found}"
            );
        }
    }

    #[test]
    fn versions_before_18_reject_a_compressor_a_width_and_a_second_peak() {
        let project = project();
        let document = serde_json::to_value(Document::new(
            &project,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap();
        let mut expected = project.clone();
        expected.tracks.iter_mut().for_each(before_version_18);
        let mut earlier = serde_json::to_value(Document::new(
            &expected,
            "2020-01-02T03:04:05Z".into(),
            timestamp(0),
        ))
        .unwrap();
        earlier["formatVersion"] = 17.into();
        // Lane 5's ducker threshold moves by the master gain, as
        // `version_17_ducker_thresholds_move_by_the_master_gain` checks.
        let mut opened = expected.clone();
        opened.tracks[5].ducker.threshold =
            threshold_without_master_gain(opened.tracks[5].ducker.threshold, opened.master_gain);
        assert_eq!(
            parse(&serde_json::to_vec(&earlier).unwrap()).unwrap(),
            opened
        );
        for (lane, key, value, error) in [
            (
                5,
                "compressor",
                document["lanes"][5]["compressor"].clone(),
                "compressor or width",
            ),
            (5, "width", 150.into(), "compressor or width"),
            (4, "width", 100.into(), "compressor or width"),
            (
                7,
                "eq",
                document["lanes"][7]["eq"].clone(),
                "eq peak2Hz, peak2Db, or peak2Q",
            ),
        ] {
            let mut changed = earlier.clone();
            changed["lanes"][lane][key] = value;
            let found = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
            assert_eq!(
                found,
                format!("Version 17 lanes cannot contain {error}."),
                "{key}"
            );
        }
        // An explicit flat second peak is new in version 18 too.
        let mut flat = earlier;
        flat["lanes"][5]["eq"]["peak2Db"] = 0.into();
        assert_eq!(
            parse(&serde_json::to_vec(&flat).unwrap()).unwrap_err(),
            "Version 17 lanes cannot contain eq peak2Hz, peak2Db, or peak2Q."
        );
        let reopened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
        assert_eq!(reopened, project);
        for (pointer, value, error) in [
            ("/lanes/5/width", serde_json::json!(201), "width"),
            ("/lanes/5/width", serde_json::json!(-1), "width"),
            (
                "/lanes/5/compressor/ratio",
                serde_json::json!(0.5),
                "compressor setting",
            ),
            (
                "/lanes/5/compressor/knee",
                serde_json::json!(6),
                "unknown field",
            ),
            (
                "/lanes/5/eq/peak2Q",
                serde_json::json!(9),
                "channel EQ setting",
            ),
            (
                "/lanes/5/eq/peak2Hz",
                serde_json::json!(30),
                "channel EQ setting",
            ),
        ] {
            let mut changed = document.clone();
            match changed.pointer_mut(pointer) {
                Some(target) => *target = value,
                None => {
                    let (parent, key) = pointer.rsplit_once('/').unwrap();
                    changed.pointer_mut(parent).unwrap()[key] = value;
                }
            }
            let found = parse(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
            assert!(found.contains(error), "{pointer}: {found}");
        }
    }

    #[test]
    fn fx_settings_reject_duplicate_fields_before_adoption() {
        let document = Document::new(&project(), timestamp(0), timestamp(0));
        let text = serde_json::to_string(&document).unwrap();
        let duplicate = text.replacen("\"powered\":true", "\"powered\":true,\"powered\":false", 1);
        assert_ne!(duplicate, text);
        assert!(
            parse(duplicate.as_bytes())
                .unwrap_err()
                .contains("duplicate field")
        );
    }

    #[test]
    fn earlier_versions_reject_explicit_own_input_keys() {
        let mut expected = project();
        for track in &mut expected.tracks {
            track.ducker = Default::default();
            before_version_18(track);
        }
        let base =
            serde_json::to_value(Document::new(&expected, timestamp(0), timestamp(0))).unwrap();
        for effect in [false, true] {
            let mut document = base.clone();
            document["formatVersion"] = 16.into();
            if effect {
                let module = crate::fx::Module::EchoformDelay(Default::default());
                document["fxBuses"][0]["module"] = serde_json::to_value(module).unwrap();
                document["fxBuses"][0]["module"]["duckKey"] = 0.into();
            } else {
                document["master"]["limiter"]["key"] = 0.into();
            }
            let reopened: Document = serde_json::from_value(document).unwrap();
            assert_eq!(
                reopened.into_project().unwrap_err(),
                "Version 16 songs cannot contain key or duckKey."
            );
        }
    }

    #[test]
    fn pedal_and_fx_keys_save_only_when_they_name_a_track() {
        let mut project = project();
        let saved = |project: &Project| {
            serde_json::to_value(Document::new(
                project,
                "2020-01-02T03:04:05Z".into(),
                timestamp(0),
            ))
            .unwrap()
        };
        let document = saved(&project);
        assert!(document["master"]["busCompressor"].get("key").is_none());
        project.master.bus_compressor.key = 2;
        project.master.bus_compressor.on = true;
        let document = saved(&project);
        assert_eq!(document["master"]["busCompressor"]["key"], 2);
        let reopened: Document = serde_json::from_value(document).unwrap();
        assert_eq!(reopened.into_project().unwrap().master, project.master);
    }

    #[test]
    fn version_8_signed_lanes_migrate_and_save_as_the_current_version_without_losing_polarity() {
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
        assert_eq!(saved["formatVersion"], FORMAT_VERSION);
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
    fn version_9_songs_open_unmuted_with_their_outputs_and_save_as_the_current_version() {
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
        assert_eq!(saved["formatVersion"], FORMAT_VERSION);
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
            ("/master/boosterEq/bands/0", serde_json::Value::from(101)),
            ("/master/boosterCompressor/speed", 1001.into()),
            ("/master/boosterWide/spread", (-1).into()),
            ("/master/boosterWide/on", serde_json::Value::Null),
            ("/master/order/0", "boosterEq".into()),
            (
                "/master/order",
                serde_json::json!(["boosterEq", "boosterWide"]),
            ),
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
        unknown["master"]["boosterWide"]["width"] = 3.into();
        assert!(
            parse(&serde_json::to_vec(&unknown).unwrap())
                .unwrap_err()
                .contains("unknown field `width`")
        );
    }

    #[test]
    fn version_11_songs_open_with_their_booster_pedals_first_and_the_rest_off() {
        let mut expected = project();
        let mut document = earlier_document(&mut expected, 11);
        let [eq, compressor, wide] = BOOSTER_KEYS.map(|(earlier, _)| earlier);
        document["master"] = serde_json::json!({"order": [wide, compressor, eq]});
        document["master"][eq] =
            serde_json::json!({"on": false, "bands": [50, 50, 50, 50, 50, 50, 50, 50, 50, 0]});
        document["master"][compressor] =
            serde_json::json!({"on": true, "drive": 368, "speed": 500, "gain": 500});
        document["master"][wide] = serde_json::json!({"on": true, "spread": 700});
        let opened = parse(&serde_json::to_vec(&document).unwrap()).unwrap();
        let master = opened.master;
        assert_eq!(
            master.order[..3],
            [
                PedalKind::BoosterWide,
                PedalKind::BoosterCompressor,
                PedalKind::BoosterEq
            ]
        );
        assert_eq!(master.order[3..], PedalKind::ALL[3..]);
        assert!(master.booster_compressor.on && master.booster_wide.on && !master.booster_eq.on);
        assert_eq!(
            (master.booster_wide.spread, master.booster_eq.bands[9]),
            (700, 0)
        );
        assert!(
            PedalKind::ALL
                .iter()
                .filter(|kind| !kind.is_booster())
                .all(|kind| !master.is_on(*kind))
        );
        assert_eq!(master.trim, 0.0);
        assert_eq!(opened, Project { master, ..expected });

        // A version 11 song keeps the Booster record's strict shape and its
        // earlier keys.
        for (pointer, value) in [
            ("/master/order".to_string(), serde_json::json!([eq, wide])),
            ("/master/order/0".into(), compressor.into()),
            ("/master/order/0".into(), "boosterWide".into()),
            (format!("/master/{wide}/spread"), 1001.into()),
        ] {
            let mut changed = document.clone();
            *changed.pointer_mut(&pointer).unwrap() = value;
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
                serde_json::json!([0, 0, 0, 1.5]),
                "sends must be",
            ),
            (
                "/fxBuses/0/module/mix",
                serde_json::json!(1),
                "unknown field",
            ),
            (
                "/fxBuses/0/returnLevel",
                serde_json::json!(2),
                "return level",
            ),
            (
                "/lanes/63/placements/0/fades/in/ticks",
                serde_json::json!(512),
                "fades longer than the clip",
            ),
            (
                "/lanes/63/placements/0/fades/in/curve",
                serde_json::json!("s-curve"),
                "unknown variant",
            ),
            (
                "/lanes/63/placements/0/pitchCents",
                serde_json::json!(2401),
                "pitch must be",
            ),
            ("/formatVersion", serde_json::json!(13), "cannot contain"),
            ("/formatVersion", serde_json::json!(14), "cannot contain"),
            (
                "/lanes/5/eq/highCut",
                serde_json::json!(20_001),
                "channel EQ setting",
            ),
            (
                "/lanes/5/eq/midQ",
                serde_json::json!(0.2),
                "channel EQ setting",
            ),
            ("/lanes/5/eq/gain", serde_json::json!(1), "unknown field"),
            (
                "/lanes/5/automation/volume/1/tick",
                serde_json::json!(0),
                "increasing tick order",
            ),
            (
                "/lanes/5/automation/filter/0/value",
                serde_json::json!(1.5),
                "Filter automation values",
            ),
            (
                "/lanes/5/automation/pan",
                serde_json::json!([]),
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
    fn version_14_songs_open_with_every_eq_off_and_the_board_in_use() {
        let mut expected = project();
        expected.master.bypass = false;
        for track in &mut expected.tracks {
            track.eq = Default::default();
            track.ducker = Default::default();
            before_version_18(track);
        }
        let mut document =
            serde_json::to_value(Document::new(&expected, timestamp(0), timestamp(0))).unwrap();
        document["formatVersion"] = 14.into();
        with_earlier_booster_keys(&mut document);
        assert_eq!(
            parse(&serde_json::to_vec(&document).unwrap()).unwrap(),
            expected
        );
        let mut with_eq = document.clone();
        with_eq["lanes"][2]["eq"] = serde_json::to_value(crate::eq::ChannelEq::default()).unwrap();
        let error = parse(&serde_json::to_vec(&with_eq).unwrap()).unwrap_err();
        assert_eq!(error, "Version 14 lanes cannot contain eq.");
        let mut bypassed = document;
        bypassed["master"]["bypass"] = true.into();
        let error = parse(&serde_json::to_vec(&bypassed).unwrap()).unwrap_err();
        assert_eq!(error, "Version 14 songs cannot contain master bypass.");
    }

    #[test]
    fn version_12_to_15_boards_keep_their_earlier_booster_keys() {
        let mut expected = project();
        for track in &mut expected.tracks {
            track.ducker = Default::default();
            before_version_18(track);
        }
        let current =
            serde_json::to_value(Document::new(&expected, timestamp(0), timestamp(0))).unwrap();
        let mut earlier = current.clone();
        earlier["formatVersion"] = 15.into();
        with_earlier_booster_keys(&mut earlier);
        // The same board opens from either key set, each at its own version.
        assert_eq!(
            parse(&serde_json::to_vec(&earlier).unwrap()).unwrap(),
            expected
        );
        assert_eq!(
            parse(&serde_json::to_vec(&current).unwrap()).unwrap(),
            expected
        );
        let (eq, booster_eq) = BOOSTER_KEYS[0];
        for version in 12..=15 {
            let mut document = earlier.clone();
            document["formatVersion"] = version.into();
            document["master"].as_object_mut().unwrap().remove("bypass");
            let mut current_key = document.clone();
            let pedal = current_key["master"]
                .as_object_mut()
                .unwrap()
                .remove(eq)
                .unwrap();
            current_key["master"][booster_eq] = pedal;
            assert_eq!(
                parse(&serde_json::to_vec(&current_key).unwrap()).unwrap_err(),
                format!("Version {version} songs cannot contain master {booster_eq}.")
            );
            let mut current_order = document.clone();
            current_order["master"]["order"][0] = "boosterWide".into();
            assert_eq!(
                parse(&serde_json::to_vec(&current_order).unwrap()).unwrap_err(),
                format!("Version {version} songs cannot contain master boosterWide.")
            );
        }
        // Versions 16 and later accept only the new keys; a version after the
        // current one does not open.
        let mut earlier_key = current.clone();
        let pedal = earlier_key["master"]
            .as_object_mut()
            .unwrap()
            .remove(booster_eq)
            .unwrap();
        earlier_key["master"][eq] = pedal;
        let error = parse(&serde_json::to_vec(&earlier_key).unwrap()).unwrap_err();
        assert!(error.contains(&format!("unknown field `{eq}`")), "{error}");
        let mut earlier_order = current.clone();
        earlier_order["master"]["order"][0] = earlier["master"]["order"][0].clone();
        let error = parse(&serde_json::to_vec(&earlier_order).unwrap()).unwrap_err();
        assert!(error.contains("unknown variant"), "{error}");
        let mut later = current;
        later["formatVersion"] = (FORMAT_VERSION + 1).into();
        assert_eq!(
            parse(&serde_json::to_vec(&later).unwrap()).unwrap_err(),
            "This MixJam format version is not supported."
        );
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
