//! Library folder scans that run on their own thread, and the catalog reloads that start them.
use super::Session;
use crate::library::Library;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

/// A library folder scan running on its own thread. The result replaces the
/// library only when it arrives through `poll`; dropping the job abandons it.
pub(super) struct WaveScan {
    pub(super) roots: Vec<PathBuf>,
    pub(super) source_bpm: f64,
    pub(super) progress: Arc<crate::library::ScanProgress>,
    pub(super) receiver: mpsc::Receiver<Result<Library, String>>,
}

/// What a frontend may show about a running library folder scan.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveScanStatus {
    pub roots: Vec<PathBuf>,
    pub files_done: usize,
    pub files_total: usize,
}

impl Session {
    /// Start scanning 1 to 9 library folders on another thread. A scan
    /// already running is abandoned. The library changes only when `poll`
    /// receives the result; `wave_scan` reports progress until then.
    pub fn start_wave_scan(&mut self, roots: &[PathBuf], source_bpm: f64) -> Result<(), String> {
        if self.is_recording() {
            return Err("Stop recording before you change the sample folder.".into());
        }
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("The source tempo must be finite and positive.".into());
        }
        crate::library::check_library_folders(roots)?;
        if let Some(path) = self.abandon_wave_scan() {
            self.error(format!(
                "{} was not opened because another folder was selected.",
                path.display()
            ));
        }
        // A scan identifies sounds by the catalogs: read changed ones first.
        if let Some(problems) = crate::catalog::refresh() {
            self.catalog_rescan_needed = true;
            self.report_catalog_reload(problems);
        }
        let progress = Arc::new(crate::library::ScanProgress::default());
        let (sender, receiver) = mpsc::channel();
        let worker_progress = Arc::clone(&progress);
        let worker_roots = roots.to_vec();
        let settings_root = self.settings_root.clone();
        std::thread::Builder::new()
            .name("mixjam-techno-scan".into())
            .spawn(move || {
                let result = crate::project_io::open_wav_folders_cached(
                    &settings_root,
                    &worker_roots,
                    source_bpm,
                    Some(&worker_progress),
                );
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Cannot start the sample folder scan: {error}"))?;
        self.wave_scan = Some(WaveScan {
            roots: roots.to_vec(),
            source_bpm,
            progress,
            receiver,
        });
        Ok(())
    }

    pub fn wave_scan(&self) -> Option<WaveScanStatus> {
        self.wave_scan.as_ref().map(|scan| {
            let (files_done, files_total) = scan.progress.counts();
            WaveScanStatus {
                roots: scan.roots.clone(),
                files_done,
                files_total,
            }
        })
    }

    /// Abandon a running scan. Its thread stops at the next file; its result
    /// is never adopted, and a song waiting for it is not opened.
    pub fn cancel_wave_scan(&mut self) {
        if self.wave_scan.is_none() {
            return;
        }
        match self.abandon_wave_scan() {
            Some(path) => self.message(format!(
                "Sample folder scan cancelled. {} was not opened.",
                path.display()
            )),
            None => self.message("Sample folder scan cancelled."),
        }
    }

    /// Stop the running scan and return the song that was waiting for it.
    fn abandon_wave_scan(&mut self) -> Option<PathBuf> {
        if let Some(scan) = self.wave_scan.take() {
            scan.progress.cancel();
        }
        self.pending_open.take().map(|(path, _, _)| path)
    }

    /// Report a successful folder adoption once, so views can discard stale IDs.
    pub fn take_wave_folder_adopted(&mut self) -> bool {
        std::mem::take(&mut self.wave_folder_adopted)
    }

    /// The song opened by the last completed scan, reported once.
    pub fn take_opened_after_scan(&mut self) -> Option<PathBuf> {
        self.opened_after_scan.take()
    }

    /// Read the sample catalogs again if a catalog was added, changed or
    /// removed since they were read, and rescan the saved sample folders so
    /// that a new catalog identifies their sounds. Returns whether a rescan
    /// started; a song to open should then wait for it ([`Self::open_after_scan`]).
    pub fn refresh_catalogs(&mut self) -> bool {
        // A rejected open must not consume the catalog change.
        if self.is_recording() {
            return false;
        }
        let changed = if let Some(problems) = crate::catalog::refresh() {
            self.report_catalog_reload(problems);
            true
        } else {
            false
        };
        self.rescan_changed_catalogs(changed)
    }

    pub(super) fn rescan_changed_catalogs(&mut self, changed: bool) -> bool {
        self.catalog_rescan_needed |= changed;
        if self.wave_scan.is_some() && !changed {
            return true;
        }
        if !self.catalog_rescan_needed {
            return false;
        }
        let config = if let Some(scan) = &self.wave_scan {
            crate::project_io::WaveFolderConfig {
                roots: scan.roots.clone(),
                source_bpm: scan.source_bpm,
            }
        } else {
            match crate::project_io::load_wave_config_in(&self.settings_root) {
                Ok(Some(config)) => config,
                Ok(None) => return false,
                Err(error) => {
                    self.error(error);
                    return false;
                }
            }
        };
        match self.start_wave_scan(&config.roots, config.source_bpm) {
            Ok(()) => true,
            Err(error) => {
                self.error(error);
                false
            }
        }
    }

    fn report_catalog_reload(&mut self, problems: &[(String, String)]) {
        let mut text = String::from("The sample catalogs changed and were read again.");
        for (file, error) in problems {
            text.push_str(&format!(" Sample catalog {file} was not loaded: {error}"));
        }
        self.notice = Some((text, !problems.is_empty()));
    }

    /// Open this song after the running folder scan completes, or now when
    /// no scan is running.
    pub fn open_after_scan(&mut self, path: &Path) -> Result<(), String> {
        if self.wave_scan.is_some() {
            self.pending_open = Some((path.to_path_buf(), self.revision, self.editor.generation()));
            Ok(())
        } else {
            self.open(path)
        }
    }

    pub(super) fn poll_wave_scan(&mut self) -> Option<u64> {
        let scan = self.wave_scan.as_ref()?;
        if self.is_recording() {
            // Adopting the folder stops the transport; let the take finish first.
            return Some(100);
        }
        let result = match scan.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return Some(100),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The sample folder scan stopped unexpectedly.".into())
            }
        };
        let scan = self.wave_scan.take()?;
        let pending = self.pending_open.take();
        let project_changed = pending.as_ref().is_some_and(|(_, revision, generation)| {
            // Pedalboard and FX bus edits change editor history without
            // invalidating the dry mix.
            *revision != self.revision
                || *generation != self.editor.generation()
                || self.has_recorded_take()
        });
        match result
            .and_then(|library| self.adopt_wave_folder(library, &scan.roots, scan.source_bpm))
        {
            Ok(warnings) => {
                self.catalog_rescan_needed = false;
                let count = self.library.samples.len();
                let mut notice = format!("Sample libraries loaded: {count} sounds.");
                let mut error = !warnings.is_empty();
                if !warnings.is_empty() {
                    notice.push_str(&format!(" {}", warnings.join(" ")));
                }
                if let Some((path, _, _)) = pending {
                    if project_changed {
                        notice.push_str(&format!(
                            " {} was not opened because the current mix changed or has an unsaved \
                             take.",
                            path.display()
                        ));
                    } else {
                        match self.open(&path) {
                            Ok(()) => self.opened_after_scan = Some(path),
                            Err(cause) => {
                                error = true;
                                notice
                                    .push_str(&format!(" Cannot open {}: {cause}", path.display()));
                            }
                        }
                    }
                }
                self.notice = Some((notice, error));
            }
            Err(error) => match pending {
                Some((path, _, _)) => {
                    self.error(format!("{error} {} was not opened.", path.display()))
                }
                None => self.error(error),
            },
        }
        None
    }
}
