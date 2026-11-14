//! Clip selection and the edits that act on it: placing, moving, duplicating
//! and deleting clips, the clip previews, and undo and redo.
use super::*;

/// The selected sound, clips, and track, and what an empty lane click does.
pub(super) struct SelectionState {
    pub(super) sample: Option<u32>,
    pub(super) clip: Option<u64>,
    pub(super) clips: BTreeSet<u64>,
    /// The track whose page the Inspector shows while no clip is selected.
    pub(super) track: Option<usize>,
    /// A click on an empty lane places the selected sound. Choosing a clip
    /// turns this off, so the next empty click only clears the selection.
    pub(super) place_on_click: bool,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            sample: None,
            clip: None,
            clips: BTreeSet::new(),
            track: None,
            place_on_click: true,
        }
    }
}

impl MixJamApp {
    pub(super) fn clear_selection(&mut self) {
        self.selection.sample = None;
        self.selection.clip = None;
        self.selection.clips.clear();
        self.selection.track = None;
        // Project and library replacement invalidate every in-flight gesture.
        self.boards.master_view_generation = self.boards.master_view_generation.wrapping_add(1);
        self.gestures.drag = None;
        self.gestures.handle_drag = None;
        self.gestures.automation_drag = None;
        self.session.end_automation_gesture();
        self.gestures.marquee_origin = None;
        self.gestures.marquee_base.clear();
        self.score.loop_start = None;
        self.cancel_section_edit();
        self.clip_menu.clip = None;
        self.clip_menu.missing_sample = None;
        self.conflicts.open = false;
        self.conflicts.dismissed = false;
        self.conflicts.error = None;
    }

    pub(super) fn cancel_section_edit(&mut self) {
        self.sections.rename = None;
        self.sections.menu = (None, 0);
    }

    pub(super) fn reconcile_selection(&mut self) {
        self.selection.clips.retain(|id| {
            self.session
                .project()
                .clips
                .iter()
                .any(|clip| clip.id == *id)
        });
        self.selection.clip = self.selection.clips.iter().next().copied();
        if self
            .selection
            .track
            .is_some_and(|track| track >= TRACK_COUNT)
        {
            self.selection.track = None;
        }
        if self.selection.sample.is_some_and(|id| {
            self.session.library().sample_by_id(id).is_none()
                && self.session.missing_sample(id).is_none()
        }) {
            self.selection.sample = None;
        }
    }

