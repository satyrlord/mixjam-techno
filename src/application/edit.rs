//! Typed edit commands and how a session applies them, with undo and redo.
use super::gesture::Gesture;
use super::{
    Fades, MasterChain, Product, Project, Section, Session, TICKS_PER_BAR, TRACK_COUNT, Track,
    TrackMix, check_pitch, duration_ticks,
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub enum EditCommand {
    SetTempo(f64),
    SetLoop(Option<[u32; 2]>),
    /// Replace the song's sections. They do not change the sound.
    SetSections(Vec<Section>),
    SetMixer {
        tracks: Vec<Track>,
        master_gain: f32,
    },
    /// Replace one track's settings. A view that edits one track sends this
    /// instead of copying all of them each frame.
    SetTrack {
        index: usize,
        track: Box<Track>,
    },
    /// Replace the settings of one track that a mixer row edits: level,
    /// polarity, pan, the switches, and the sends. Its processing and
    /// automation stay. Undo and live playback treat it as a `SetTrack`.
    SetTrackMix {
        index: usize,
        mix: TrackMix,
    },
    /// Fades that do not fit the new length are shortened, the fade out first.
    /// A trim of the clip's left edge changes its start, trim, and length.
    SetClip {
        id: u64,
        start_tick: u32,
        gain: f32,
        offset_frames: u32,
        length_ticks: u32,
        fades: Fades,
        pitch_cents: i32,
        reverse: bool,
    },
    /// The gain, fades, pitch, and reverse of several clips, as one step.
    /// Their places, trims, and lengths stay. Fades that do not fit a clip
    /// are shortened, the fade out first. Give each clip ID once.
    SetClipSounds(Vec<ClipSound>),
    SetSwitch {
        track: usize,
        switch: TrackSwitch,
        on: bool,
    },
    DeleteClips(Vec<u64>),
    /// Replace the master pedalboard settings. Playback takes them as it plays.
    SetMaster(MasterChain),
    /// Replace the FX buses' settings. Playback takes them as it plays.
    SetEffects(crate::fx::FxBuses),
    /// Replace one automation lane of a track.
    SetAutomation {
        track: usize,
        lane: crate::automation::Lane,
        points: Vec<crate::automation::Point>,
    },
    /// Replace the return lane of one FX bus. A lane with points replaces
    /// the bus's return level.
    SetReturnAutomation {
        bus: usize,
        points: Vec<crate::automation::Point>,
    },
}

/// What `EditCommand::SetClipSounds` sets on one clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipSound {
    pub id: u64,
    pub gain: f32,
    pub fades: Fades,
    pub pitch_cents: i32,
    pub reverse: bool,
}

/// The on/off controls that the arrangement lane and the mixer strip share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackSwitch {
    Left,
    Right,
    Mute,
    Solo,
}

impl TrackSwitch {
    pub const ALL: [Self; 4] = [Self::Mute, Self::Solo, Self::Left, Self::Right];

    pub fn letter(self) -> &'static str {
        match self {
            Self::Left => "L",
            Self::Right => "R",
            Self::Mute => "M",
            Self::Solo => "S",
        }
    }

    /// The control's accessible name after its track name, such as "left output".
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left output",
            Self::Right => "right output",
            Self::Mute => "mute",
            Self::Solo => "solo",
        }
    }

    pub fn get(self, track: &Track) -> bool {
        match self {
            Self::Left => track.left_enabled,
            Self::Right => track.right_enabled,
            Self::Mute => track.muted,
            Self::Solo => track.solo,
        }
    }

    pub fn get_mut(self, track: &mut Track) -> &mut bool {
        match self {
            Self::Left => &mut track.left_enabled,
            Self::Right => &mut track.right_enabled,
            Self::Mute => &mut track.muted,
            Self::Solo => &mut track.solo,
        }
    }
}

