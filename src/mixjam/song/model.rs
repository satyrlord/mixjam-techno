//! The saved document: the structs that map one-to-one onto the JSON keys.

use super::migration::{Saved, UniqueJson};
use crate::{
    engine::Fades,
    master::MasterChain,
    products::{LibraryBinding, Product},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Document {
    pub(super) format_version: u32,
    pub(super) app_version: String,
    pub(super) created_at: String,
    pub(super) modified_at: String,
    pub(super) song: Song,
    // Keep the raw settings until the version is checked. Deserializing the
    // current types first loses the presence of optional keys set to zero.
    // Required from version 11; earlier versions have no master pedalboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) master: Option<Saved<MasterChain>>,
    // Required from version 13; earlier versions have no FX buses. Raw
    // until the version is checked, as version 23 added each bus's
    // `returnAutomation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) fx_buses: Option<Saved<SongBuses>>,
    pub(super) lanes: Vec<Lane>,
    pub(super) samples: Vec<Sample>,
    pub(super) missing_samples: Vec<Missing>,
    pub(super) sample_conflicts: Vec<Conflict>,
    pub(super) import_notices: Vec<Notice>,
    pub(super) source_notices: Vec<String>,
    // Songs saved before this field existed, and songs without settings, omit it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) source_settings: BTreeMap<String, String>,
    pub(super) libraries: Vec<LibraryBinding>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Song {
    pub(super) title: String,
    pub(super) author: String,
    pub(super) bpm: f64,
    pub(super) master_gain: f32,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub(super) loop_region: Option<[u32; 2]>,
    // Version 19 added sections; songs without them omit the key.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "saved_sections"
    )]
    pub(super) sections: Option<Vec<SavedSection>>,
}

fn saved_sections<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<SavedSection>>, D::Error> {
    Vec::<SavedSection>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SavedSection {
    pub(super) start_tick: u32,
    pub(super) name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Lane {
    pub(super) gain: f32,
    #[serde(
        default,
        deserialize_with = "crate::engine::optional_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) polarity_inverted: Option<bool>,
    pub(super) pan: f32,
    pub(super) left_enabled: bool,
    pub(super) right_enabled: bool,
    #[serde(
        default,
        deserialize_with = "crate::engine::optional_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) muted: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::engine::optional_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) solo: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) sends: Option<[f32; crate::fx::BUSES]>,
    // Raw until the version is checked, as version 20 added `pan`, version
    // 21 `repeat`, version 23 `repeatGate`, and version 24 `pitch`, which an
    // earlier song must not hold even as an empty list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) automation: Option<Saved<crate::automation::Automation>>,
    // Raw until the version is checked, as the second peak's keys are
    // optional in the EQ record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) eq: Option<Saved<crate::eq::ChannelEq>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "saved_drive"
    )]
    pub(super) drive: Option<crate::drive::Drive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) compressor: Option<crate::compressor::Compressor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) ducker: Option<crate::ducker::Ducker>,
    pub(super) placements: Vec<Placement>,
}

// A missing drive is off. An explicit null is not a drive record and must
// not erase the presence of a version-22 setting in an earlier song.
fn saved_drive<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<crate::drive::Drive>, D::Error> {
    crate::drive::Drive::deserialize(deserializer).map(Some)
}

/// The FX buses as a song saves them: each bus's settings and, from version
/// 23, its return lane when it has points.
#[derive(Serialize)]
#[serde(transparent)]
pub(super) struct SongBuses(pub(super) Vec<SongBus>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SongBus {
    #[serde(flatten)]
    pub(super) bus: crate::fx::FxBus,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) return_automation: Vec<crate::automation::Point>,
}

// Serde also accepts positional arrays for structs. The song format names
// each point's fields, while legacy project JSON keeps its existing reader.
pub(super) fn point_arrays_have_objects(value: &serde_json::Value) -> bool {
    value
        .as_array()
        .is_none_or(|points| points.iter().all(serde_json::Value::is_object))
}

/// The FX buses of a saved song and their return lanes. Versions 13 to 22
/// reject a bus's `returnAutomation`, even as an empty list.
pub(super) fn saved_buses(
    buses: &Saved<SongBuses>,
    version: u32,
) -> Result<
    (
        crate::fx::FxBuses,
        [Vec<crate::automation::Point>; crate::fx::BUSES],
    ),
    String,
> {
    let invalid = |error: serde_json::Error| format!("The FX buses are invalid: {error}");
    let Saved::Raw(raw) = buses else {
        return Err("The FX buses are not a saved record.".into());
    };
    if version < 23 {
        let lanes = buses.json().as_array().is_some_and(|buses| {
            buses
                .iter()
                .any(|bus| bus.get("returnAutomation").is_some())
        });
        if lanes {
            return Err(format!(
                "Version {version} songs cannot contain FX return automation."
            ));
        }
        let fx = serde_json::from_str(raw.get()).map_err(invalid)?;
        return Ok((fx, Default::default()));
    }
    let UniqueJson(mut json) = serde_json::from_str(raw.get()).map_err(invalid)?;
    let mut returns: [Vec<crate::automation::Point>; crate::fx::BUSES] = Default::default();
    if let Some(buses) = json.as_array_mut() {
        for (bus, record) in buses.iter_mut().enumerate() {
            let lane = record
                .as_object_mut()
                .and_then(|record| record.remove("returnAutomation"));
            if let (Some(lane), Some(slot)) = (lane, returns.get_mut(bus)) {
                if !point_arrays_have_objects(&lane) {
                    return Err(format!(
                        "FX {} return automation points must be JSON objects.",
                        bus + 1
                    ));
                }
                *slot = serde_json::from_value(lane).map_err(|error| {
                    format!("The FX {} return automation is invalid: {error}", bus + 1)
                })?;
            }
        }
    }
    let fx = serde_json::from_value(json).map_err(invalid)?;
    Ok((fx, returns))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Placement {
    pub(super) id: u64,
    pub(super) sample_id: u32,
    pub(super) start_tick: u32,
    pub(super) duration_ticks: u32,
    pub(super) offset_frames: u32,
    pub(super) gain: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) fades: Option<Fades>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) pitch_cents: Option<i32>,
    // Version 23 added reverse; a placement that plays forward omits it. An
    // explicit null is not a switch.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::engine::optional_bool"
    )]
    pub(super) reverse: Option<bool>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Sample {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) group_name: String,
    pub(super) path: PathBuf,
    pub(super) source_bpm: f64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub(super) source_sample_rate: Option<u32>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub(super) origin: Option<Product>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Missing {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) source_bpm: f64,
    pub(super) sample_rate: u32,
    pub(super) nominal_ticks: u32,
    pub(super) length_inferred: bool,
    pub(super) reference: Reference,
    pub(super) reason: String,
    pub(super) metadata: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Reference {
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
pub(super) struct Conflict {
    pub(super) reference: String,
    pub(super) sample_ids: Vec<u32>,
    pub(super) candidates: Vec<PathBuf>,
    pub(super) selected: usize,
    pub(super) confirmed: bool,
    pub(super) source_spans: BTreeMap<u32, u32>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Notice {
    pub(super) source_bpm: f64,
    pub(super) target_bpm: f64,
}
