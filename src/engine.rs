use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const PROJECT_SCHEMA_VERSION: u32 = 4;
pub const TRACK_COUNT: usize = 64;
pub const SUPPORTED_BPMS: [f64; 7] = [120.0, 125.0, 130.0, 135.0, 140.0, 160.0, 180.0];
pub const TICKS_PER_BAR: u32 = 256;
pub const SOURCE_BPM: f64 = 140.0;
/// A new song's title until it is saved under a file name.
pub const DEFAULT_TITLE: &str = "Untitled";
pub const MAX_TICKS: u32 = 4096 * TICKS_PER_BAR;
/// A guard against malformed songs. It is above the 262,144 one-bar clips
/// that the 64-track timeline holds.
pub const MAX_CLIPS: usize = 1_000_000;
pub const MAX_IMPORTED_SAMPLES: usize = 200_000;
// Guards against a file that is not a project, as the song reader's limit does.
const MAX_PROJECT_BYTES: usize = 1024 * 1024 * 1024;
pub const MAX_MIXER_GAIN_DB: f32 = 12.0;

/// Convert a nonnegative amplitude multiplier to decibels. Zero is silence.
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    (20.0 * f64::from(amplitude).log10()) as f32
}

/// Convert decibels to a nonnegative amplitude multiplier. Negative infinity is silence.
pub fn db_to_amplitude(db: f32) -> f32 {
    10.0_f64.powf(f64::from(db) / 20.0) as f32
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Track {
    /// Nonnegative amplitude. Polarity is independent, including at silence.
    pub gain: f32,
    pub polarity_inverted: bool,
    pub pan: f32,
    pub left_enabled: bool,
    pub right_enabled: bool,
    pub muted: bool,
    pub solo: bool,
    /// The share of the track sent to each FX bus, from 0 to 1, after the
    /// fader and pan.
    pub sends: [f32; crate::fx::BUSES],
    /// Lanes that move the volume, sends, and filter over the song.
    pub automation: crate::automation::Automation,
    /// The channel EQ, between the clips and the automation.
    pub eq: crate::eq::ChannelEq,
}
impl Default for Track {
    fn default() -> Self {
        Self {
            gain: 1.0,
            polarity_inverted: false,
            pan: 0.0,
            left_enabled: true,
            right_enabled: true,
            muted: false,
            solo: false,
            sends: [0.0; crate::fx::BUSES],
            automation: Default::default(),
            eq: Default::default(),
        }
    }
}

impl Track {
    pub fn effective_gain(&self) -> f32 {
        if self.polarity_inverted {
            -self.gain
        } else {
            self.gain
        }
    }

    /// Whether Mute and Solo let this track play. While any track is soloed,
    /// only soloed tracks play; Mute wins over Solo, as in the original.
    pub fn audible(&self, any_solo: bool) -> bool {
        !self.muted && (!any_solo || self.solo)
    }

    /// Preserve the effective amplitude of an older signed gain, including -0.0.
    pub fn set_legacy_gain(&mut self, signed: f32) {
        self.gain = signed.abs();
        self.polarity_inverted = signed.is_sign_negative();
    }
}

impl<'de> Deserialize<'de> for Track {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct SavedTrack {
            gain: f32,
            #[serde(default, deserialize_with = "optional_bool")]
            polarity_inverted: Option<bool>,
            #[serde(default)]
            pan: f32,
            left_enabled: bool,
            right_enabled: bool,
            // Projects saved before Mute and Solo existed have neither.
            #[serde(default)]
            muted: bool,
            #[serde(default)]
            solo: bool,
            // Projects saved before the FX buses existed send nothing.
            #[serde(default)]
            sends: [f32; crate::fx::BUSES],
            #[serde(default)]
            automation: crate::automation::Automation,
            // Projects saved before the channel EQ existed have it off.
            #[serde(default)]
            eq: crate::eq::ChannelEq,
        }
        let saved = SavedTrack::deserialize(deserializer)?;
        let mut track = Self {
            gain: saved.gain,
            polarity_inverted: saved.polarity_inverted.unwrap_or(false),
            pan: saved.pan,
            left_enabled: saved.left_enabled,
            right_enabled: saved.right_enabled,
            muted: saved.muted,
            solo: saved.solo,
            sends: saved.sends,
            automation: saved.automation,
            eq: saved.eq,
        };
        if saved.polarity_inverted.is_none() {
            if !saved.gain.is_finite() || !(-2.0..=4.0).contains(&saved.gain) {
                return Err(serde::de::Error::custom(
                    "Legacy track gain must be from -2 to 4.",
                ));
            }
            track.set_legacy_gain(saved.gain);
        }
        Ok(track)
    }
}

