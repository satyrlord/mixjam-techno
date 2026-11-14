//! Views of a session for replies: the song, transport, samples, and library as the channel reports them.
use super::{
    BusView, Busy, ClipView, ConflictView, LibraryState, MissingView, ProviderView, SampleView,
    ScanView, SectionView, SongView, TrackView, Transport, UnloadedView,
};
use crate::application::{Product, Sample, Session, TICKS_PER_BAR};
use std::path::PathBuf;

fn bar_of(tick: u32) -> f64 {
    f64::from(tick) / f64::from(TICKS_PER_BAR) + 1.0
}

/// The tick of a position given as a bar counted from 1, or as a tick.
pub fn position_tick(bar: Option<f64>, tick: Option<u32>) -> Result<u32, String> {
    match (bar, tick) {
        (Some(_), Some(_)) => Err("Give a bar or a tick, not both.".into()),
        (None, Some(tick)) => Ok(tick),
        (Some(bar), None) => {
            let tick = ((bar - 1.0) * f64::from(TICKS_PER_BAR)).round();
            if bar.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&tick) {
                Ok(tick as u32)
            } else {
                Err("Bars count from 1.".into())
            }
        }
        (None, None) => Err("Give a bar or a tick.".into()),
    }
}

fn sample_library(session: &Session, id: u32) -> Option<Product> {
    Product::sample_key(id)
        .map(|(product, _)| product)
        .or_else(|| {
            session
                .project()
                .imported_samples
                .iter()
                .find(|sample| sample.id == id)
                .and_then(|sample| sample.origin)
        })
}

pub fn sample_view(session: &Session, sample: &Sample) -> SampleView {
    SampleView {
        id: sample.id,
        name: sample.name.clone(),
        group: sample.group_name.clone(),
        library: sample_library(session, sample.id).map(Product::label),
        bars: sample.bars,
        bpm: sample.source_bpm,
        path: sample.wav_path.clone(),
    }
}

pub fn song_view(session: &Session) -> SongView {
    let project = session.project();
    let library = session.library();
    let mut clip_counts = vec![0; project.tracks.len()];
    for clip in &project.clips {
        if let Some(count) = clip_counts.get_mut(clip.track) {
            *count += 1;
        }
    }
    // A soloed track silences the others, so it is listed even without clips.
    let tracks: Vec<TrackView> = project
        .tracks
        .iter()
        .zip(clip_counts)
        .enumerate()
        .filter(|(_, (track, clips))| *clips > 0 || track.solo)
        .map(|(index, (track, clips))| TrackView {
            track: index + 1,
            gain: track.gain,
            polarity_inverted: track.polarity_inverted,
            pan: track.pan,
            left: track.left_enabled,
            right: track.right_enabled,
            muted: track.muted,
            solo: track.solo,
            sends: track.sends,
            automation: track.automation.clone(),
            eq_on: track.eq.on,
            drive: track.drive,
            compressor_on: track.compressor.on,
            width: track.width,
            clips,
        })
        .collect();
    let mut groups: Vec<String> = library
        .samples
        .iter()
        .map(|sample| sample.group_name.clone())
        .collect();
    groups.sort();
    groups.dedup();
    SongView {
        title: project.title.clone(),
        file: session.file().map(PathBuf::from),
        unsaved: session.unsaved(),
        bpm: project.bpm,
        master_gain: project.master_gain,
        master_bypass: project.master.bypass,
        loop_bars: project.loop_region.map(|[start, end]| {
            [
                start / TICKS_PER_BAR + 1,
                end.div_ceil(TICKS_PER_BAR).max(start / TICKS_PER_BAR + 1),
            ]
        }),
        sections: project
            .sections
            .iter()
            .map(|section| SectionView {
                bar: bar_of(section.start_tick),
                tick: section.start_tick,
                name: section.name.clone(),
            })
            .collect(),
        end_tick: project.end_tick(),
        tracks,
        clips: project
            .clips
            .iter()
            .map(|clip| {
                let missing = session.missing_sample(clip.sample_id);
                ClipView {
                    id: clip.id,
                    sample: clip.sample_id,
                    name: missing.map_or_else(
                        || {
                            library
                                .sample_by_id(clip.sample_id)
                                .map(|sample| sample.name.clone())
                                .unwrap_or_default()
                        },
                        |missing| missing.name.clone(),
                    ),
                    missing: missing.is_some(),
                    track: clip.track + 1,
                    bar: bar_of(clip.start_tick),
                    start_tick: clip.start_tick,
                    length_ticks: clip.length_ticks,
                    offset_frames: clip.offset_frames,
                    gain: clip.gain,
                    fades: clip.fades,
                    pitch_cents: clip.pitch_cents,
                    reverse: clip.reverse,
                    rect: None,
                }
            })
            .collect(),
        fx_buses: project
            .fx
            .buses
            .iter()
            .zip(&project.return_automation)
            .enumerate()
            .map(|(index, (bus, lane))| BusView {
                bus: index + 1,
                effect: bus.module.label(),
                powered: bus.powered,
                return_level: bus.return_level,
                return_automation: lane.clone(),
            })
            .collect(),
        missing_samples: session
            .missing_samples()
            .iter()
            .map(|missing| MissingView {
                sample: missing.id,
                name: missing.name.clone(),
                reason: missing.reason.clone(),
            })
            .collect(),
        sample_conflicts: session
            .sample_conflicts()
            .iter()
            .map(|conflict| ConflictView {
                reference: conflict.reference.clone(),
                candidates: conflict.candidates.clone(),
                selected: conflict.selected,
                confirmed: conflict.confirmed,
            })
            .collect(),
        import_notices: project
            .import_notices
            .iter()
            .map(|notice| {
                format!(
                    "{} BPM changed to {} BPM.",
                    notice.source_bpm, notice.target_bpm
                )
            })
            .chain(project.source_notices.iter().cloned())
            .collect(),
        library: LibraryState {
            folders: library.wav_roots().to_vec(),
            unloaded: unloaded_views(session),
            samples: library.samples.len(),
            groups,
            providers: library
                .bindings()
                .iter()
                .map(|binding| ProviderView {
                    library: binding.product.label(),
                    root: binding.root.clone(),
                })
                .collect(),
        },
    }
}

pub fn transport(session: &Session) -> Transport {
    Transport {
        playing: session.wants_playback(),
        recording: session.is_recording(),
        cursor_tick: session.cursor_tick(),
        bar: bar_of(session.cursor_tick()),
    }
}

/// The library folders that the last scan could not load.
pub fn unloaded_views(session: &Session) -> Vec<UnloadedView> {
    session
        .library()
        .unloaded_roots()
        .iter()
        .map(|(folder, error)| UnloadedView {
            folder: folder.clone(),
            error: error.clone(),
        })
        .collect()
}

pub fn busy(session: &Session) -> Busy {
    Busy {
        scan: session.wave_scan().map(|scan| ScanView {
            folders: scan.roots,
            files_done: scan.files_done,
            files_total: scan.files_total,
        }),
        exporting: session.is_exporting(),
        unsaved_take: session.has_recorded_take(),
    }
}
