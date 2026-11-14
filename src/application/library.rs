//! The sample catalog of a session: its read-only view, the waveform peaks of its sounds,
//! and changes of library folders and providers.
use super::{PeakState, Product, Sample, Session};
use crate::library::Library;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// A read-only catalog projection. It cannot decode, register, or remove sources.
#[derive(Clone, Copy)]
pub struct LibraryView<'a> {
    pub samples: &'a [Sample],
    library: &'a Library,
}
impl<'a> LibraryView<'a> {
    pub fn sample_by_id(self, id: u32) -> Option<&'a Sample> {
        self.library.sample_by_id(id)
    }
    pub fn bindings(self) -> &'a [crate::products::LibraryBinding] {
        self.library.bindings()
    }
    /// The library folders of the last folder scan, in order.
    pub fn wav_roots(self) -> &'a [PathBuf] {
        self.library.wav_roots()
    }
    /// Library folders that the last folder scan could not load, with the reason.
    pub fn unloaded_roots(self) -> &'a [(PathBuf, String)] {
        self.library.unloaded_roots()
    }
    /// The library folder that holds `path`, if any.
    pub fn wav_root_of(self, path: &Path) -> Option<&'a Path> {
        self.library.wav_root_of(path)
    }
    pub fn folder_tempos(self) -> &'a [(PathBuf, f64, usize)] {
        self.library.folder_tempos()
    }
    pub fn skipped_waves(self) -> &'a [(PathBuf, String)] {
        self.library.skipped_waves()
    }
    pub fn source_root(self, product: Product) -> Option<&'a Path> {
        self.library.source_root(product)
    }
}

impl Session {
    pub(super) fn set_library(&mut self, library: Library) {
        self.master.reset_compressors();
        self.cancel_stretch();
        self.library = Arc::new(library);
        self.catalog_revision += 1;
    }
    /// A count that advances whenever the sample catalog is replaced. Views
    /// can keep work derived from `library()` until it changes.
    pub fn catalog_revision(&self) -> u64 {
        self.catalog_revision
    }
    /// The waveform peaks of a library sound. A sound that is not ready reads
    /// as pending; `poll` queues the sounds asked for since the last poll for
    /// a worker thread and collects the results. A new catalog keeps the
    /// tables of sounds whose audio file is unchanged.
    pub fn sample_peaks(&self, id: u32) -> PeakState {
        self.peaks
            .borrow_mut()
            .get(self.catalog_revision, id, &self.library)
    }

    /// How many sounds' peaks views have asked for in this catalog.
    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn requested_peaks(&self) -> usize {
        self.peaks.borrow().len()
    }

    /// Give a sound finished peaks, as if the worker had read its file.
    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn set_sample_peaks(&self, id: u32, peaks: Arc<super::Peaks>) {
        self.peaks
            .borrow_mut()
            .insert(self.catalog_revision, id, peaks);
    }
    pub fn library(&self) -> LibraryView<'_> {
        LibraryView {
            samples: &self.library.samples,
            library: &self.library,
        }
    }
    fn commit_library(&mut self, library: Library) -> Result<(), String> {
        let bindings = library.bindings().to_vec();
        let mut checked = self.project().clone();
        checked.libraries = bindings.clone();
        checked.validate()?;
        let persisted: Vec<_> = bindings
            .iter()
            .filter(|binding| !library.wav_roots().contains(&binding.root))
            .cloned()
            .collect();
        crate::project_io::save_library_config(&self.settings_root, &persisted)?;
        self.editor.set_libraries(bindings);
        self.set_library(library);
        self.changed();
        Ok(())
    }
    pub fn add_library(&mut self, product: Product, root: &Path) -> Result<(), String> {
        self.require_library_change_ready()?;
        let mut library = (*self.library).clone();
        library.register(product, root, &crate::project_io::reference_samples_root())?;
        self.commit_library(library)
    }
    pub fn remove_library(&mut self, product: Product) -> Result<(), String> {
        self.require_library_change_ready()?;
        if self.project().clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id).is_some_and(|(source, _)| source == product)
        }) {
            return Err(format!(
                "This mix uses {} samples. Start a new mix before you remove this library.",
                product.label()
            ));
        }
        let mut library = (*self.library).clone();
        library.remove_product(product);
        self.commit_library(library)
    }

    fn require_library_change_ready(&self) -> Result<(), String> {
        if self.is_recording() {
            return Err("Stop recording before you change the sample libraries.".into());
        }
        if self.wave_scan.is_some() {
            return Err(
                "Wait for the sample folder scan to finish before you change the sample libraries."
                    .into(),
            );
        }
        Ok(())
    }

    /// Let these products' providers replace the same products found in a
    /// sample folder. Other providers only fill products the folder lacks.
    pub fn prefer_providers(&mut self, products: &[Product]) {
        self.preferred_providers = products.to_vec();
    }

    /// Replace the library with scanned library folders, keeping the product
    /// providers already in use. Returns the warnings for the status line:
    /// library folders that did not load, unreadable WAVs, and providers that
    /// could not be kept.
    pub(super) fn adopt_wave_folder(
        &mut self,
        mut library: Library,
        roots: &[PathBuf],
        source_bpm: f64,
    ) -> Result<Vec<String>, String> {
        let mut warnings: Vec<String> = library
            .unloaded_roots()
            .iter()
            .map(|(root, error)| format!("Library {} was not loaded: {error}", root.display()))
            .collect();
        if let Some((path, error)) = library.skipped_waves().first() {
            let count = library.skipped_waves().len();
            let files = if count == 1 { "WAV" } else { "WAVs" };
            warnings.push(format!(
                "Skipped {count} unreadable {files}, listed in Libraries. First: {}: {error}",
                path.display()
            ));
        }
        for binding in self.library.bindings() {
            // Bindings at the previous library folders came from their WAVs.
            if self.library.wav_roots().contains(&binding.root) {
                continue;
            }
            let mut candidate = library.clone();
            if candidate.source_root(binding.product).is_some() {
                // A folder that merely shares some audio with this product
                // does not displace its provider.
                if !self.preferred_providers.contains(&binding.product)
                    && !library.is_sparse_wav_product(binding.product)
                {
                    continue;
                }
                candidate.remove_product(binding.product);
            }
            // The current library already holds this provider. Read it from
            // disk again only when its folder is gone, to report that.
            let kept = candidate
                .register_loaded(binding.product, &self.library)
                .and_then(|kept| {
                    if kept {
                        return Ok(());
                    }
                    candidate.register(
                        binding.product,
                        &binding.root,
                        &crate::project_io::reference_samples_root(),
                    )
                });
            match kept {
                Ok(()) => library = candidate,
                Err(error) => warnings.push(error),
            }
        }
        let (project, library) = crate::project_io::resolve(self.project().clone(), &library)?;
        crate::project_io::save_wave_config_in(&self.settings_root, roots, source_bpm)?;
        self.stop();
        // New sample sources are not an edit: a saved or untouched mix stays so.
        let unsaved = self.unsaved;
        self.set_library(library);
        // New sources are not an edit: undo and redo stay while their steps
        // still name the same sounds.
        if let Err(project) = self.editor.rebind_sources(project) {
            self.replace_project(*project);
        }
        self.changed();
        self.unsaved = unsaved;
        self.wave_folder_adopted = true;
        Ok(warnings)
    }
}
