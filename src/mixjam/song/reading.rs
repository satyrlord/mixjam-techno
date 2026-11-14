//! Opening a saved document: version checks and the conversion into a project.

use super::{
    FORMAT_VERSION,
    migration::{Booster, PEAK2_KEYS, Saved, current_booster_keys, threshold_without_master_gain},
    model::{Document, Reference, point_arrays_have_objects, saved_buses},
};
use crate::{
    engine::{
        Clip, ImportNotice, ImportedSample, MissingReference, MissingSample,
        PROJECT_SCHEMA_VERSION, Project, SampleConflict, TRACK_COUNT, TRACK_WIDTH, Track,
    },
    master::MasterChain,
    mixjam::text,
};

pub(in crate::mixjam) fn parse(bytes: &[u8]) -> Result<Project, String> {
    serde_json::from_slice::<Document>(bytes)
        .map_err(|e| format!("Cannot read the MixJam song: {e}"))?
        .into_project()
}

impl Document {
    pub(super) fn into_project(self) -> Result<Project, String> {
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
        if self.format_version < 19 && self.song.sections.is_some() {
            return Err(format!(
                "Version {} songs cannot contain sections.",
                self.format_version
            ));
        }
        let invalid =
            |error: serde_json::Error| format!("The master pedalboard is invalid: {error}");
        let master = match (self.format_version, self.master) {
            (16.., Some(master)) => master.settings().map_err(invalid)?,
            (version @ 12.., Some(master)) => {
                serde_json::from_value(current_booster_keys(master.migration_json()?, version)?)
                    .map_err(invalid)?
            }
            (11, Some(master)) => serde_json::from_value::<Booster>(current_booster_keys(
                master.migration_json()?,
                11,
            )?)
            .map_err(invalid)?
            .into_board(),
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
        let (fx, return_automation) = match (self.format_version, self.fx_buses) {
            (version @ 13.., Some(buses)) => saved_buses(&buses, version)?,
            (version @ 13.., None) => {
                return Err(format!("Version {version} songs require fxBuses."));
            }
            (_, None) => Default::default(),
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
            let automation = match lane.automation {
                Some(automation) => {
                    let json = automation.json();
                    let Some(record) = json.as_object() else {
                        return Err("The automation must be a JSON object.".into());
                    };
                    if self.format_version < 20 && record.contains_key("pan") {
                        return Err(format!(
                            "Version {} lanes cannot contain pan automation.",
                            self.format_version
                        ));
                    }
                    if self.format_version < 21 && record.contains_key("repeat") {
                        return Err(format!(
                            "Version {} lanes cannot contain repeat automation.",
                            self.format_version
                        ));
                    }
                    if self.format_version < 23 && record.contains_key("repeatGate") {
                        return Err(format!(
                            "Version {} lanes cannot contain repeat gate automation.",
                            self.format_version
                        ));
                    }
                    if self.format_version < 24 && record.contains_key("pitch") {
                        return Err(format!(
                            "Version {} lanes cannot contain pitch automation.",
                            self.format_version
                        ));
                    }
                    let lanes_are_objects =
                        ["volume", "filter", "pan", "repeat", "repeatGate", "pitch"]
                            .iter()
                            .filter_map(|key| record.get(*key))
                            .all(point_arrays_have_objects);
                    let sends_are_objects = record
                        .get("sends")
                        .and_then(serde_json::Value::as_array)
                        .is_none_or(|sends| sends.iter().all(point_arrays_have_objects));
                    if !lanes_are_objects || !sends_are_objects {
                        return Err("The automation points must be JSON objects.".into());
                    }
                    automation
                        .settings()
                        .map_err(|error| format!("The automation is invalid: {error}"))?
                }
                None => Default::default(),
            };
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
            if self.format_version < 22 && lane.drive.is_some() {
                return Err(format!(
                    "Version {} lanes cannot contain drive.",
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
                automation,
                eq,
                drive: lane.drive.unwrap_or_default(),
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
                if self.format_version < 23 && placement.reverse.is_some() {
                    return Err(format!(
                        "Version {} placements cannot contain reverse.",
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
                    reverse: placement.reverse.unwrap_or(false),
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
            sections: self
                .song
                .sections
                .unwrap_or_default()
                .into_iter()
                .map(|section| crate::engine::Section {
                    start_tick: section.start_tick,
                    name: section.name,
                })
                .collect(),
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
            return_automation,
        };
        project.validate()?;
        Ok(project)
    }
}
