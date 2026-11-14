//! Audio sources: registering and saving sounds and patterns, processing them, and the Time Stretcher.
use super::{EffectsState, Pcm, Session, StepSynthState, pattern_beside};
use crate::audio::{self};
use std::{path::Path, sync::mpsc};

/// The source and tempo values captured for one Time Stretcher operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StretchSettings {
    pub source_id: u32,
    pub source_bpm: f64,
    pub target_bpm: f64,
}

pub(super) struct StretchJob {
    pub(super) settings: StretchSettings,
    pub(super) receiver: mpsc::Receiver<Result<Pcm, String>>,
}

impl Session {
    pub fn register_file(
        &mut self,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        let mut library = (*self.library).clone();
        library.reserve_missing_samples(&self.project().missing_samples);
        let imported = library.import_wav_with_tempo(path, None, group, source_bpm)?;
        let id = imported.id;
        self.editor.transact(|project| {
            if let Some(existing) = project
                .imported_samples
                .iter_mut()
                .find(|sample| sample.id == id)
            {
                *existing = imported;
            } else {
                project.imported_samples.push(imported);
            }
        })?;
        self.set_library(library);
        self.changed();
        Ok(id)
    }
    pub fn save_pcm(
        &mut self,
        pcm: &Pcm,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        Self::validate_pcm_save(pcm, path, group, source_bpm)?;
        pcm.write_wav(path)?;
        self.register_file(path, group, source_bpm)
    }
    /// Save the sequence and its note pattern in one publication batch.
    pub fn save_sequence(
        &mut self,
        pcm: &Pcm,
        path: &Path,
        state: &StepSynthState,
    ) -> Result<u32, String> {
        use std::io::Write;
        state.validate()?;
        Self::validate_pcm_save(pcm, path, "Step synth", state.bpm)?;
        let pattern = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
        crate::atomic_file::write_many(&[path.to_path_buf(), pattern_beside(path)], |files| {
            pcm.write_wav_to(&mut files[0])?;
            files[1]
                .write_all(&pattern)
                .map_err(|error| error.to_string())
        })?;
        self.register_file(path, "Step synth", state.bpm)
    }
    fn validate_pcm_save(
        pcm: &Pcm,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<(), String> {
        pcm.validate()?;
        crate::engine::validate_import_metadata(&crate::engine::ImportedSample {
            id: crate::products::IMPORTED_ID_START,
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .trim()
                .to_string(),
            group_name: group.trim().to_string(),
            path: path.to_path_buf(),
            source_bpm,
            source_sample_rate: None,
            origin: None,
        })?;
        if pcm.frames() == 0 {
            return Err("The audio source is empty.".into());
        }
        Ok(())
    }
    pub fn source_pcm(&self, id: Option<u32>) -> Result<Pcm, String> {
        if id.is_some_and(|id| self.missing_sample(id).is_some()) {
            return Err("This sample is missing. Select a folder with its source audio.".into());
        }
        self.library
            .read_pcm(id.ok_or("Choose a sound in the palette first.")?)
    }
    pub fn source_bpm(&self, id: Option<u32>) -> f64 {
        id.and_then(|id| self.library.sample_by_id(id))
            .map_or(crate::engine::SOURCE_BPM, |sample| sample.source_bpm)
    }
    pub fn processed_pcm(&self, id: Option<u32>, effects: &EffectsState) -> Result<Pcm, String> {
        crate::effects::process(&self.source_pcm(id)?, effects, self.source_bpm(id))
    }
    pub fn step_synth_pcm(
        &self,
        id: Option<u32>,
        path: Option<&Path>,
        state: &StepSynthState,
    ) -> Result<Pcm, String> {
        let source = if let Some(path) = path {
            let mut source = if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            {
                audio::read_wav(path)?
            } else {
                crate::library::read_packed_file(path)?
            };
            audio::hide_stray_end(&mut source);
            source
        } else {
            self.source_pcm(id)?
        };
        crate::synth::render(&source, state)
    }
    pub fn edited_pcm(
        &self,
        id: Option<u32>,
        start: u64,
        end: u64,
        pitch: f64,
    ) -> Result<Pcm, String> {
        crate::sample_edit::pitch_shift(
            &crate::sample_edit::trim(&self.source_pcm(id)?, start, end)?,
            pitch,
        )
    }
    /// Read and stretch the selected source on a worker. Later control changes
    /// do not change the settings or source catalog used by this request.
    pub fn start_stretch(&mut self, settings: StretchSettings) -> Result<(), String> {
        if self.stretch_job.is_some() || self.stretch_result.is_some() {
            return Err("Wait for the current stretch result before you start another.".into());
        }
        if self.missing_sample(settings.source_id).is_some() {
            return Err("This sample is missing. Select a folder with its source audio.".into());
        }
        let library = self.library.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("mixjam-techno-stretch".into())
            .spawn(move || {
                let result = library.read_pcm(settings.source_id).and_then(|pcm| {
                    crate::sample_edit::time_stretch(&pcm, settings.source_bpm, settings.target_bpm)
                });
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Cannot start Time Stretcher: {error}"))?;
        self.stretch_job = Some(StretchJob { settings, receiver });
        Ok(())
    }
    pub fn is_stretching(&self) -> bool {
        self.stretch_job.is_some()
    }
    /// Receive a completed operation once. Call `poll` to advance the worker.
    pub fn take_stretch_result(&mut self) -> Option<(StretchSettings, Result<Pcm, String>)> {
        self.stretch_result.take()
    }
    pub(super) fn poll_stretch(&mut self) {
        let Some(job) = self.stretch_job.as_ref() else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Time Stretcher stopped unexpectedly. Try again.".into())
            }
        };
        self.stretch_result = Some((job.settings, result));
        self.stretch_job = None;
    }
    pub(super) fn cancel_stretch(&mut self) {
        let settings = self
            .stretch_job
            .take()
            .map(|job| job.settings)
            .or_else(|| self.stretch_result.take().map(|(settings, _)| settings));
        if let Some(settings) = settings {
            self.stretch_result = Some((
                settings,
                Err("The song or sample library changed. Start Time Stretcher again.".into()),
            ));
        }
    }
    pub fn save_pattern(&self, path: &Path, state: &StepSynthState) -> Result<(), String> {
        state.validate()?;
        let bytes = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
        crate::atomic_file::write(path, &bytes)
    }
    pub fn load_pattern(&self, path: &Path) -> Result<StepSynthState, String> {
        use std::io::Read;
        const MAX_BYTES: u64 = 1024 * 1024;
        let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("The pattern file is too large.".into());
        }
        let mut state: StepSynthState =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        state.validate()?;
        state.bpm = crate::engine::nearest_supported_bpm(state.bpm)?;
        Ok(state)
    }
}