/// Whether `new` differs from `old` only in level, pan, and polarity, which
/// playback takes live without a new render.
fn levels_differ_only(old: &Track, new: &Track) -> bool {
    // Name every field, so that a new one is compared too.
    let Track {
        gain: _,
        polarity_inverted: _,
        pan: _,
        left_enabled,
        right_enabled,
        muted,
        solo,
        sends,
        automation,
        eq,
        drive,
        compressor,
        width,
        mono_below_hz,
        filter_resonance,
        filter_slope,
        ducker,
    } = old;
    let same_routing = (*left_enabled, *right_enabled, *muted, *solo, *sends)
        == (
            new.left_enabled,
            new.right_enabled,
            new.muted,
            new.solo,
            new.sends,
        );
    let same_processing = (
        *eq,
        *drive,
        *compressor,
        *ducker,
        *width,
        *mono_below_hz,
        *filter_resonance,
        *filter_slope,
    ) == (
        new.eq,
        new.drive,
        new.compressor,
        new.ducker,
        new.width,
        new.mono_below_hz,
        new.filter_resonance,
        new.filter_slope,
    );
    same_routing && same_processing && automation == &new.automation
}

/// A part of the song that playback takes live, without a new render.
#[derive(Clone, Copy, PartialEq)]
enum LivePart {
    /// The master pedalboard and the FX buses.
    MasterAndFx,
    Sections,
    /// The master gain, and each track's level, pan, and polarity.
    Levels,
}

/// Whether `a` and `b` are the same song apart from `part`.
fn same_apart_from(a: &Project, b: &Project, part: LivePart) -> bool {
    // Name every field, so that a new one is compared too.
    let Project {
        schema_version,
        title,
        author,
        bpm,
        master_gain,
        pitch_ratio,
        tracks,
        clips,
        loop_region,
        sections,
        imported_samples,
        missing_samples,
        sample_conflicts,
        import_notices,
        source_notices,
        source_settings,
        libraries,
        master,
        fx,
        return_automation,
    } = a;
    let tracks_same = if part == LivePart::Levels {
        tracks.len() == b.tracks.len()
            && tracks
                .iter()
                .zip(&b.tracks)
                .all(|(a, b)| levels_differ_only(a, b))
    } else {
        *tracks == b.tracks
    };
    *schema_version == b.schema_version
        && *bpm == b.bpm
        && *pitch_ratio == b.pitch_ratio
        && *loop_region == b.loop_region
        && (part == LivePart::Levels || *master_gain == b.master_gain)
        && (part == LivePart::MasterAndFx || (*master == b.master && *fx == b.fx))
        && (part == LivePart::Sections || *sections == b.sections)
        && *title == b.title
        && *author == b.author
        && *return_automation == b.return_automation
        && *libraries == b.libraries
        && *source_settings == b.source_settings
        && *import_notices == b.import_notices
        && *source_notices == b.source_notices
        && *sample_conflicts == b.sample_conflicts
        && *missing_samples == b.missing_samples
        && *imported_samples == b.imported_samples
        && tracks_same
        && *clips == b.clips
}

/// The one part that a history step from `before` to `after` changed, when
/// playback can take it live. The pedals and the buses must keep their keys.
fn live_step(before: &Project, after: &Project) -> Option<LivePart> {
    if (before.master != after.master || before.fx != after.fx)
        && before.side_keys() == after.side_keys()
        && same_apart_from(before, after, LivePart::MasterAndFx)
    {
        Some(LivePart::MasterAndFx)
    } else if before.sections != after.sections
        && same_apart_from(before, after, LivePart::Sections)
    {
        Some(LivePart::Sections)
    } else if same_apart_from(before, after, LivePart::Levels) {
        Some(LivePart::Levels)
    } else {
        None
    }
}

