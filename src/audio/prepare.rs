//! Preparing a project's mix: decoding the sources that its clips play
//! and resolving each clip to a voice.
use super::{
    Mix, Pcm,
    frames::track_levels,
    resample,
    track::{TrackChain, Voice},
};
use crate::{
    engine::{Project, SOURCE_BPM},
    fx::ReturnLanes,
    library::Library,
};
use std::{collections::HashMap, sync::Arc};

impl Mix {
    /// `samples` holds each source at each pitch that a clip plays it at, as
    /// `load_sources` checked it. The caller has validated `project`.
    pub(super) fn new(
        project: &Project,
        samples: &HashMap<SourceKey, Arc<Pcm>>,
        source_tempos: &HashMap<u32, f64>,
        quality: crate::playback_quality::PlaybackQuality,
        render_quality: crate::fx::RenderQuality,
    ) -> Result<Self, String> {
        let frames_per_tick = project.frames_per_tick(Self::SAMPLE_RATE);
        let frames = (project.end_tick() as f64 * frames_per_tick).ceil() as u64;
        let mut sources = VoiceSources {
            project,
            source_tempos,
            playback_tempos: HashMap::new(),
            varispeeds: std::array::from_fn(|_| None),
            high_quality: quality.samples == crate::playback_quality::SampleQuality::High,
            frames_per_tick,
            frames,
        };
        let missing = missing_ids(project);
        let any_solo = project.any_solo();
        let side_keys = project.side_keys();
        let side = !side_keys.is_empty();
        let mut tracks = TrackChains::new(
            project,
            any_solo,
            &side_keys,
            frames_per_tick,
            quality,
            render_quality,
        );
        for clip in &project.clips {
            if missing.contains(&clip.sample_id) {
                continue;
            }
            let source = samples
                .get(&(clip.sample_id, clip.pitch_cents))
                .ok_or_else(|| format!("Sample {} is missing.", clip.sample_id))?;
            let track = &project.tracks[clip.track];
            if track.sounds(any_solo) {
                tracks.add(track, sources.voice(clip, track, source)?);
            }
        }
        let drives = tracks.assign_drives();
        let TrackChains {
            voices,
            chains,
            keys,
            ..
        } = tracks;
        let fed = std::array::from_fn(|bus| {
            voices.iter().any(|voice| voice.sends[bus] > 0.0)
                || chains.iter().any(|chain| chain.feeds(bus))
        });
        Ok(Self {
            quality,
            render_quality,
            frames,
            voices,
            chains,
            key_tracks: keys.iter().map(|chain| chain.track).collect(),
            side,
            keys,
            levels: track_levels(project),
            automated_pan: std::array::from_fn(|track| {
                !project.tracks[track].automation.pan.is_empty()
            }),
            master_gain: project.master_gain,
            bpm: project.bpm,
            fed,
            sends: fed.contains(&true),
            drives,
            returns: ReturnLanes::new(&project.return_automation, frames_per_tick),
        })
    }
}

// Supplies the tempo and Pitch lane shared by a track's voices.
struct VoiceSources<'a> {
    project: &'a Project,
    source_tempos: &'a HashMap<u32, f64>,
    playback_tempos: HashMap<u32, f64>,
    varispeeds: [Option<Option<Arc<crate::automation::Varispeed>>>; crate::engine::TRACK_COUNT],
    high_quality: bool,
    frames_per_tick: f64,
    frames: u64,
}

