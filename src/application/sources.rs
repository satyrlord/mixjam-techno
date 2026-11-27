//! Audio sources: registering and saving sounds and patterns, processing them, and the Time Stretcher.
use super::{EffectsState, Pcm, Session, StepSynthState, pattern_beside};
use crate::audio;
use crate::engine::{ImportedSample, Project};
use crate::library::Library;
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

struct PreparedSound {
    library: Library,
    project: Project,
    id: u32,
}

fn store_sound(project: &mut Project, imported: ImportedSample) {
    if let Some(existing) = project
        .imported_samples
        .iter_mut()
        .find(|s| s.id == imported.id)
    {
        *existing = imported;
    } else {
        project.imported_samples.push(imported);
    }
}

impl Session {
    pub fn register_file(
        &mut self,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        let sound = self.prepare_sound(|library| {
            library.import_wav_with_tempo(path, None, group, source_bpm)
        })?;
        self.adopt_sound(sound)
    }

    fn prepare_sound(
        &self,
        import: impl FnOnce(&mut Library) -> Result<ImportedSample, String>,
    ) -> Result<PreparedSound, String> {
        let mut library = (*self.library).clone();
        library.reserve_missing_samples(&self.project().missing_samples);
        let imported = import(&mut library)?;
        let id = imported.id;
        let mut project = self
            .project()
            .with_import_rates(|alias| {
                library
                    .sample_by_id(alias.id)
                    .filter(|known| known.wav_path.as_ref() == Some(&imported.path))
                    .map(|known| known.sample_rate)
            })?
            .unwrap_or_else(|| self.project().clone());
        store_sound(&mut project, imported);
        project.prune_sample_conflicts();
        project.validate()?;
        Ok(PreparedSound {
            library,
            project,
            id,
        })
    }

    fn adopt_sound(&mut self, sound: PreparedSound) -> Result<u32, String> {
        self.editor.transact(|current| *current = sound.project)?;
        self.set_library(sound.library);
        self.changed();
        Ok(sound.id)
    }
    pub fn save_pcm(
        &mut self,
        pcm: &Pcm,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        Self::validate_pcm_save(pcm, path, group, source_bpm)?;
        self.write_sound(&[path.to_path_buf()], group, source_bpm, |files| {
            pcm.write_wav_to(&mut files[0])
        })
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
        self.write_sound(
            &[path.to_path_buf(), pattern_beside(path)],
            "Step synth",
            state.bpm,
            |files| {
                pcm.write_wav_to(&mut files[0])?;
                files[1]
                    .write_all(&pattern)
                    .map_err(|error| error.to_string())
            },
        )
    }

    fn write_sound(
        &mut self,
        paths: &[std::path::PathBuf],
        group: &str,
        source_bpm: f64,
        fill: impl FnOnce(&mut [std::fs::File]) -> Result<(), String>,
    ) -> Result<u32, String> {
        let target = crate::atomic_file::destination_path(&paths[0])?;
        let sound = crate::atomic_file::write_many_checked(paths, fill, |staged| {
            self.prepare_sound(|library| {
                library.import_wav_at(&staged[0], &target, None, group, source_bpm)
            })
        })?;
        self.adopt_sound(sound)
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
    /// The rendered pattern, and the dB it was turned down to stay within
    /// full scale.
    pub fn step_synth_pcm(
        &self,
        id: Option<u32>,
        state: &StepSynthState,
    ) -> Result<(Pcm, f64), String> {
        if state.source_mode == crate::synth::SourceMode::Synth {
            return crate::synth::render_standalone_measured(state);
        }
        crate::synth::render_measured(&self.step_synth_source(id, state)?, state)
    }
    /// The Step synth pattern's source audio: its source file, or else the
    /// palette sound `id`.
    pub fn step_synth_source(
        &self,
        id: Option<u32>,
        state: &StepSynthState,
    ) -> Result<Pcm, String> {
        let Some(path) = state.source.as_deref() else {
            return self.source_pcm(id);
        };
        let mut source = if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        {
            audio::read_wav(path)?
        } else {
            crate::library::read_packed_file(path)?
        };
        audio::hide_stray_end(&mut source);
        Ok(source)
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
        self.load_pattern_snapped(path).map(|(state, _)| state)
    }
    /// Load a pattern, and give the file's own BPM too: the load moves it to
    /// the nearest supported tempo.
    pub fn load_pattern_snapped(&self, path: &Path) -> Result<(StepSynthState, f64), String> {
        use std::io::Read;
        const MAX_BYTES: u64 = 1024 * 1024;
        let file = std::fs::File::open(path).map_err(crate::atomic_file::error_text)?;
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(crate::atomic_file::error_text)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("The file is larger than 1 MB.".into());
        }
        // The parser's own words name JSON details, not the problem.
        let mut state: StepSynthState = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "It is not a Step synth pattern (line {}, column {}). Choose a pattern that the \
                 Step synth saved.",
                error.line(),
                error.column()
            )
        })?;
        state.validate()?;
        let stored = state.bpm;
        state.bpm = crate::engine::nearest_supported_bpm(state.bpm)?;
        Ok((state, stored))
    }
}
