//! The sample references that a song keeps: imported samples, placeholders
//! for audio that is unavailable, import notices, and conflict choices.
use super::{MAX_TICKS, SOURCE_BPM, TICKS_PER_BAR};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

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
pub struct ImportedSamples(pub(super) std::sync::Arc<Vec<ImportedSample>>);

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

/// A clip's trim offset after its source changes from `from` to `to`, each
/// a sample rate and a source tempo: the same point in the music, unless
/// `preserve_frames`. `None` when the offset leaves the frame range.
pub fn rescale_trim(
    offset: u32,
    preserve_frames: bool,
    (from_rate, from_bpm): (u32, f64),
    (to_rate, to_bpm): (u32, f64),
) -> Option<u32> {
    if offset == 0 || preserve_frames {
        return Some(offset);
    }
    let offset = (f64::from(offset) * f64::from(to_rate) / f64::from(from_rate)
        * (from_bpm / to_bpm))
        .round();
    (offset.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&offset)).then_some(offset as u32)
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
            sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
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
            sample_rate: sample
                .source_sample_rate
                .unwrap_or(legacy_format::NATIVE_SAMPLE_RATE),
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

    /// Whether the original keeps a clip's trim in frames when the sample's
    /// rate or tempo changes, instead of rescaling it.
    pub fn preserves_trim_frames(&self) -> bool {
        self.metadata
            .get("Trim rate policy")
            .is_some_and(|policy| policy == "preserve-frames")
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

    pub(super) fn validate(&self) -> Result<(), String> {
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
