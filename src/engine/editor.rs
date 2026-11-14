//! The editor: validated edits to a project with undo and redo.
use super::{Clip, ImportedSample, MAX_TICKS, Project, TRACK_COUNT};
use std::collections::{HashMap, HashSet};

pub struct Editor {
    pub(super) project: Project,
    pub(super) undo: Vec<Project>,
    pub(super) redo: Vec<Project>,
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
    /// Take new sample sources: library bindings and the Sounds palette. They
    /// are not an edit, so undo and redo keep their steps when every step's
    /// clips name the same sounds in the new palette. Otherwise, and when
    /// `project` differs in more than its sources, nothing changes and the
    /// project comes back.
    pub fn rebind_sources(&mut self, project: Project) -> Result<(), Box<Project>> {
        let mut bound = self.project.clone();
        bound.libraries.clone_from(&project.libraries);
        bound.imported_samples = project.imported_samples.clone();
        if bound != project {
            return Err(Box::new(project));
        }
        let palette: HashMap<u32, &ImportedSample> = project
            .imported_samples
            .iter()
            .map(|sample| (sample.id, sample))
            .collect();
        let same_sounds = |step: &Project| {
            let before: HashMap<u32, &ImportedSample> = step
                .imported_samples
                .iter()
                .map(|sample| (sample.id, sample))
                .collect();
            step.clips
                .iter()
                .all(|clip| before.get(&clip.sample_id) == palette.get(&clip.sample_id))
        };
        if !self.undo.iter().chain(&self.redo).all(same_sounds) {
            return Err(Box::new(project));
        }
        for step in self.undo.iter_mut().chain(&mut self.redo) {
            step.libraries.clone_from(&project.libraries);
            step.imported_samples = project.imported_samples.clone();
        }
        self.project = project;
        self.generation += 1;
        Ok(())
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
                reverse: false,
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