impl VoiceSources<'_> {
    fn voice(
        &mut self,
        clip: &crate::engine::Clip,
        track: &crate::engine::Track,
        source: &Arc<Pcm>,
    ) -> Result<Voice, String> {
        let start = (clip.start_tick as f64 * self.frames_per_tick).round() as u64;
        let end = (((clip.start_tick + clip.length_ticks) as f64 * self.frames_per_tick).round()
            as u64)
            .min(self.frames);
        let step = self.step(source, clip.sample_id)?;
        let frames_of = |ticks: u32| (f64::from(ticks) * self.frames_per_tick).round() as u64;
        let source_left = (source.frames() as f64 - f64::from(clip.offset_frames)).max(0.0) / step;
        let varispeed = self.varispeeds[clip.track]
            .get_or_insert_with(|| {
                crate::automation::Varispeed::new(&track.automation.pitch, self.frames_per_tick)
                    .map(Arc::new)
            })
            .clone();
        // Under a Pitch lane the source runs out where the lane's clock
        // has run its length, so the fade out ends there.
        let source_end = match &varispeed {
            Some(clock) => clock.reach(start, source_left, end),
            None => start.saturating_add(source_left.ceil() as u64).min(end),
        };
        let band_limited = self.high_quality && (step != 1.0 || varispeed.is_some());
        if band_limited {
            check_read_speed(clip, track, step)?;
            // The kernel's table is ready before playback reads it.
            resample::prepare();
        }
        // Channel enables depend only on the track: resolve them once per clip.
        Ok(Voice {
            track: clip.track,
            source: Arc::clone(source),
            start,
            end,
            offset_frames: clip.offset_frames,
            step,
            channel_gains: [
                track.left_enabled.then_some(clip.gain),
                track.right_enabled.then_some(clip.gain),
            ],
            sends: track.sends,
            fade_in: frames_of(clip.fades.fade_in.ticks),
            fade_out: frames_of(clip.fades.fade_out.ticks),
            curves: [clip.fades.fade_in.curve, clip.fades.fade_out.curve],
            // A reversed clip's sound ends with the clip: a source shorter
            // than the clip leaves silence at its start instead.
            sound_end: if clip.reverse {
                end
            } else {
                end.min(source_end)
            },
            reverse: clip.reverse,
            varispeed,
            band_limited,
        })
    }

    fn step(&mut self, source: &Pcm, sample_id: u32) -> Result<f64, String> {
        let source_bpm = self
            .source_tempos
            .get(&sample_id)
            .copied()
            .unwrap_or(SOURCE_BPM);
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("A sample has an invalid source tempo.".into());
        }
        let playback_bpm = *self
            .playback_tempos
            .entry(sample_id)
            .or_insert_with(|| self.project.sample_playback_bpm(sample_id, source_bpm));
        let step =
            source.sample_rate as f64 / Mix::SAMPLE_RATE as f64 * self.project.bpm / playback_bpm;
        if !step.is_finite() || step <= 0.0 {
            return Err("The source tempo cannot be represented for sample playback.".into());
        }
        Ok(step)
    }
}

fn check_read_speed(
    clip: &crate::engine::Clip,
    track: &crate::engine::Track,
    step: f64,
) -> Result<(), String> {
    let maximum_cents = track
        .automation
        .pitch
        .iter()
        .map(|point| point.value)
        .fold(0.0_f32, f32::max);
    let maximum_speed = step * (f64::from(maximum_cents) / 1200.0).exp2();
    if !resample::supports_speed(maximum_speed) {
        return Err(format!(
            "Sample {} cannot be read at this speed with High sample quality.",
            clip.sample_id
        ));
    }
    Ok(())
}

struct TrackChains {
    voices: Vec<Voice>,
    chains: Vec<TrackChain>,
    keys: Vec<TrackChain>,
    chain_index: [Option<usize>; crate::engine::TRACK_COUNT],
    frames_per_tick: f64,
    quality: crate::playback_quality::PlaybackQuality,
    render_quality: crate::fx::RenderQuality,
}

impl TrackChains {
    fn new(
        project: &Project,
        any_solo: bool,
        side_keys: &[usize],
        frames_per_tick: f64,
        quality: crate::playback_quality::PlaybackQuality,
        render_quality: crate::fx::RenderQuality,
    ) -> Self {
        let mut keys: Vec<TrackChain> = Vec::new();
        // A ducker works when its own track sounds; its key may be silent.
        let ducker_keys = project
            .tracks
            .iter()
            .filter(|track| track.ducker.on && track.sounds(any_solo))
            .map(|track| track.ducker.key_track());
        for key in ducker_keys.chain(side_keys.iter().copied()) {
            if !keys.iter().any(|chain| chain.track == key) {
                keys.push(TrackChain::new(
                    key,
                    &project.tracks[key],
                    frames_per_tick,
                    quality,
                    render_quality,
                ));
            }
        }
        Self {
            voices: Vec::new(),
            chains: Vec::new(),
            keys,
            chain_index: [None; crate::engine::TRACK_COUNT],
            frames_per_tick,
            quality,
            render_quality,
        }
    }

