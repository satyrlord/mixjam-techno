//! Missing and ambiguous sample references of an opened song, and how a user resolves them.
use super::{MissingReference, MissingSample, Project, SampleConflict, Session, TICKS_PER_BAR};
use crate::library::Library;
use std::path::Path;

impl Session {
    pub fn missing_samples(&self) -> &[MissingSample] {
        &self.project().missing_samples
    }
    pub fn missing_sample(&self, id: u32) -> Option<&MissingSample> {
        self.project().missing_sample(id)
    }
    pub fn sample_conflicts(&self) -> &[SampleConflict] {
        &self.project().sample_conflicts
    }
    /// Replace every placement of one ambiguous reference with the chosen WAV.
    pub fn resolve_sample_conflict(
        &mut self,
        conflict_index: usize,
        candidate_index: usize,
    ) -> Result<(), String> {
        self.require_idle_recording()?;
        let conflict = self
            .sample_conflicts()
            .get(conflict_index)
            .ok_or("The sample conflict no longer exists.")?
            .clone();
        let path = conflict
            .candidates
            .get(candidate_index)
            .ok_or("The selected sample choice no longer exists.")?;
        let path = std::fs::canonicalize(path)
            .map_err(|error| format!("Cannot open the selected sample: {error}"))?;
        let mut project = self.project().clone();
        let mut next_id = last_imported_id(&project, &self.library);
        for id in &conflict.sample_ids {
            let missing = project.missing_sample(*id);
            let original = project
                .imported_samples
                .iter()
                .find(|sample| sample.id == *id);
            let known = self.library.sample_by_id(*id);
            let (mut imported, previous_rate, preserve_frames) = if let Some(source) = original {
                let rate = source.source_sample_rate.or_else(|| {
                    known
                        .filter(|sample| sample.wav_path.as_deref() == Some(source.path.as_path()))
                        .map(|sample| sample.sample_rate)
                });
                (
                    source.clone(),
                    rate.unwrap_or(legacy_format::NATIVE_SAMPLE_RATE),
                    rate.is_none(),
                )
            } else if let Some(source) = missing {
                (
                    crate::engine::ImportedSample {
                        id: *id,
                        name: source.name.clone(),
                        group_name: source.group_name().unwrap_or("Resolved samples").into(),
                        path: path.clone(),
                        source_bpm: source.source_bpm,
                        source_sample_rate: Some(source.sample_rate),
                        origin: match &source.reference {
                            MissingReference::External { origin, .. } => *origin,
                            _ => None,
                        },
                    },
                    source.sample_rate,
                    source.preserves_trim_frames(),
                )
            } else if let Some(source) = known {
                (
                    crate::engine::ImportedSample {
                        id: *id,
                        name: source.name.clone(),
                        group_name: source.group_name.clone(),
                        path: path.clone(),
                        source_bpm: source.source_bpm,
                        source_sample_rate: Some(source.sample_rate),
                        origin: None,
                    },
                    source.sample_rate,
                    false,
                )
            } else {
                return Err("The sample conflict source is unavailable.".into());
            };
            let previous_bpm = imported.source_bpm;
            if *id < crate::products::IMPORTED_ID_START {
                next_id = next_id
                    .checked_add(1)
                    .ok_or("Imported sample ID limit reached.")?;
                imported.id = next_id;
            }
            imported.path = path.clone();
            let ready = Library::check_import(&imported)?;
            if let Some(ticks) = conflict.source_spans.get(id) {
                imported.source_bpm = f64::from(*ticks) * 240.0 * f64::from(ready.sample_rate)
                    / (f64::from(TICKS_PER_BAR) * f64::from(ready.frame_count));
            }
            imported.source_sample_rate = Some(ready.sample_rate);
            for clip in project
                .clips
                .iter_mut()
                .filter(|clip| clip.sample_id == *id)
            {
                clip.offset_frames = crate::engine::rescale_trim(
                    clip.offset_frames,
                    preserve_frames,
                    (previous_rate, previous_bpm),
                    (ready.sample_rate, imported.source_bpm),
                )
                .ok_or("The selected sample trim exceeds the supported frame range.")?;
                clip.sample_id = imported.id;
            }
            let updated = &mut project.sample_conflicts[conflict_index];
            for source_id in &mut updated.sample_ids {
                if source_id == id {
                    *source_id = imported.id;
                }
            }
            if imported.id != *id
                && let Some(span) = updated.source_spans.remove(id)
            {
                updated.source_spans.insert(imported.id, span);
            }
            project.imported_samples.retain(|sample| sample.id != *id);
            project.missing_samples.retain(|sample| sample.id != *id);
            project.imported_samples.push(imported);
        }
        project.sample_conflicts[conflict_index].selected = candidate_index;
        project.sample_conflicts[conflict_index].confirmed = true;
        project.validate()?;
        let mut library = (*self.library).clone();
        library.apply_imports(&project.imported_samples)?;
        library.reserve_missing_samples(&project.missing_samples);
        self.editor.transact(|current| *current = project)?;
        self.set_library(library);
        self.changed();
        Ok(())
    }
    /// Accept the current choices as one undoable action.
    pub fn confirm_sample_conflicts(&mut self) -> Result<(), String> {
        self.require_idle_recording()?;
        if self
            .sample_conflicts()
            .iter()
            .all(|conflict| conflict.confirmed)
        {
            return Ok(());
        }
        self.editor.transact(|project| {
            for conflict in &mut project.sample_conflicts {
                conflict.confirmed = true;
            }
        })?;
        self.changed();
        Ok(())
    }
    /// Bind every occurrence of one retained source to an explicitly chosen WAV.
    /// Manual choices receive imported identities, never verified factory identities.
    pub fn resolve_missing_sample(&mut self, id: u32, path: &Path) -> Result<(), String> {
        self.require_idle_recording()?;
        let selected = self
            .missing_sample(id)
            .ok_or("The missing sample no longer exists.")?
            .clone();
        let path = std::fs::canonicalize(path)
            .map_err(|error| format!("Cannot open the selected sample: {error}"))?;
        let path = crate::project_io::retain_manual_wave(&path, &self.settings_root)?;
        let mut project = self.project().clone();
        let matched: Vec<_> = project
            .missing_samples
            .iter()
            .filter(|sample| same_missing_reference(&selected.reference, &sample.reference))
            .cloned()
            .collect();
        let mut next_id = last_imported_id(&project, &self.library);
        for missing in matched {
            let new_id = if missing.id >= crate::products::IMPORTED_ID_START {
                missing.id
            } else {
                next_id = next_id
                    .checked_add(1)
                    .ok_or("Imported sample ID limit reached.")?;
                next_id
            };
            let mut imported = crate::engine::ImportedSample {
                id: new_id,
                name: missing.name.clone(),
                group_name: missing.group_name().unwrap_or("Resolved samples").into(),
                path: path.clone(),
                source_bpm: missing.source_bpm,
                source_sample_rate: None,
                origin: None,
            };
            let ready = Library::check_import(&imported)?;
            imported.source_bpm =
                missing.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
            imported.source_sample_rate = Some(ready.sample_rate);
            for clip in project
                .clips
                .iter_mut()
                .filter(|clip| clip.sample_id == missing.id)
            {
                clip.offset_frames = crate::engine::rescale_trim(
                    clip.offset_frames,
                    missing.preserves_trim_frames(),
                    (missing.sample_rate, missing.source_bpm),
                    (ready.sample_rate, imported.source_bpm),
                )
                .ok_or("The selected sample trim exceeds the supported frame range.")?;
                clip.sample_id = new_id;
            }
            project.imported_samples.push(imported);
            project
                .missing_samples
                .retain(|sample| sample.id != missing.id);
            for conflict in &mut project.sample_conflicts {
                conflict.sample_ids.retain(|id| *id != missing.id);
                conflict.source_spans.remove(&missing.id);
            }
        }
        project
            .sample_conflicts
            .retain(|conflict| !conflict.sample_ids.is_empty());
        project.validate()?;
        let mut library = (*self.library).clone();
        library.apply_imports(&project.imported_samples)?;
        library.reserve_missing_samples(&project.missing_samples);
        self.editor.transact(|current| *current = project)?;
        self.set_library(library);
        self.changed();
        Ok(())
    }
}

/// The highest imported sample ID in use; new imports follow it.
fn last_imported_id(project: &Project, library: &Library) -> u32 {
    project
        .imported_samples
        .iter()
        .map(|sample| sample.id)
        .chain(project.missing_samples.iter().map(|sample| sample.id))
        .chain(library.samples.iter().map(|sample| sample.id))
        .max()
        .unwrap_or(0)
        .max(crate::products::IMPORTED_ID_START - 1)
}

pub(super) fn same_missing_reference(left: &MissingReference, right: &MissingReference) -> bool {
    match (left, right) {
        (
            MissingReference::External { path: left, .. },
            MissingReference::External { path: right, .. },
        ) => {
            let normalized = |path: &Path| {
                let path = path.to_string_lossy().replace('\\', "/").to_lowercase();
                let path = if let Some(unc) = path.strip_prefix("//?/unc/") {
                    format!("//{unc}")
                } else {
                    path.strip_prefix("//?/").unwrap_or(&path).to_owned()
                };
                path.split('/')
                    .filter(|part| !part.is_empty() && *part != ".")
                    .collect::<Vec<_>>()
                    .join("/")
            };
            normalized(left) == normalized(right)
        }
        _ => left == right,
    }
}