/// Resolve the clip indices in the order of the requested edits.
fn clip_sound_indices(project: &Project, sounds: &[ClipSound]) -> Result<Vec<usize>, String> {
    let mut requests = HashMap::with_capacity(sounds.len());
    for (index, sound) in sounds.iter().enumerate() {
        if requests.insert(sound.id, index).is_some() {
            return Err("A selected clip appears more than once.".into());
        }
        check_pitch(sound.pitch_cents, "Clip pitch")?;
    }
    let mut indices = vec![None; sounds.len()];
    for (index, clip) in project.clips.iter().enumerate() {
        if let Some(request) = requests.remove(&clip.id) {
            indices[request] = Some(index);
            if requests.is_empty() {
                break;
            }
        }
    }
    indices
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "A selected clip does not exist.".into())
}

impl Session {
    /// Cut the ringing tail of one FX bus during playback. This changes no
    /// setting and is not an undo step.
    pub fn clear_fx_tail(&self, bus: usize) {
        self.master.clear_tail(bus);
    }
    /// Apply a completed action as its own undo step, outside continuous controls.
    pub fn edit_discrete(&mut self, command: EditCommand) -> Result<(), String> {
        self.end_automation_gesture();
        self.gesture = None;
        let result = self.edit(command);
        self.gesture = None;
        result
    }
    pub fn edit(&mut self, command: EditCommand) -> Result<(), String> {
        // A mix edit is a track edit that keeps the rest of the track.
        let command = match command {
            EditCommand::SetTrackMix { index, mix } => {
                let mut track = self
                    .project()
                    .tracks
                    .get(index)
                    .filter(|_| index < TRACK_COUNT)
                    .ok_or("The track is outside the project.")?
                    .clone();
                track.set_mix(mix);
                EditCommand::SetTrack {
                    index,
                    track: Box::new(track),
                }
            }
            command => command,
        };
        let mut sound_indices = Vec::new();
        let mut clip_index = None;
        // Validate identifiers before a transaction. A stale UI command must fail visibly.
        match &command {
            EditCommand::SetClip {
                id,
                start_tick,
                offset_frames,
                length_ticks,
                pitch_cents,
                ..
            } => {
                check_pitch(*pitch_cents, "Clip pitch")?;
                let index = self
                    .project()
                    .clips
                    .iter()
                    .position(|clip| clip.id == *id)
                    .ok_or("The clip does not exist.")?;
                clip_index = Some(index);
                let clip = &self.project().clips[index];
                let max_offset = if let Some(sample) = self.missing_sample(clip.sample_id) {
                    (f64::from(sample.nominal_ticks) * f64::from(sample.sample_rate) * 240.0
                        / sample.source_bpm
                        / f64::from(TICKS_PER_BAR))
                    .ceil() as u32
                } else {
                    self.library
                        .sample_by_id(clip.sample_id)
                        .ok_or("The sample is unavailable.")?
                        .frame_count
                };
                if *offset_frames > max_offset.saturating_sub(1) {
                    return Err("The trim start must be within the source audio.".into());
                }
                // A wider clip must not cover its neighbor. Other clip edits
                // stay available for imported arrangements that already overlap.
                let end = u64::from(*start_tick) + u64::from(*length_ticks);
                if *start_tick < clip.start_tick
                    || end > u64::from(clip.start_tick) + u64::from(clip.length_ticks)
                {
                    crate::engine::ensure_space(
                        self.project(),
                        Some(*id),
                        clip.track,
                        *start_tick,
                        *length_ticks,
                    )?;
                }
            }
            EditCommand::SetSwitch { track, .. }
            | EditCommand::SetAutomation { track, .. }
            | EditCommand::SetTrack { index: track, .. }
                if *track >= TRACK_COUNT.min(self.project().tracks.len()) =>
            {
                return Err("The track is outside the project.".into());
            }
            EditCommand::SetAutomation { lane, points, .. } => {
                if let crate::automation::Lane::Send(bus) = lane
                    && *bus >= crate::fx::BUSES
                {
                    return Err("The FX bus does not exist.".into());
                }
                crate::automation::validate_points(*lane, points)?;
            }
            EditCommand::SetReturnAutomation { bus, points } => {
                if *bus >= crate::fx::BUSES {
                    return Err("The FX bus does not exist.".into());
                }
                crate::automation::validate_return_points(points)?;
            }
            EditCommand::DeleteClips(ids) => {
                let existing: std::collections::HashSet<_> =
                    self.project().clips.iter().map(|clip| clip.id).collect();
                if ids.iter().any(|id| !existing.contains(id)) {
                    return Err("A selected clip does not exist.".into());
                }
            }
            EditCommand::SetClipSounds(sounds) => {
                if sounds.is_empty() {
                    return Ok(());
                }
                sound_indices = clip_sound_indices(self.project(), sounds)?;
            }
            _ => {}
        }
        let explicit_automation = match &command {
            EditCommand::SetAutomation { track, lane, .. } => {
                self.automation_gesture == Some((*track, *lane))
            }
            EditCommand::SetReturnAutomation { bus, .. } => self.return_gesture == Some(*bus),
            _ => false,
        };
        let gesture = if explicit_automation {
            match &command {
                EditCommand::SetReturnAutomation { bus, .. } => Some(Gesture::Return(*bus)),
                _ => self
                    .automation_gesture
                    .map(|(track, lane)| Gesture::Automation(track, lane)),
            }
        } else {
            self.end_automation_gesture();
            match &command {
                EditCommand::SetClipSounds(sounds) => {
                    Gesture::clips(sounds, &sound_indices, self.project())
                }
                _ => Gesture::of(&command, self.project()),
            }
        };
        let now = std::time::Instant::now();
        let continuing = gesture.is_some()
            && self.gesture.as_ref().is_some_and(|(previous, at)| {
                Some(previous) == gesture.as_ref()
                    && (explicit_automation || now.duration_since(*at) < self.gesture_gap)
            });
        let generation = self.editor.generation();
        let master_only = matches!(
            command,
            EditCommand::SetMaster(_) | EditCommand::SetEffects(_)
        );
        let sections_only = matches!(command, EditCommand::SetSections(_));
        // The mix renders the tracks that the pedals and the buses listen
        // to, so a new key prepares it again.
        let side_keys = self.project().side_keys();
        let levels_only = match &command {
            EditCommand::SetMixer { tracks, .. } => {
                tracks.len() == self.project().tracks.len()
                    && self
                        .project()
                        .tracks
                        .iter()
                        .zip(tracks)
                        .all(|(old, new)| levels_differ_only(old, new))
            }
            EditCommand::SetTrack { index, track } => {
                levels_differ_only(&self.project().tracks[*index], track)
            }
            _ => false,
        };
        if continuing && self.editor.can_undo() {
            match command {
                EditCommand::SetTempo(bpm) => self.editor.transact_continuing_in_place(
                    |p| {
                        let old = std::mem::replace(&mut p.bpm, bpm);
                        (old, old != p.bpm)
                    },
                    Project::validate_bpm,
                    |p, old| p.bpm = old,
                )?,
                EditCommand::SetMixer {
                    tracks,
                    master_gain,
                } => self.editor.transact_continuing_in_place(
                    |p| {
                        let old = (
                            std::mem::replace(&mut p.tracks, tracks),
                            std::mem::replace(&mut p.master_gain, master_gain),
                        );
                        let changed = old.0 != p.tracks || old.1 != p.master_gain;
                        (old, changed)
                    },
                    |p| {
                        p.validate_master_gain()?;
                        p.validate_tracks()?;
                        p.validate_keys()
                    },
                    |p, (tracks, master_gain)| {
                        p.tracks = tracks;
                        p.master_gain = master_gain;
                    },
                )?,
                EditCommand::SetTrack { index, track } => {
                    self.editor.transact_continuing_in_place(
                        |p| {
                            let old = std::mem::replace(&mut p.tracks[index], *track);
                            let changed = old != p.tracks[index];
                            (old, changed)
                        },
                        |p| {
                            p.validate_track(index)?;
                            p.validate_keys()
                        },
                        |p, old| p.tracks[index] = old,
                    )?
                }
                EditCommand::SetMaster(chain) => self.editor.transact_continuing_in_place(
                    |p| {
                        let old = std::mem::replace(&mut p.master, chain);
                        let changed = old != p.master;
                        (old, changed)
                    },
                    |p| {
                        p.master.validate()?;
                        p.validate_keys()
                    },
                    |p, old| p.master = old,
                )?,
                EditCommand::SetEffects(buses) => self.editor.transact_continuing_in_place(
                    |p| {
                        let old = std::mem::replace(&mut p.fx, buses);
                        let changed = old != p.fx;
                        (old, changed)
                    },
                    |p| {
                        p.fx.validate()?;
                        p.validate_keys()
                    },
                    |p, old| p.fx = old,
                )?,
                EditCommand::SetAutomation {
                    track,
                    lane,
                    points,
                } => self.editor.transact_continuing_in_place(
                    |p| {
                        let target = p.tracks[track].automation.lane_mut(lane);
                        let old = std::mem::replace(target, points);
                        let changed = old != *target;
                        (old, changed)
                    },
                    |p| p.validate_track(track),
                    |p, old| *p.tracks[track].automation.lane_mut(lane) = old,
                )?,
                EditCommand::SetReturnAutomation { bus, points } => {
                    self.editor.transact_continuing_in_place(
                        |p| {
                            let old = std::mem::replace(&mut p.return_automation[bus], points);
                            let changed = old != p.return_automation[bus];
                            (old, changed)
                        },
                        |p| p.validate_return_lane(bus),
                        |p, old| p.return_automation[bus] = old,
                    )?
                }
                EditCommand::SetClip {
                    id: _,
                    start_tick,
                    gain,
                    offset_frames,
                    length_ticks,
                    fades,
                    pitch_cents,
                    reverse,
                } => {
                    let index = clip_index.expect("validated clip index");
                    self.editor.transact_continuing_in_place(
                        |p| {
                            let clip = &mut p.clips[index];
                            let old = clip.clone();
                            clip.start_tick = start_tick;
                            clip.gain = gain;
                            clip.offset_frames = offset_frames;
                            clip.length_ticks = length_ticks;
                            clip.fades = fades.fit(length_ticks);
                            clip.pitch_cents = pitch_cents;
                            clip.reverse = reverse;
                            let changed = old != *clip;
                            (old, changed)
                        },
                        |p| Project::validate_clip_fields(&p.clips[index]),
                        |p, old| p.clips[index] = old,
                    )?;
                }
                EditCommand::SetClipSounds(sounds) => self.editor.transact_continuing_in_place(
                    |p| {
                        let mut saved = Vec::with_capacity(sounds.len());
                        let mut changed = false;
                        for (sound, index) in sounds.into_iter().zip(sound_indices.iter().copied())
                        {
                            let clip = &mut p.clips[index];
                            let old = clip.clone();
                            clip.gain = sound.gain;
                            clip.fades = sound.fades.fit(clip.length_ticks);
                            clip.pitch_cents = sound.pitch_cents;
                            clip.reverse = sound.reverse;
                            changed |= old != *clip;
                            saved.push((index, old));
                        }
                        (saved, changed)
                    },
                    |p| {
                        for &index in &sound_indices {
                            Project::validate_clip_fields(&p.clips[index])?;
                        }
                        Ok(())
                    },
                    |p, saved| {
                        for (index, old) in saved {
                            p.clips[index] = old;
                        }
                    },
                )?,
                other => {
                    self.editor.transact_continuing(|project| match other {
                        EditCommand::SetLoop(region) => project.loop_region = region,
                        EditCommand::SetSections(sections) => project.sections = sections,
                        EditCommand::SetSwitch { track, switch, on } => {
                            *switch.get_mut(&mut project.tracks[track]) = on;
                        }
                        EditCommand::DeleteClips(ids) => {
                            let ids: BTreeSet<_> = ids.into_iter().collect();
                            project.clips.retain(|clip| !ids.contains(&clip.id));
                        }
                        _ => unreachable!("continuing variants handled in place"),
                    })?;
                }
            }
        } else {
            let apply = |project: &mut Project| match command {
                EditCommand::SetTempo(bpm) => project.bpm = bpm,
                EditCommand::SetLoop(region) => project.loop_region = region,
                EditCommand::SetSections(sections) => project.sections = sections,
                EditCommand::SetMixer {
                    tracks,
                    master_gain,
                } => {
                    project.tracks = tracks;
                    project.master_gain = master_gain;
                }
                EditCommand::SetTrack { index, track } => project.tracks[index] = *track,
                EditCommand::SetTrackMix { .. } => unreachable!("normalised to SetTrack above"),
                EditCommand::SetClip {
                    id: _,
                    start_tick,
                    gain,
                    offset_frames,
                    length_ticks,
                    fades,
                    pitch_cents,
                    reverse,
                } => {
                    if let Some(index) = clip_index {
                        let clip = &mut project.clips[index];
                        clip.start_tick = start_tick;
                        clip.gain = gain;
                        clip.offset_frames = offset_frames;
                        clip.length_ticks = length_ticks;
                        clip.fades = fades.fit(length_ticks);
                        clip.pitch_cents = pitch_cents;
                        clip.reverse = reverse;
                    }
                }
                EditCommand::SetClipSounds(sounds) => {
                    for (sound, index) in sounds.into_iter().zip(sound_indices) {
                        let clip = &mut project.clips[index];
                        clip.gain = sound.gain;
                        clip.fades = sound.fades.fit(clip.length_ticks);
                        clip.pitch_cents = sound.pitch_cents;
                        clip.reverse = sound.reverse;
                    }
                }
                EditCommand::SetSwitch { track, switch, on } => {
                    *switch.get_mut(&mut project.tracks[track]) = on;
                }
                EditCommand::DeleteClips(ids) => {
                    let ids: BTreeSet<_> = ids.into_iter().collect();
                    project.clips.retain(|clip| !ids.contains(&clip.id));
                }
                EditCommand::SetMaster(chain) => project.master = chain,
                EditCommand::SetEffects(buses) => project.fx = buses,
                EditCommand::SetAutomation {
                    track,
                    lane,
                    points,
                } => *project.tracks[track].automation.lane_mut(lane) = points,
                EditCommand::SetReturnAutomation { bus, points } => {
                    project.return_automation[bus] = points;
                }
            };
            if continuing {
                self.editor.transact_continuing(apply)?;
            } else {
                self.editor.transact(apply)?;
            }
        }
        // An accepted command that changes nothing is not an edit.
        if self.editor.generation() != generation {
            if master_only && self.project().side_keys() == side_keys {
                self.master_changed();
            } else if sections_only {
                self.sections_changed();
            } else if levels_only {
                self.levels_changed();
            } else {
                self.changed();
            }
            self.gesture = gesture.map(|gesture| (gesture, now));
        }
        Ok(())
    }
    /// The length that `place` gives a new clip of this sample.
    pub fn placement_ticks(&self, sample_id: u32) -> Option<u32> {
        let sample = self.library.sample_by_id(sample_id)?;
        let bpm = self
            .project()
            .sample_playback_bpm(sample.id, sample.source_bpm);
        Some(if Product::sample_key(sample_id).is_some() {
            // A native loop's bars are at its own tempo; at half time each
            // bar spans two of the song's.
            (f64::from(sample.bars) * bpm / sample.source_bpm * f64::from(TICKS_PER_BAR)).round()
                as u32
        } else {
            duration_ticks(u64::from(sample.frame_count), sample.sample_rate, bpm)
        })
    }
    pub fn place(&mut self, sample_id: u32, track: usize, tick: u32) -> Result<u64, String> {
        let length = self
            .placement_ticks(sample_id)
            .ok_or("The sample is unavailable.")?;
        let id = self.editor.place(sample_id, track, tick, length)?;
        self.changed();
        Ok(id)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        self.edit(EditCommand::DeleteClips(vec![id]))
    }
    pub fn move_clips(
        &mut self,
        ids: &[u64],
        track_delta: i32,
        tick_delta: i64,
        copy: bool,
    ) -> Result<Vec<u64>, String> {
        let generation = self.editor.generation();
        let ids = self.editor.move_clips(ids, track_delta, tick_delta, copy)?;
        if self.editor.generation() != generation {
            self.changed();
        }
        Ok(ids)
    }
    /// Whether an undo (`redo == false`) or a redo step is available.
    pub fn has_history_step(&self, redo: bool) -> bool {
        if redo {
            self.editor.can_redo()
        } else {
            self.editor.can_undo()
        }
    }
    pub fn history_step(&mut self, redo: bool) -> Result<bool, String> {
        if self.is_recording() {
            return Err("Stop recording before you undo or redo.".into());
        }
        self.end_automation_gesture();
        let changed = if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        };
        if !changed {
            return Ok(false);
        }
        if self.apply_live_history(redo) {
            return Ok(true);
        }
        if let Err(error) = self.restore_history_sources() {
            if redo {
                self.editor.undo();
            } else {
                self.editor.redo();
            }
            return Err(error);
        }
        self.changed();
        Ok(true)
    }

    fn apply_live_history(&mut self, redo: bool) -> bool {
        // A step of the master pedalboard or the FX buses alone leaves the
        // mix and playback running. Level history uses the same live path as
        // the original edit: a render replacement would reset filters, FX
        // tails, and meters.
        match self
            .editor
            .replaced_by_step(redo)
            .and_then(|before| live_step(before, self.project()))
        {
            Some(LivePart::MasterAndFx) => self.master_changed(),
            Some(LivePart::Sections) => self.sections_changed(),
            Some(LivePart::Levels) => self.levels_changed(),
            None => return false,
        }
        true
    }

    fn restore_history_sources(&mut self) -> Result<(), String> {
        // Most steps leave the imported sources as they are. Rebuild the
        // catalog only for a step that adds, removes, or relinks one.
        let library = if self.library.has_imports(&self.project().imported_samples) {
            None
        } else {
            let mut library = (*self.library).clone();
            library.apply_imports(&self.project().imported_samples)?;
            Some(library)
        };
        self.check_history_clips(library.as_ref().unwrap_or(&self.library))?;
        if let Some(library) = library {
            self.editor.refresh_restored_rates(|source| {
                library
                    .sample_by_id(source.id)
                    .map(|sample| sample.sample_rate)
            })?;
            self.set_library(library);
        }
        self.editor.set_libraries(self.library.bindings().to_vec());
        // Replacing the library discards the render worker's decoded sources.
        if !self
            .library
            .has_reserved_missing_samples(&self.editor.project().missing_samples)
        {
            Arc::make_mut(&mut self.library)
                .reserve_missing_samples(&self.editor.project().missing_samples);
        }
        Ok(())
    }

    fn check_history_clips(&self, library: &crate::library::Library) -> Result<(), String> {
        if let Some(clip) = self.project().clips.iter().find(|clip| {
            library.sample_by_id(clip.sample_id).is_none()
                && self.project().missing_sample(clip.sample_id).is_none()
        }) {
            return Err(format!(
                "Cannot restore this edit because sample {} is unavailable. Add its sample \
                 library and try again.",
                clip.sample_id
            ));
        }
        Ok(())
    }
}