/// A field that may be absent but, when present, must be a boolean, not null.
pub(crate) fn optional_bool<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
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
    /// The fade in from the clip's start and the fade out to the end of its
    /// sound: the clip's end, or earlier where the source audio ends.
    #[serde(default)]
    pub fades: Fades,
    /// A pitch change in cents that keeps the clip's timing.
    #[serde(default)]
    pub pitch_cents: i32,
}

/// The widest clip pitch change: two octaves either way.
pub const MAX_PITCH_CENTS: i32 = 2400;

/// The shape of a clip fade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FadeCurve {
    #[default]
    Linear,
    /// Constant power: two clips that cross with this curve keep their loudness.
    EqualPower,
    /// Even steps in decibels over 60 dB, like a natural decay.
    Exponential,
}

impl FadeCurve {
    pub const ALL: [Self; 3] = [Self::Linear, Self::EqualPower, Self::Exponential];

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::EqualPower => "Equal power",
            Self::Exponential => "Exponential",
        }
    }

    /// The gain at `progress` through a fade in, from 0 (silent) to 1 (full).
    /// A fade out reads the same curve backward.
    pub fn gain(self, progress: f32) -> f32 {
        let t = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EqualPower => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::Exponential => (1000.0_f32.powf(t) - 1.0) / 999.0,
        }
    }
}

/// One clip edge's fade. Zero ticks is no fade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fade {
    pub ticks: u32,
    pub curve: FadeCurve,
}

/// A clip's two fades. Together they are no longer than the clip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fades {
    #[serde(rename = "in")]
    pub fade_in: Fade,
    #[serde(rename = "out")]
    pub fade_out: Fade,
}