    fn add(&mut self, track: &crate::engine::Track, voice: Voice) {
        let track_index = voice.track;
        let keyed = self
            .keys
            .iter()
            .position(|chain| chain.track == track_index);
        if let Some(slot) = keyed {
            self.keys[slot].voices.push(voice.clone());
        }
        if !needs_chain(track, keyed.is_some()) {
            self.voices.push(voice);
            return;
        }
        let index = *self.chain_index[track_index].get_or_insert_with(|| {
            let mut chain = TrackChain::new(
                track_index,
                track,
                self.frames_per_tick,
                self.quality,
                self.render_quality,
            );
            if track.ducker.on {
                let key = track.ducker.key_track();
                let slot = self.keys.iter().position(|chain| chain.track == key);
                chain.ducker = slot.map(|slot| {
                    (
                        crate::ducker::Run::new(&track.ducker, Mix::SAMPLE_RATE as f32),
                        slot,
                    )
                });
            }
            self.chains.push(chain);
            self.chains.len() - 1
        });
        self.chains[index].voices.push(voice);
    }

    fn assign_drives(&mut self) -> usize {
        // Key chains and audible chains each keep their own history.
        let mut count = 0;
        for (_, slot) in self
            .keys
            .iter_mut()
            .chain(&mut self.chains)
            .filter_map(|chain| chain.drive.as_mut())
        {
            *slot = count;
            count += 1;
        }
        if count > 0 {
            crate::master::dsp::prepare_tables();
        }
        count
    }
}

fn needs_chain(track: &crate::engine::Track, is_key: bool) -> bool {
    // With the compressor off, its Compression lane does not affect the track.
    is_key
        || track.eq.on
        || track.drive.on
        || track.compressor.on
        || track.widens()
        || track.ducker.on
        || !track.automation.is_empty_except_compression()
}

/// Render a whole project in memory. Use `prepare_project` for a song of any length.
pub fn render_project(project: &Project, library: &Library) -> Result<Pcm, String> {
    prepare_project(project, library)?.to_pcm(&project.master, &project.fx)
}

/// Decode a project's sources and prepare its mix.
pub fn prepare_project(project: &Project, library: &Library) -> Result<Mix, String> {
    prepare_project_reusing_inner(
        project,
        library,
        &mut HashMap::new(),
        crate::playback_quality::PlaybackQuality::FULL,
        crate::fx::RenderQuality::Offline,
    )
}

/// Prepare playback without changing the song or any export constructor.
pub fn prepare_project_with_quality(
    project: &Project,
    library: &Library,
    quality: crate::playback_quality::PlaybackQuality,
) -> Result<Mix, String> {
    prepare_project_reusing_with_quality(project, library, &mut HashMap::new(), quality)
}

/// A decoded source at one clip pitch in cents. Pitch 0 is the source as it is.
pub(crate) type SourceKey = (u32, i32);

/// Fill `decoded` with exactly the sources and pitches that the project's
/// clips play. Entries already there are reused. Each source that is not
/// there is read and checked once, and each pitched copy is made from the
/// plain source. Both steps share out the sources among threads.
pub(super) fn load_sources(
    project: &Project,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
    read: impl Fn(u32) -> Result<Pcm, String> + Sync,
) -> Result<(), String> {
    let missing = missing_ids(project);
    let used: std::collections::BTreeSet<SourceKey> = project
        .clips
        .iter()
        .map(|clip| (clip.sample_id, clip.pitch_cents))
        .filter(|(id, _)| !missing.contains(id))
        .collect();
    decoded.retain(|key, _| used.contains(key));
    let new: Vec<SourceKey> = used
        .into_iter()
        .filter(|key| !decoded.contains_key(key))
        .collect();
    let mut unread: Vec<u32> = new
        .iter()
        .map(|&(id, _)| id)
        .filter(|id| !decoded.contains_key(&(*id, 0)))
        .collect();
    unread.dedup();
    let sources = in_parallel(&unread, |&id| {
        let source = read(id)?;
        source.validate()?;
        Ok(Arc::new(source))
    });
    let mut plains = HashMap::new();
    let mut error = None;
    for (id, source) in unread.into_iter().zip(sources) {
        match source {
            Ok(source) => {
                plains.insert(id, source);
            }
            Err(message) => {
                error.get_or_insert(message);
            }
        }
    }
    // Pitch the longest sources first, so that the last thread to finish
    // does not start a long one late.
    let mut pitched: Vec<(SourceKey, Arc<Pcm>)> = new
        .iter()
        .filter(|&&(_, cents)| cents != 0)
        .filter_map(|&(id, cents)| {
            let plain = plains.get(&id).or_else(|| decoded.get(&(id, 0)))?;
            Some(((id, cents), Arc::clone(plain)))
        })
        .collect();
    pitched.sort_by_key(|(_, plain)| std::cmp::Reverse(plain.samples.len()));
    let copies = in_parallel(&pitched, |((_, cents), plain)| {
        crate::sample_edit::repitch(plain, *cents).map(Arc::new)
    });
    for ((key, _), copy) in pitched.into_iter().zip(copies) {
        match copy {
            Ok(copy) => {
                decoded.insert(key, copy);
            }
            Err(message) => {
                error.get_or_insert(message);
            }
        }
    }
    for (id, plain) in plains {
        if new.binary_search(&(id, 0)).is_ok() {
            decoded.insert((id, 0), plain);
        }
    }
    error.map_or(Ok(()), Err)
}

