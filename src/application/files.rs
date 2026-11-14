//! Songs on disk and renders of the mix: opening, saving, new mixes, and export.
use super::{Project, Session};
use crate::audio;
use std::{
    path::Path,
    sync::{Arc, mpsc},
};

/// An export running on its own thread. It reports the finished status
/// line, or an error that `failure` introduces.
pub(super) struct Export {
    failure: &'static str,
    receiver: mpsc::Receiver<Result<String, String>>,
    thread: std::thread::JoinHandle<()>,
}

impl Drop for Session {
    /// Let a running export finish its file before the application exits.
    fn drop(&mut self) {
        if let Some(export) = self.export.take() {
            let _ = export.thread.join();
        }
    }
}

impl Session {
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        self.require_idle_recording()?;
        let (project, library, song) = crate::project_io::open_song(path, &self.library)?;
        self.stop();
        self.set_library(library);
        self.replace_project(project);
        self.unsaved = false;
        self.prepare_ahead();
        // Save replaces a song. An import or earlier JSON project asks for a new file.
        self.file = song.then(|| path.to_path_buf());
        Ok(())
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        // A new song, or one named after its earlier file, takes the new
        // file's name. An imported title stays.
        let previous = self.project().title.clone();
        let file_named = self
            .file
            .as_deref()
            .and_then(Path::file_stem)
            .is_some_and(|stem| stem.to_string_lossy() == previous);
        if (previous.is_empty() || previous == crate::engine::DEFAULT_TITLE || file_named)
            && let Some(stem) = path.file_stem()
        {
            self.editor.set_title(stem.to_string_lossy().into_owned());
        }
        if let Err(error) =
            crate::project_io::save_with_library(self.project(), &self.library, path)
        {
            self.editor.set_title(previous);
            return Err(error);
        }
        // An edit after Save must be undoable back to the saved state.
        self.gesture = None;
        self.file = Some(path.to_path_buf());
        self.unsaved = false;
        Ok(())
    }
    /// The mix prepared for the project as it is now: playback's, when it
    /// is current, or a new one.
    pub(super) fn current_mix(&self) -> Result<Arc<audio::Mix>, String> {
        match &self.render {
            Some(mix)
                if !self.render_dirty
                    && mix.has_levels(self.project())
                    && mix.quality() == crate::playback_quality::PlaybackQuality::FULL =>
            {
                Ok(Arc::clone(mix))
            }
            _ => audio::prepare_project(self.project(), &self.library).map(Arc::new),
        }
    }

    /// Render and write the mix on this thread. Headless callers use this;
    /// the GUI calls `start_export` so that the window stays responsive.
    pub fn export(&self, path: &Path) -> Result<(), String> {
        let (master, fx) = (&self.project().master, &self.project().fx);
        // Playback already rendered an unchanged mix; write that audio.
        self.current_mix()?.write_wav(path, master, fx)
    }

    /// Export each track and each FX return as its own WAV file in
    /// `folder`, on this thread, and give the status line that reports it.
    /// The files are the premaster: the master pedalboard is skipped.
    pub fn export_tracks(&self, folder: &Path) -> Result<String, String> {
        let stems =
            self.current_mix()?
                .write_stems(folder, &self.project().title, &self.project().fx)?;
        Ok(tracks_exported(stems, folder))
    }

    /// Export the mix as it is now on another thread. `poll` reports the
    /// result as a notice. Later edits do not change the exported audio.
    pub fn start_export(&mut self, path: &Path) -> Result<(), String> {
        let (master, fx) = (self.project().master, self.project().fx);
        let target = path.to_path_buf();
        self.start_export_job("Cannot export mix", move |mix| {
            mix.write_wav(&target, &master, &fx)
                .map(|()| format!("Exported {}.", target.display()))
        })?;
        self.message(format!("Exporting {}.", path.display()));
        Ok(())
    }

    /// Export each track and each FX return as its own WAV file in `folder`
    /// on another thread, as `export_tracks` does.
    pub fn start_track_export(&mut self, folder: &Path) -> Result<(), String> {
        let (title, fx) = (self.project().title.clone(), self.project().fx);
        let target = folder.to_path_buf();
        self.start_export_job("Cannot export tracks", move |mix| {
            mix.write_stems(&target, &title, &fx)
                .map(|stems| tracks_exported(stems, &target))
        })?;
        self.message(format!("Exporting tracks to {}.", folder.display()));
        Ok(())
    }

    /// Run `write` on the mix as it is now on the export thread. One export
    /// runs at a time.
    fn start_export_job(
        &mut self,
        failure: &'static str,
        write: impl FnOnce(&audio::Mix) -> Result<String, String> + Send + 'static,
    ) -> Result<(), String> {
        if self.export.is_some() {
            return Err("Wait for the current export to finish.".into());
        }
        let rendered = self.render.clone().filter(|mix| {
            !self.render_dirty
                && mix.has_levels(self.project())
                && mix.quality() == crate::playback_quality::PlaybackQuality::FULL
        });
        let project = self.project().clone();
        let library = self.library.clone();
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("mixjam-techno-export".into())
            .spawn(move || {
                let result = match rendered {
                    Some(mix) => write(&mix),
                    None => audio::prepare_project(&project, &library).and_then(|mix| write(&mix)),
                };
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Cannot start the export: {error}"))?;
        self.export = Some(Export {
            failure,
            receiver,
            thread,
        });
        Ok(())
    }

    pub fn is_exporting(&self) -> bool {
        self.export.is_some()
    }

    pub(super) fn poll_export(&mut self) -> Option<u64> {
        let export = self.export.as_ref()?;
        let result = match export.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return Some(100),
            Err(mpsc::TryRecvError::Disconnected) => Err("The export stopped unexpectedly.".into()),
        };
        let export = self.export.take()?;
        let _ = export.thread.join();
        match result {
            Ok(status) => self.message(status),
            Err(error) => self.error(format!("{}: {error}", export.failure)),
        }
        None
    }
    pub fn new_mix(&mut self) -> Result<(), String> {
        self.require_idle_recording()?;
        self.stop();
        self.cancel_stretch();
        self.replace_project(Project {
            libraries: self.library.bindings().to_vec(),
            imported_samples: self.library.imported_samples().into(),
            ..Project::default()
        });
        self.file = None;
        self.unsaved = false;
        Ok(())
    }
}

/// The status line of a finished track export.
fn tracks_exported(stems: audio::Stems, folder: &Path) -> String {
    format!("Exported {stems} to {}.", folder.display())
}