impl Fades {
    pub fn is_none(&self) -> bool {
        self.fade_in.ticks == 0 && self.fade_out.ticks == 0
    }
    /// Shorten the fades to fit a clip of `length_ticks`, the fade out first.
    pub fn fit(mut self, length_ticks: u32) -> Self {
        self.fade_in.ticks = self.fade_in.ticks.min(length_ticks);
        self.fade_out.ticks = self.fade_out.ticks.min(length_ticks - self.fade_in.ticks);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportedSample {
    pub id: u32,
    pub name: String,
    pub group_name: String,
    pub path: PathBuf,
    #[serde(default = "default_source_bpm")]
    pub source_bpm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_sample_rate: Option<u32>,
    #[serde(default)]
    pub origin: Option<crate::products::Product>,
}
fn default_source_bpm() -> f64 {
    SOURCE_BPM
}

/// A project's imported samples. A selected WAV folder can supply thousands of
/// them, so project copies share one list until an edit changes it: history
/// entries and render requests then cost the same for any palette size.
#[derive(Clone, Debug, Default)]
pub struct ImportedSamples(std::sync::Arc<Vec<ImportedSample>>);

impl std::ops::Deref for ImportedSamples {
    type Target = Vec<ImportedSample>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ImportedSamples {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.0)
    }
}
impl PartialEq for ImportedSamples {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}
impl PartialEq<Vec<ImportedSample>> for ImportedSamples {
    fn eq(&self, other: &Vec<ImportedSample>) -> bool {
        *self.0 == *other
    }
}
impl From<Vec<ImportedSample>> for ImportedSamples {
    fn from(samples: Vec<ImportedSample>) -> Self {
        Self(std::sync::Arc::new(samples))
    }
}
impl FromIterator<ImportedSample> for ImportedSamples {
    fn from_iter<I: IntoIterator<Item = ImportedSample>>(samples: I) -> Self {
        Vec::from_iter(samples).into()
    }
}
impl IntoIterator for ImportedSamples {
    type Item = ImportedSample;
    type IntoIter = std::vec::IntoIter<ImportedSample>;
    fn into_iter(self) -> Self::IntoIter {
        std::sync::Arc::unwrap_or_clone(self.0).into_iter()
    }
}
impl<'a> IntoIterator for &'a ImportedSamples {
    type Item = &'a ImportedSample;
    type IntoIter = std::slice::Iter<'a, ImportedSample>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
impl<'a> IntoIterator for &'a mut ImportedSamples {
    type Item = &'a mut ImportedSample;
    type IntoIter = std::slice::IterMut<'a, ImportedSample>;
    fn into_iter(self) -> Self::IntoIter {
        std::sync::Arc::make_mut(&mut self.0).iter_mut()
    }
}
impl Serialize for ImportedSamples {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for ImportedSamples {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::deserialize(deserializer).map(Self::from)
    }
}

/// A retained source reference whose audio is currently unavailable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MissingReference {
    Native {
        product: crate::products::Product,
        native_id: u32,
    },
    External {
        path: PathBuf,
        origin: Option<crate::products::Product>,
    },
    Generated {
        product: crate::products::Product,
        kind: String,
        data: serde_json::Value,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissingSample {
    pub id: u32,
    pub name: String,
    pub source_bpm: f64,
    pub sample_rate: u32,
    pub nominal_ticks: u32,
    pub length_inferred: bool,
    pub reference: MissingReference,
    pub reason: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl MissingSample {
    pub fn native(
        product: crate::products::Product,
        native_id: u32,
        name: String,
        source_bpm: f64,
        nominal_ticks: u32,
        length_inferred: bool,
        reason: String,
    ) -> Result<Self, String> {
        Ok(Self {
            id: product.global_sample_id(native_id)?,
            name,
            source_bpm,
            sample_rate: 44_100,
            nominal_ticks,
            length_inferred,
            reference: MissingReference::Native { product, native_id },
            reason,
            metadata: BTreeMap::new(),
        })
    }

    pub fn external(sample: &ImportedSample, nominal_ticks: u32, reason: String) -> Self {
        Self {
            id: sample.id,
            name: sample.name.clone(),
            source_bpm: sample.source_bpm,
            sample_rate: sample.source_sample_rate.unwrap_or(44_100),
            nominal_ticks,
            length_inferred: true,
            reference: MissingReference::External {
                path: sample.path.clone(),
                origin: sample.origin,
            },
            reason,
            metadata: BTreeMap::from([("group_name".into(), sample.group_name.clone())]),
        }
    }

    /// The sample group saved with this placeholder. An external placeholder
    /// saves it as `group_name`. A native one saves it as `Group` or `category`.
    pub fn group_name(&self) -> Option<&str> {
        ["group_name", "Group", "category"]
            .into_iter()
            .filter_map(|key| self.metadata.get(key))
            .map(String::as_str)
            .find(|name| !name.trim().is_empty())
    }

    pub fn resolved_source_bpm(&self, frames: u32, sample_rate: u32) -> Result<f64, String> {
        let bpm = if self
            .metadata
            .get("Playback timing policy")
            .is_some_and(|policy| policy == "mixjam-span")
        {
            f64::from(self.nominal_ticks) * 240.0 * f64::from(sample_rate)
                / (f64::from(TICKS_PER_BAR) * f64::from(frames))
        } else {
            self.source_bpm
        };
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err("The restored sample has no valid playback tempo.".into());
        }
        Ok(bpm)
    }

    fn validate(&self) -> Result<(), String> {
        let valid_text =
            |text: &str| !text.trim().is_empty() && text.len() <= 65_536 && !text.contains('\0');
        if !valid_text(&self.name)
            || !valid_text(&self.reason)
            || !self.source_bpm.is_finite()
            || self.source_bpm <= 0.0
            || !(1..=384_000).contains(&self.sample_rate)
            || !(1..=MAX_TICKS).contains(&self.nominal_ticks)
            || self.metadata.len() > 256
            || self.metadata.iter().any(|(key, value)| {
                !valid_text(key) || value.len() > 65_536 || value.contains('\0')
            })
        {
            return Err("The missing sample has invalid metadata or timing.".into());
        }
        match &self.reference {
            MissingReference::Native { product, native_id }
                if product.global_sample_id(*native_id)? == self.id => {}
            MissingReference::External { path, .. }
                if self.id >= crate::products::IMPORTED_ID_START
                    && !path.as_os_str().is_empty() => {}
            MissingReference::Generated { kind, data, .. }
                if self.id >= crate::products::IMPORTED_ID_START
                    && valid_text(kind)
                    && data.is_object() => {}
            _ => {
                return Err(
                    "The missing sample identity does not match its source reference.".into(),
                );
            }
        }
        Ok(())
    }
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
    if sample
        .source_sample_rate
        .is_some_and(|rate| !(1..=384_000).contains(&rate))
    {
        return Err("The imported sample has an invalid source sample rate.".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportNotice {
    pub source_bpm: f64,
    pub target_bpm: f64,
}

/// A source reference with several WAV choices, shared by all its placements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleConflict {
    pub reference: String,
    pub sample_ids: Vec<u32>,
    pub candidates: Vec<PathBuf>,
    #[serde(default)]
    pub selected: usize,
    #[serde(default)]
    pub confirmed: bool,
    /// MixJam source spans use shared ticks and determine playback tempo.
    #[serde(default)]
    pub source_spans: BTreeMap<u32, u32>,
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
            imported_samples: ImportedSamples::default(),
            missing_samples: Vec::new(),
            sample_conflicts: Vec::new(),
            import_notices: Vec::new(),
            source_notices: Vec::new(),
            source_settings: BTreeMap::new(),
            libraries: Vec::new(),
            master: crate::master::MasterChain::default(),
            fx: crate::fx::FxBuses::default(),
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
            return Err("Tempo must be 120, 125, 130, 135, 140, 160, or 180 BPM.".into());
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
        if self
            .source_settings
            .iter()
            .any(|(key, value)| key.is_empty() || key.contains('\0') || value.contains('\0'))
        {
            return Err("The project has invalid saved source settings.".into());
        }
        let mut library_products = HashSet::new();
        if self.libraries.iter().any(|binding| {
            binding.root.as_os_str().is_empty() || !library_products.insert(binding.product)
        }) {
            return Err("Each product needs one library binding with a nonempty root.".into());
        }
        if !valid_mixer_gain(self.master_gain) {
            return Err("Master amplitude must be from 0 to 4.".into());
        }
        self.master.validate()?;
        self.fx.validate()?;
        if self
            .tracks
            .iter()
            .any(|track| !valid_mixer_gain(track.gain))
        {
            return Err("Track amplitude must be from 0 to 4.".into());
        }
        if self
            .tracks
            .iter()
            .any(|track| !track.pan.is_finite() || !(-1.0..=1.0).contains(&track.pan))
        {
            return Err("Track pan must be from -1 to 1.".into());
        }
        if self.tracks.iter().any(|track| {
            track
                .sends
                .iter()
                .any(|send| !send.is_finite() || !(0.0..=1.0).contains(send))
        }) {
            return Err("Track sends must be from 0 to 1.".into());
        }
        for (index, track) in self.tracks.iter().enumerate() {
            track
                .automation
                .validate()
                .and_then(|()| track.eq.validate())
                .map_err(|error| format!("Track {}: {error}", index + 1))?;
        }
        if self.clips.len() > MAX_CLIPS {
            return Err("The project has too many clips.".into());
        }
        let mut ids = HashSet::new();
        let mut imported_ids = HashSet::new();
        if self.imported_samples.len() > MAX_IMPORTED_SAMPLES {
            return Err("The project has too many imported samples.".into());
        }
        for sample in &self.imported_samples {
            validate_import_metadata(sample)?;
            if !imported_ids.insert(sample.id) {
                return Err("Imported sample IDs must be unique.".into());
            }
        }
        if self.missing_samples.len() > 100_000 {
            return Err("The project has too many missing samples.".into());
        }
        for sample in &self.missing_samples {
            sample.validate()?;
            if !imported_ids.insert(sample.id) {
                return Err("Resolved and missing sample IDs must be unique.".into());
            }
        }
        if self.sample_conflicts.len() > 100_000 {
            return Err("The project has too many sample conflicts.".into());
        }
        let mut conflict_ids = HashSet::new();
        for conflict in &self.sample_conflicts {
            let mut paths = HashSet::new();
            let source_ids: HashSet<_> = conflict.sample_ids.iter().copied().collect();
            if conflict.reference.trim().is_empty()
                || conflict.reference.len() > 65_536
                || conflict.reference.contains('\0')
                || conflict.sample_ids.is_empty()
                || conflict.sample_ids.len() > 100_000
                || !(2..=100_000).contains(&conflict.candidates.len())
                || conflict.selected >= conflict.candidates.len()
                || conflict.candidates.iter().any(|path| {
                    path.as_os_str().is_empty()
                        || path.to_string_lossy().contains('\0')
                        || !paths.insert(path)
                })
                || conflict.sample_ids.iter().any(|id| {
                    !conflict_ids.insert(*id)
                        || (!imported_ids.contains(id)
                            && crate::products::Product::sample_key(*id).is_none())
                })
                || conflict
                    .source_spans
                    .iter()
                    .any(|(id, ticks)| !source_ids.contains(id) || !(1..=MAX_TICKS).contains(ticks))
            {
                return Err(
                    "The sample conflict has invalid references, choices, or timing.".into(),
                );
            }
        }
        for c in &self.clips {
            if c.sample_id >= crate::products::IMPORTED_ID_START
                && !imported_ids.contains(&c.sample_id)
            {
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
            if c.fades.fit(c.length_ticks) != c.fades {
                return Err(format!("Clip {} has fades longer than the clip.", c.id));
            }
            if !(-MAX_PITCH_CENTS..=MAX_PITCH_CENTS).contains(&c.pitch_cents) {
                return Err(format!(
                    "Clip {} pitch must be from -24 to 24 semitones.",
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

    fn parse_json(bytes: &[u8]) -> Result<Self, String> {
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

fn valid_mixer_gain(g: f32) -> bool {
    // Earlier tracks could reach exactly 4.0 (+12.041 dB). Retain that value
    // when reopening a song; ordinary mixer controls stop at +12 dB.
    g.is_finite() && (0.0..=4.0).contains(&g)
}

pub struct Editor {
    project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    generation: u64,
}
impl Editor {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            generation: 0,
        }
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn into_project(self) -> Project {
        self.project
    }
    /// A count that advances with every change to the project, including
    /// undo and redo. A rejected or no-op transaction leaves it unchanged.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn transact(&mut self, change: impl FnOnce(&mut Project)) -> Result<(), String> {
        self.transact_step(false, change)
    }
    /// Apply a change that continues the previous transaction's gesture, such
    /// as one more frame of a slider drag. Undo then returns to the state
    /// before the gesture instead of stepping through each frame.
    pub fn transact_continuing(&mut self, change: impl FnOnce(&mut Project)) -> Result<(), String> {
        self.transact_step(true, change)
    }
    fn transact_step(
        &mut self,
        continuing: bool,
        change: impl FnOnce(&mut Project),
    ) -> Result<(), String> {
        let previous = self.project.clone();
        change(&mut self.project);
        self.project.prune_sample_conflicts();
        if let Err(e) = self.project.validate() {
            self.project = previous;
            return Err(e);
        }
        if previous != self.project {
            if !continuing || self.undo.is_empty() {
                self.undo.push(previous);
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
            }
            self.redo.clear();
            self.generation += 1;
        }
        Ok(())
    }
    /// Record current library bindings. They follow the selected providers
    /// and are not an edit, so history does not change.
    pub fn set_libraries(&mut self, libraries: Vec<crate::products::LibraryBinding>) {
        self.project.libraries = libraries;
    }
    /// Name the song after the file it is saved to. The name is not an edit,
    /// so undo and redo keep it.
    pub fn set_title(&mut self, title: String) {
        for project in self.undo.iter_mut().chain(&mut self.redo) {
            project.title.clone_from(&title);
        }
        self.project.title = title;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) -> bool {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            self.generation += 1;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            self.generation += 1;
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
                fades: Default::default(),
                pitch_cents: 0,
            })
        })?;
        Ok(id)
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
        let by_id: std::collections::HashMap<_, _> =
            self.project.clips.iter().map(|c| (c.id, c)).collect();
        for id in ids {
            let mut c = (*by_id.get(id).ok_or("A selected clip does not exist.")?).clone();
            c.track = usize::try_from(c.track as i64 + track_delta as i64)
                .map_err(|_| "The selection is outside the timeline.")?;
            c.start_tick = (c.start_tick as i64)
                .checked_add(tick_delta)
                .and_then(|tick| u32::try_from(tick).ok())
                .ok_or("The selection is outside the timeline.")?;
            if copy {
                next_id = next_id.checked_add(1).ok_or("Clip ID limit reached.")?;
                c.id = next_id;
            }
            additions.push(c);
        }
        // Keep ordering and history intact when a drag ends at its starting
        // position. Resolve every selected ID above before accepting the move.
        if !copy && track_delta == 0 && tick_delta == 0 {
            return Ok(ids.to_vec());
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
pub(crate) fn ensure_space(
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
    fn missing_sample_group_reads_the_key_of_each_importer() {
        let mut missing = MissingSample::native(
            crate::products::Product::D2,
            7,
            "Lead".into(),
            140.0,
            256,
            false,
            "The sample is unavailable.".into(),
        )
        .unwrap();
        assert_eq!(missing.group_name(), None);
        for (key, group) in [
            ("category", "Bass"),
            ("Group", "Drum"),
            ("group_name", "Loop"),
        ] {
            missing.metadata.insert(key.into(), group.into());
            assert_eq!(missing.group_name(), Some(group));
        }
        missing.metadata.insert("group_name".into(), " ".into());
        assert_eq!(missing.group_name(), Some("Drum"));
    }

    #[test]
    fn sample_conflict_choices_persist_validate_and_prune_after_clip_deletion() {
        let source = ImportedSample {
            id: 1_000_000,
            name: "Lead".into(),
            group_name: "MixJam".into(),
            path: "A/lead.wav".into(),
            source_bpm: 120.0,
            source_sample_rate: Some(44_100),
            origin: None,
        };
        let mut project = Project {
            imported_samples: vec![source].into(),
            sample_conflicts: vec![SampleConflict {
                reference: "Old/lead.wav".into(),
                sample_ids: vec![1_000_000],
                candidates: vec!["A/lead.wav".into(), "B/lead.wav".into()],
                selected: 0,
                confirmed: false,
                source_spans: BTreeMap::from([(1_000_000, 256)]),
            }],
            ..Project::default()
        };
        project.validate().unwrap();
        let bytes = serde_json::to_vec(&project).unwrap();
        let reopened: Project = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reopened, project);
        let mut old = serde_json::to_value(&project).unwrap();
        old.as_object_mut().unwrap().remove("sample_conflicts");
        assert!(
            serde_json::from_value::<Project>(old)
                .unwrap()
                .sample_conflicts
                .is_empty()
        );
        for case in 0..6 {
            let mut invalid = project.clone();
            let conflict = &mut invalid.sample_conflicts[0];
            match case {
                0 => conflict.selected = 2,
                1 => conflict.sample_ids.push(1_000_000),
                2 => conflict.candidates[1] = conflict.candidates[0].clone(),
                3 => conflict.sample_ids[0] = 1_000_099,
                4 => {
                    conflict.source_spans.insert(1_000_001, 256);
                }
                _ => {
                    conflict.source_spans.insert(1_000_000, 0);
                }
            }
            assert!(invalid.validate().is_err(), "case {case}");
        }
        // Deleting the last placement is valid while its import metadata remains.
        project.validate().unwrap();
        project.prune_sample_conflicts();
        assert!(project.sample_conflicts.is_empty());
    }

    #[test]
    fn recovered_mixjam_source_tempos_survive_json_without_float_drift() {
        for tempo in [
            124.999_999_999_999_99,
            151.964_162_646_450_75,
            113.562_231_759_656_65,
        ] {
            let source = ImportedSample {
                id: 1_000_000,
                name: "Recovered source".into(),
                group_name: "MixJam".into(),
                path: "Genre/source.wav".into(),
                source_bpm: tempo,
                source_sample_rate: Some(44100),
                origin: None,
            };
            let mut project = Project::default();
            project.missing_samples.push(MissingSample::external(
                &source,
                256,
                "Unavailable".into(),
            ));
            let mut ready = source;
            ready.id += 1;
            project.imported_samples.push(ready);
            project.validate().unwrap();
            let bytes = serde_json::to_vec(&project).unwrap();
            let reopened: Project = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(reopened, project);
            assert_eq!(serde_json::to_vec(&reopened).unwrap(), bytes);
        }
    }

    #[test]
    fn mixer_decibels_use_amplitude_and_keep_legacy_headroom() {
        assert_eq!(amplitude_to_db(0.0), f32::NEG_INFINITY);
        assert_eq!(db_to_amplitude(f32::NEG_INFINITY), 0.0);
        assert_eq!(amplitude_to_db(1.0), 0.0);
        assert_eq!(db_to_amplitude(0.0), 1.0);
        assert!((db_to_amplitude(-6.0) - 0.501_187_2).abs() < 1e-6);
        assert!((amplitude_to_db(0.5) + 6.020_6).abs() < 1e-5);
        assert_eq!(db_to_amplitude(MAX_MIXER_GAIN_DB), 3.981_071_7);
        for db in [-90.0, -60.0, -24.0, -12.0, -3.0, 0.0, 6.0, 12.0] {
            assert!((amplitude_to_db(db_to_amplitude(db)) - db).abs() < 1e-5);
        }
        let mut project = Project::default();
        project.tracks[0].gain = 4.0;
        project.tracks[0].polarity_inverted = true;
        project.master_gain = db_to_amplitude(MAX_MIXER_GAIN_DB);
        project.validate().unwrap();
        assert!(amplitude_to_db(project.tracks[0].gain) > MAX_MIXER_GAIN_DB);
        project.tracks[0].gain = -0.5;
        assert!(project.validate().is_err());
        project.tracks[0].gain = 4.01;
        assert!(project.validate().is_err());
        project.tracks[0].gain = 0.0;
        project.master_gain = 4.01;
        assert!(project.validate().is_err());
    }

    #[test]
    fn signed_legacy_json_migrates_polarity_in_all_earlier_schemas() {
        for (schema, slots) in [(0, 16), (1, 32), (2, 64), (3, 64)] {
            let mut value = serde_json::to_value(Project::default()).unwrap();
            if schema == 0 {
                value.as_object_mut().unwrap().remove("schema_version");
            } else {
                value["schema_version"] = schema.into();
            }
            value["tracks"].as_array_mut().unwrap().truncate(slots);
            for track in value["tracks"].as_array_mut().unwrap() {
                track.as_object_mut().unwrap().remove("polarity_inverted");
            }
            value["tracks"][0]["gain"] = (-0.7533).into();
            value["tracks"][1]["gain"] = (-0.0_f32).into();
            value["tracks"][2]["gain"] = 4.0.into();
            let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
            assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
            assert_eq!(migrated.tracks[0].gain, 0.7533);
            assert!(migrated.tracks[0].polarity_inverted);
            assert_eq!(migrated.tracks[0].effective_gain(), -0.7533);
            assert_eq!(migrated.tracks[1].gain, 0.0);
            assert!(migrated.tracks[1].polarity_inverted);
            assert!(migrated.tracks[1].effective_gain().is_sign_negative());
            assert_eq!(migrated.tracks[2].gain, 4.0);
            assert!(!migrated.tracks[2].polarity_inverted);
            assert_eq!(
                Project::parse_json(&serde_json::to_vec(&migrated).unwrap()).unwrap(),
                migrated
            );
            value["tracks"][0]["gain"] = (-2.01).into();
            assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
        }
    }

    #[test]
    fn earlier_json_master_keys_keep_the_board_settings_and_order() {
        let mut project = Project::default();
        project.master.booster_eq.on = true;
        project.master.booster_eq.bands[4] = 71;
        project.master.booster_compressor.drive = 731;
        project.master.booster_wide.spread = 217;
        project.master.move_pedal(2, 0);
        let current = serde_json::to_string(&project).unwrap();
        let earlier = current
            .replace("boosterEq", "ejayEq")
            .replace("boosterCompressor", "ejayCompressor")
            .replace("boosterWide", "ejayWide");
        let opened = Project::parse_json(earlier.as_bytes()).unwrap();
        assert_eq!(opened, project);
        assert_eq!(
            Project::parse_json(&serde_json::to_vec(&opened).unwrap()).unwrap(),
            project
        );
        let duplicate = earlier.replace("\"ejayWide\":", "\"boosterWide\":{},\"ejayWide\":");
        assert!(Project::parse_json(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn paired_native_labels_and_signed_track_levels_survive_json() {
        let mut project = Project::default();
        project.tracks[31].set_legacy_gain(-2.0 / 65536.0);
        project.tracks[0].gain = 2.352_630_6;
        project.imported_samples.push(ImportedSample {
            id: 1_000_000,
            name: format!("{} {}", "ă".repeat(200), "Î".repeat(200)),
            group_name: "Wave".into(),
            path: "source.wav".into(),
            source_bpm: 2000.0,
            source_sample_rate: None,
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
    fn last_of_sixty_four_tracks_supports_edit_undo_and_redo() {
        let mut editor = Editor::new(Project::default());
        assert_eq!(editor.project.tracks.len(), 64);
        editor.project.validate().unwrap();
        let id = editor.place(1, 63, 17, 31).unwrap();
        let placed = editor.project.clone();
        editor.move_clips(&[id], -1, 31, false).unwrap();
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
                polarity_inverted: false,
                left_enabled: false,
                right_enabled: true,
                muted: true,
                solo: true,
                sends: [0.0; crate::fx::BUSES],
                automation: Default::default(),
                eq: Default::default(),
            };
            project.clips.push(Clip {
                id: 37,
                sample_id: 13_000,
                track: source_track_count - 1,
                start_tick: 17,
                length_ticks: 31,
                offset_frames: 7,
                gain: 0.5,
                fades: Default::default(),
                pitch_cents: 0,
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
                fades: Default::default(),
                pitch_cents: 0,
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
            (122.5, 120.0),
            (122.501, 125.0),
            (127.5, 125.0),
            (130.0, 130.0),
            (132.501, 135.0),
            (137.5, 135.0),
            (137.501, 140.0),
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
            source_sample_rate: None,
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
        let path = std::env::temp_dir().join(format!(
            "mixjam-oversized-project-{}.json",
            std::process::id()
        ));
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_PROJECT_BYTES as u64 + 1).unwrap();
        drop(file);
        let result = Project::load(&path);
        fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap_err(), "The project file is too large.");
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
            fades: Default::default(),
            pitch_cents: 0,
        });
        original.import_notices.push(ImportNotice {
            source_bpm: 146.0,
            target_bpm: 140.0,
        });
        let mut value = serde_json::to_value(&original).unwrap();
        let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
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
    fn schema_two_migration_preserves_the_arrangement_and_adds_empty_missing_sources() {
        let mut value = serde_json::to_value(Project::default()).unwrap();
        value["schema_version"] = 2.into();
        value.as_object_mut().unwrap().remove("missing_samples");
        let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(migrated, Project::default());
    }

    #[test]
    fn placeholder_validation_rejects_conflicting_identities_and_invalid_metadata() {
        let imported = ImportedSample {
            id: 1_000_000,
            name: "Lead".into(),
            group_name: "Wave".into(),
            path: PathBuf::from("missing.wav"),
            source_bpm: 140.0,
            source_sample_rate: None,
            origin: None,
        };
        let missing = MissingSample::external(&imported, 256, "File is missing.".into());
        let mut project = Project {
            missing_samples: vec![missing.clone()],
            ..Project::default()
        };
        project.clips.push(Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
        });
        project.validate().unwrap();
        assert_eq!(
            Project::parse_json(&serde_json::to_vec(&project).unwrap()).unwrap(),
            project
        );
        project.imported_samples.push(imported);
        assert!(project.validate().is_err());
        project.imported_samples.clear();
        project.missing_samples.push(missing.clone());
        assert!(project.validate().is_err());
        project.missing_samples.pop();
        project.missing_samples[0].sample_rate = 0;
        assert!(project.validate().is_err());
        project.missing_samples[0] = missing.clone();
        project.missing_samples[0].nominal_ticks = 0;
        assert!(project.validate().is_err());
        project.missing_samples[0] = missing;
        project.missing_samples[0].reference = MissingReference::Native {
            product: crate::products::Product::T2,
            native_id: 7,
        };
        assert!(project.validate().is_err());
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
            source_sample_rate: None,
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
            fades: Default::default(),
            pitch_cents: 0,
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
                source_sample_rate: None,
                origin: Some(Product::T1),
            }]
            .into(),
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
        assert!(e.move_clips(&[a], 0, 128, false).is_err());
        assert_eq!(e.project, before);
        e.move_clips(&[a], 1, 128, false).unwrap();
        assert!(e.undo());
        assert_eq!(e.project, before);
        assert!(e.redo());
        // A move appends the clip again, so find it by identity.
        let moved = e.project.clips.iter().find(|c| c.id == a).unwrap();
        assert_eq!(moved.track, 1);
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
    fn save_replaces_the_project_beside_an_existing_temporary_file() {
        let directory =
            std::env::temp_dir().join(format!("mixjam-techno-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("mix.json");
        let temp = directory.join(format!(".mix.json.{}.tmp", std::process::id()));
        fs::write(&temp, b"unrelated existing file").unwrap();
        let p = Project::default();
        p.save(&path).unwrap();
        assert_eq!(fs::read(&temp).unwrap(), b"unrelated existing file");
        assert_eq!(Project::load(&path).unwrap(), p);
        fs::remove_file(&temp).unwrap();
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }
    #[test]
    fn history_entries_share_the_import_list_until_an_edit_changes_it() {
        let sample = |id| ImportedSample {
            id,
            name: "Palette sound".into(),
            group_name: "Wave".into(),
            path: PathBuf::from("palette.wav"),
            source_bpm: 140.0,
            source_sample_rate: None,
            origin: None,
        };
        let mut e = Editor::new(Project {
            imported_samples: vec![sample(1_000_000)].into(),
            ..Project::default()
        });
        e.place(1, 0, 0, 256).unwrap();
        e.transact(|p| p.master_gain = 1.0).unwrap();
        // Project copies hold one allocation until an edit changes the imports.
        assert!(e.undo.iter().all(|snapshot| {
            std::sync::Arc::ptr_eq(&snapshot.imported_samples.0, &e.project.imported_samples.0)
        }));
        e.transact(|p| p.imported_samples.push(sample(1_000_001)))
            .unwrap();
        let before = e.undo.last().unwrap();
        assert!(!std::sync::Arc::ptr_eq(
            &before.imported_samples.0,
            &e.project.imported_samples.0
        ));
        assert_eq!(before.imported_samples.len(), 1);
        assert_eq!(e.project.imported_samples.len(), 2);
    }
    #[test]
    fn zero_move_preserves_clip_order_history_and_redo_and_checks_ids() {
        let mut editor = Editor::new(Project::default());
        let first = editor.place(1, 0, 0, 256).unwrap();
        let second = editor.place(2, 0, 256, 256).unwrap();
        editor
            .transact(|project| project.master_gain = 1.0)
            .unwrap();
        assert!(editor.undo());
        let before = editor.project.clone();
        let generation = editor.generation();
        let undo_count = editor.undo.len();
        let redo = editor.redo.clone();
        assert_eq!(editor.move_clips(&[first], 0, 0, false).unwrap(), [first]);
        assert_eq!(
            editor.move_clips(&[second, first], 0, 0, false).unwrap(),
            [second, first]
        );
        assert!(editor.move_clips(&[first, u64::MAX], 0, 0, false).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.generation(), generation);
        assert_eq!(editor.undo.len(), undo_count);
        assert_eq!(editor.redo, redo);
        assert!(editor.redo());
        assert_eq!(editor.project.master_gain, 1.0);
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