/// The ids of the project's missing samples. Their clips do not play.
fn missing_ids(project: &Project) -> std::collections::HashSet<u32> {
    project
        .missing_samples
        .iter()
        .map(|sample| sample.id)
        .collect()
}

/// `work` applied to each item, in the items' order. The items are shared
/// out among all but one of the processor's cores, so that playback keeps
/// one while a mix is prepared.
pub(super) fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |count| count.get().saturating_sub(1))
        .clamp(1, items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<R>>> =
        items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = work(item);
                    *results[index].lock().unwrap() = Some(result);
                }
            });
        }
    });
    results
        .into_iter()
        .map(|result| {
            result
                .into_inner()
                .unwrap()
                .expect("every item has a result")
        })
        .collect()
}

/// Prepare a mix with the sources that an earlier call for the same library
/// decoded. `decoded` then holds exactly the sources of this project, so its
/// size follows the song. The caller clears it when the library changes.
#[cfg(test)]
pub(crate) fn prepare_project_reusing(
    project: &Project,
    library: &Library,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
) -> Result<Mix, String> {
    prepare_project_reusing_inner(
        project,
        library,
        decoded,
        crate::playback_quality::PlaybackQuality::FULL,
        crate::fx::RenderQuality::Offline,
    )
}

pub(crate) fn prepare_project_reusing_with_quality(
    project: &Project,
    library: &Library,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
    quality: crate::playback_quality::PlaybackQuality,
) -> Result<Mix, String> {
    prepare_project_reusing_inner(
        project,
        library,
        decoded,
        quality,
        crate::fx::RenderQuality::Live,
    )
}

fn prepare_project_reusing_inner(
    project: &Project,
    library: &Library,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
    quality: crate::playback_quality::PlaybackQuality,
    render_quality: crate::fx::RenderQuality,
) -> Result<Mix, String> {
    project.validate()?;
    load_sources(project, decoded, |id| library.read_pcm(id))?;
    let source_tempos = project
        .clips
        .iter()
        .map(|c| {
            (
                c.sample_id,
                library
                    .sample_by_id(c.sample_id)
                    .map_or(SOURCE_BPM, |s| s.source_bpm),
            )
        })
        .collect();
    Mix::new(project, decoded, &source_tempos, quality, render_quality)
}

#[cfg(test)]
pub(crate) fn mix_with_samples(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
) -> Result<Mix, String> {
    mix_with_samples_inner(
        project,
        samples,
        crate::playback_quality::PlaybackQuality::FULL,
        crate::fx::RenderQuality::Offline,
    )
}

#[cfg(test)]
pub(super) fn mix_with_samples_quality(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
    quality: crate::playback_quality::PlaybackQuality,
) -> Result<Mix, String> {
    mix_with_samples_inner(project, samples, quality, crate::fx::RenderQuality::Live)
}

#[cfg(test)]
fn mix_with_samples_inner(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
    quality: crate::playback_quality::PlaybackQuality,
    render_quality: crate::fx::RenderQuality,
) -> Result<Mix, String> {
    let source_tempos = project
        .imported_samples
        .iter()
        .map(|s| (s.id, s.source_bpm))
        .collect();
    let mut decoded = HashMap::new();
    load_sources(project, &mut decoded, |id| {
        samples
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("Sample {id} is missing."))
    })?;
    project.validate()?;
    Mix::new(project, &decoded, &source_tempos, quality, render_quality)
}

#[cfg(test)]
pub(crate) fn render_with_samples(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
) -> Result<Pcm, String> {
    mix_with_samples(project, samples)?.to_pcm(&project.master, &project.fx)
}
