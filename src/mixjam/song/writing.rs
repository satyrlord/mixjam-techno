//! Saving a project as a document, with the size limit and the timestamps.

use super::{
    FORMAT_VERSION,
    migration::Saved,
    model::{
        Conflict, Document, Lane, Missing, Notice, Placement, Reference, Sample, SavedSection,
        Song, SongBus, SongBuses,
    },
};
use crate::{
    engine::{Fades, MissingReference, Project, TRACK_WIDTH},
    mixjam::{Header, MAX_SONG_BYTES, text},
};
use std::{fs, io::Read, path::Path};

impl Document {
    pub(super) fn new(project: &Project, created_at: String, modified_at: String) -> Self {
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
                automation: (!track.automation.is_empty())
                    .then(|| Saved::Known(track.automation.clone())),
                eq: (!track.eq.is_default()).then_some(Saved::Known(track.eq)),
                drive: (!track.drive.is_default()).then_some(track.drive),
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
                reverse: clip.reverse.then_some(true),
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
                sections: (!project.sections.is_empty()).then(|| {
                    project
                        .sections
                        .iter()
                        .map(|section| SavedSection {
                            start_tick: section.start_tick,
                            name: section.name.clone(),
                        })
                        .collect()
                }),
            },
            master: Some(Saved::Known(project.master)),
            fx_buses: Some(Saved::Known(SongBuses(
                project
                    .fx
                    .buses
                    .iter()
                    .zip(&project.return_automation)
                    .map(|(bus, lane)| SongBus {
                        bus: *bus,
                        return_automation: lane.clone(),
                    })
                    .collect(),
            ))),
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
}

pub(in crate::mixjam) fn save(project: &Project, path: &Path) -> Result<(), String> {
    save_with_limit(project, path, MAX_SONG_BYTES)
}

pub(super) fn save_with_limit(
    project: &Project,
    path: &Path,
    byte_limit: u64,
) -> Result<(), String> {
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

pub(super) fn existing_creation_time(path: &Path) -> Option<String> {
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
pub(super) fn timestamp(seconds: u64) -> String {
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
