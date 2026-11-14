//! The project: the song's tracks, clips, samples, and pedalboards.
use super::{
    Clip, DEFAULT_TITLE, ImportNotice, ImportedSamples, MissingSample, PROJECT_SCHEMA_VERSION,
    SOURCE_BPM, SampleConflict, Section, TRACK_COUNT, Track,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

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
    /// Named parts of the song, in time order. Songs without sections leave it empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub imported_samples: ImportedSamples,
    #[serde(default)]
    pub missing_samples: Vec<MissingSample>,
    #[serde(default)]
    pub sample_conflicts: Vec<SampleConflict>,
    #[serde(default)]
    pub import_notices: Vec<ImportNotice>,
    #[serde(default)]
    pub source_notices: Vec<String>,
    /// Settings that the original product saved with an imported song, as text.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_settings: BTreeMap<String, String>,
    #[serde(default)]
    pub libraries: Vec<crate::products::LibraryBinding>,
    /// The master pedalboard. Every pedal is off unless the song switched it on.
    #[serde(default, deserialize_with = "deserialize_project_master")]
    pub master: crate::master::MasterChain,
    /// The four FX buses that the tracks' sends feed. All are empty unless
    /// the song filled them.
    #[serde(default)]
    pub fx: crate::fx::FxBuses,
    /// Each FX bus's return lane. A lane with points replaces its bus's
    /// return level for playback and export.
    #[serde(default, skip_serializing_if = "crate::automation::no_returns")]
    pub return_automation: [Vec<crate::automation::Point>; crate::fx::BUSES],
}

/// Earlier JSON projects have no board-format version. Accept their saved
/// Booster names here, without changing the strict versioned song reader.
fn deserialize_project_master<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<crate::master::MasterChain, D::Error> {
    use crate::master::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct Board {
        #[serde(default)]
        bypass: bool,
        trim: f32,
        order: [String; 13],
        #[serde(alias = "ejayEq")]
        booster_eq: BoosterEq,
        #[serde(alias = "ejayCompressor")]
        booster_compressor: BoosterCompressor,
        #[serde(alias = "ejayWide")]
        booster_wide: BoosterWide,
        soft_clip: SoftClip,
        tube: Tube,
        trim_eq: TrimEq,
        bus_compressor: BusCompressor,
        maximizer: Maximizer,
        lift_eq: LiftEq,
        tape: Tape,
        imaging: Imaging,
        multiband: Multiband,
        limiter: Limiter,
    }
    let board = Board::deserialize(deserializer)?;
    let mut order = PedalKind::ALL;
    for (pedal, name) in order.iter_mut().zip(board.order) {
        let name = match name.as_str() {
            "ejayEq" => "boosterEq",
            "ejayCompressor" => "boosterCompressor",
            "ejayWide" => "boosterWide",
            name => name,
        };
        *pedal = serde_json::from_value(serde_json::Value::String(name.into()))
            .map_err(serde::de::Error::custom)?;
    }
    Ok(MasterChain {
        bypass: board.bypass,
        trim: board.trim,
        order,
        booster_eq: board.booster_eq,
        booster_compressor: board.booster_compressor,
        booster_wide: board.booster_wide,
        soft_clip: board.soft_clip,
        tube: board.tube,
        trim_eq: board.trim_eq,
        bus_compressor: board.bus_compressor,
        maximizer: board.maximizer,
        lift_eq: board.lift_eq,
        tape: board.tape,
        imaging: board.imaging,
        multiband: board.multiband,
        limiter: board.limiter,
    })
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
            title: DEFAULT_TITLE.into(),
            author: String::new(),
            bpm: SOURCE_BPM,
            master_gain: 0.75,
            pitch_ratio: 1.0,
            tracks: vec![Track::default(); TRACK_COUNT],
            clips: Vec::new(),
            loop_region: None,
            sections: Vec::new(),
            imported_samples: ImportedSamples::default(),
            missing_samples: Vec::new(),
            sample_conflicts: Vec::new(),
            import_notices: Vec::new(),
            source_notices: Vec::new(),
            source_settings: BTreeMap::new(),
            libraries: Vec::new(),
            master: crate::master::MasterChain::default(),
            fx: crate::fx::FxBuses::default(),
            return_automation: Default::default(),
        }
    }
}

impl Project {
    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|track| track.solo)
    }

    /// Remove choices that no longer affect the arrangement before saving or relinking.
    pub fn prune_sample_conflicts(&mut self) {
        if self.sample_conflicts.is_empty() {
            return;
        }
        let used: HashSet<_> = self.clips.iter().map(|clip| clip.sample_id).collect();
        for conflict in &mut self.sample_conflicts {
            conflict.sample_ids.retain(|id| used.contains(id));
            conflict.source_spans.retain(|id, _| used.contains(id));
        }
        self.sample_conflicts
            .retain(|conflict| !conflict.sample_ids.is_empty());
    }

    /// The ID for a song import's next imported or missing sample. Song
    /// imports number both lists together from `IMPORTED_ID_START`.
    pub(crate) fn next_import_id(&self) -> Result<u32, String> {
        u32::try_from(self.imported_samples.len() + self.missing_samples.len())
            .ok()
            .and_then(|count| crate::products::IMPORTED_ID_START.checked_add(count))
            .ok_or_else(|| "Imported sample ID limit reached.".into())
    }

    pub fn missing_sample(&self, id: u32) -> Option<&MissingSample> {
        self.missing_samples.iter().find(|sample| sample.id == id)
    }

    /// The tracks, from 0, that the master pedals that run and the FX buses
    /// with an effect listen to, apart from the lanes' duckers, in that order
    /// and without repeats.
    pub fn side_keys(&self) -> Vec<usize> {
        let buses = self.fx.buses.iter().filter_map(|bus| {
            let key = match &bus.module {
                crate::fx::Module::Empty => 0,
                crate::fx::Module::EchoformDelay(delay) => delay.duck_key,
                crate::fx::Module::AetherformReverb(reverb) => reverb.duck_key,
            };
            usize::from(key).checked_sub(1)
        });
        let mut keys: Vec<usize> = Vec::new();
        for key in self.master.key_tracks().chain(buses) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
    }
}