    pub(super) fn apply(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.error(error);
        } else {
            self.sync_notice();
        }
    }

    pub(super) fn try_place(
        &mut self,
        sample_id: u32,
        track: usize,
        tick: u32,
    ) -> Result<u64, String> {
        let id = self.session.place(sample_id, track, tick)?;
        self.selection.clip = Some(id);
        self.selection.clips = [id].into_iter().collect();
        self.sync_notice();
        Ok(id)
    }

    pub(super) fn place(&mut self, sample_id: u32, track: usize, tick: u32) {
        if let Err(error) = self.try_place(sample_id, track, tick) {
            self.error(error);
        }
    }

    /// Move or copy clips and select the result.
    pub(super) fn try_move_clips(
        &mut self,
        ids: &[u64],
        track_delta: i32,
        tick_delta: i64,
        copy: bool,
    ) -> Result<Vec<u64>, String> {
        let ids = self
            .session
            .move_clips(ids, track_delta, tick_delta, copy)?;
        self.selection.clip = ids.first().copied();
        self.selection.clips = ids.iter().copied().collect();
        self.sync_notice();
        Ok(ids)
    }

    pub(super) fn history_step(&mut self, redo: bool) {
        self.cancel_section_edit();
        self.gestures.handle_drag = None;
        self.gestures.automation_drag = None;
        self.session.end_automation_gesture();
        match self.session.history_step(redo) {
            Ok(true) => {
                self.reconcile_selection();
                self.sync_notice();
            }
            Ok(false) => {}
            Err(error) => self.error(error),
        }
    }

    /// Select clip `id`, or add it to the selection. A chosen track clears,
    /// so the Inspector never falls back to it once the clips are gone.
    pub(super) fn select_clip(&mut self, id: u64, additive: bool) {
        self.selection.track = None;
        if let Some(clip) = self.session.project().clips.iter().find(|c| c.id == id) {
            self.selection.sample = Some(clip.sample_id);
        }
        if additive {
            if !self.selection.clips.insert(id) {
                self.selection.clips.remove(&id);
            }
        } else {
            self.selection.clips.clear();
            self.selection.clips.insert(id);
        }
        self.selection.clip = self
            .selection
            .clips
            .contains(&id)
            .then_some(id)
            .or_else(|| self.selection.clips.iter().next().copied());
    }

    pub(super) fn delete_selection(&mut self) {
        if self.selection.clips.is_empty() {
            return;
        }
        let result = self.session.edit(EditCommand::DeleteClips(
            self.selection.clips.iter().copied().collect(),
        ));
        if result.is_ok() {
            self.selection.clip = None;
            self.selection.clips.clear();
            // A fade or trim handle of a deleted clip must not edit on release.
            self.gestures.handle_drag = None;
        }
        self.apply(result);
    }

    /// Choose the next clip along a track (`dx`) or the nearest clip on the
    /// next used track (`dy`). Without a selection, choose the first clip.
    pub(super) fn step_clip_selection(&mut self, dx: i32, dy: i32) {
        let clips = &self.session.project().clips;
        let current = self
            .selection
            .clip
            .and_then(|id| clips.iter().find(|clip| clip.id == id));
        let next = match current {
            None => clips
                .iter()
                .min_by_key(|clip| (clip.track, clip.start_tick)),
            Some(current) if dy == 0 => clips
                .iter()
                .filter(|clip| clip.track == current.track)
                .filter(|clip| {
                    if dx > 0 {
                        clip.start_tick > current.start_tick
                    } else {
                        clip.start_tick < current.start_tick
                    }
                })
                .min_by_key(|clip| clip.start_tick.abs_diff(current.start_tick)),
            Some(current) => clips
                .iter()
                .filter(|clip| {
                    if dy > 0 {
                        clip.track > current.track
                    } else {
                        clip.track < current.track
                    }
                })
                .min_by_key(|clip| {
                    (
                        clip.track.abs_diff(current.track),
                        clip.start_tick.abs_diff(current.start_tick),
                    )
                }),
        };
        if let Some(id) = next.map(|clip| clip.id) {
            self.select_clip(id, false);
            self.selection.place_on_click = false;
        }
    }

    pub(super) fn preview_selection(&mut self) {
        if let Some(id) = self.selection.clip {
            self.preview_clip(id);
        }
    }

    pub(super) fn preview_clip(&mut self, id: u64) {
        if let Some(sample) = self
            .session
            .project()
            .clips
            .iter()
            .find(|clip| clip.id == id)
            .map(|clip| clip.sample_id)
        {
            self.preview(sample);
        }
    }

    /// Copy the selected clips to the bars that follow the selection.
    pub(super) fn duplicate_selection(&mut self) {
        let selected: Vec<_> = self
            .session
            .project()
            .clips
            .iter()
            .filter(|clip| self.selection.clips.contains(&clip.id))
            .collect();
        let (Some(start), Some(end)) = (
            selected.iter().map(|clip| clip.start_tick).min(),
            selected
                .iter()
                .map(|clip| clip.start_tick.saturating_add(clip.length_ticks))
                .max(),
        ) else {
            return;
        };
        // Whole bars keep the copy on the grid.
        let span = (end - start).div_ceil(TICKS_PER_BAR).max(1) * TICKS_PER_BAR;
        let ids: Vec<_> = selected.iter().map(|clip| clip.id).collect();
        if let Err(error) = self.try_move_clips(&ids, 0, i64::from(span), true) {
            self.error(error);
        }
    }

    pub(super) fn select_all_clips(&mut self) {
        self.selection.clips = self
            .session
            .project()
            .clips
            .iter()
            .map(|clip| clip.id)
            .collect();
        self.selection.clip = self.selection.clips.iter().next().copied();
        self.selection.place_on_click = false;
    }
}
