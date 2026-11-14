//! Typed edit commands and how a session applies them, with undo and redo.
use super::gesture::Gesture;
use super::{
    Fades, MAX_PITCH_CENTS, MasterChain, Product, Project, Section, Session, TICKS_PER_BAR,
    TRACK_COUNT, Track, duration_ticks,
};
use std::{collections::BTreeSet, sync::Arc};

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
    let mut same = new.clone();
    same.gain = old.gain;
    same.pan = old.pan;
    same.polarity_inverted = old.polarity_inverted;
    same == *old
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
                if !(-MAX_PITCH_CENTS..=MAX_PITCH_CENTS).contains(pitch_cents) {
                    return Err("Clip pitch must be from -24 to 24 semitones.".into());
                }
                let clip = self
                    .project()
                    .clips
                    .iter()
                    .find(|clip| clip.id == *id)
                    .ok_or("The clip does not exist.")?;
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
            Gesture::of(&command, self.project())
        };
        let now = std::time::Instant::now();
        let continuing = gesture.is_some()
            && self.gesture.is_some_and(|(previous, at)| {
                Some(previous) == gesture
                    && (explicit_automation || now.duration_since(at) < self.gesture_gap)
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
            EditCommand::SetClip {
                id,
                start_tick,
                gain,
                offset_frames,
                length_ticks,
                fades,
                pitch_cents,
                reverse,
            } => {
                if let Some(clip) = project.clips.iter_mut().find(|clip| clip.id == id) {
                    clip.start_tick = start_tick;
                    clip.gain = gain;
                    clip.offset_frames = offset_frames;
                    clip.length_ticks = length_ticks;
                    clip.fades = fades.fit(length_ticks);
                    clip.pitch_cents = pitch_cents;
                    clip.reverse = reverse;
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
        let before = self.project().clone();
        let changed = if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        };
        if !changed {
            return Ok(false);
        }
        // A step of the master pedalboard or the FX buses alone leaves the
        // mix and playback running.
        let master_only = (before.master != self.project().master
            || before.fx != self.project().fx)
            && before.side_keys() == self.project().side_keys()
            && Project {
                master: self.project().master,
                fx: self.project().fx,
                ..before.clone()
            } == *self.project();
        if master_only {
            self.master_changed();
            return Ok(true);
        }
        let sections_only = before.sections != self.project().sections
            && (Project {
                sections: self.project().sections.clone(),
                ..before.clone()
            }) == *self.project();
        if sections_only {
            self.sections_changed();
            return Ok(true);
        }
        // Level history uses the same live path as the original edit. A
        // render replacement would reset filters, FX tails, and meters.
        let mut without_level_change = self.project().clone();
        without_level_change.master_gain = before.master_gain;
        for (track, old) in without_level_change.tracks.iter_mut().zip(&before.tracks) {
            track.gain = old.gain;
            track.pan = old.pan;
            track.polarity_inverted = old.polarity_inverted;
        }
        if without_level_change == before {
            self.levels_changed();
            return Ok(true);
        }
        // Most steps leave the imported sources as they are. Rebuild the
        // catalog only for a step that adds, removes, or relinks one.
        let mut library = if self.library.has_imports(&self.project().imported_samples) {
            None
        } else {
            Some((*self.library).clone())
        };
        let result = match &mut library {
            Some(library) => library.apply_imports(&self.project().imported_samples),
            None => Ok(()),
        }
        .and_then(|()| {
            let library = library.as_ref().unwrap_or(&self.library);
            match self.project().clips.iter().find(|clip| {
                library.sample_by_id(clip.sample_id).is_none()
                    && self.project().missing_sample(clip.sample_id).is_none()
            }) {
                Some(clip) => Err(format!(
                    "Cannot restore this edit because sample {} is unavailable. Add its sample \
                     library and try again.",
                    clip.sample_id
                )),
                None => Ok(()),
            }
        });
        if let Err(error) = result {
            if redo {
                self.editor.undo();
            } else {
                self.editor.redo();
            }
            return Err(error);
        }
        if let Some(library) = library {
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
        self.changed();
        Ok(true)
    }
}
