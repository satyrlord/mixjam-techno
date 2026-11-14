pub(crate) use crate::catalog::decode_1252;
mod import;
mod naming;
mod pcm;
mod providers;
mod scan;
#[cfg(test)]
mod tests;
mod wav_folders;
pub(crate) use naming::compact_name;
#[cfg(test)]
pub(crate) use naming::packed_folder;
pub use pcm::read_packed_file;
pub(crate) use scan::record_wav_conflict;
pub use scan::{ScanCache, ScanProgress, WavProbe, check_library_folders};

use crate::products::{IMPORTED_ID_START, LibraryBinding, Product};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// The most library folders that one scan merges into the Sounds panel.
pub const MAX_LIBRARY_FOLDERS: usize = 9;

#[derive(Clone, Debug)]
pub struct Sample {
    pub id: u32,
    pub code: String,
    pub name: String,
    pub bars: u32,
    pub group_name: String,
    pub offset: u64,
    pub length: u64,
    pub frame_count: u32,
    pub sample_rate: u32,
    pub source_bpm: f64,
    pub channels: u16,
    pub wav_path: Option<PathBuf>,
    /// [`crate::catalog::pcm_identity`] of a decoded mono 44.1 kHz WAV with
    /// at least three frames; `None` when native PCM comparison is unavailable.
    pub digest: Option<[u8; 16]>,
}

#[derive(Clone, Debug)]
pub struct Library {
    pub samples: Vec<Sample>,
    pub archive_path: PathBuf,
    by_id: HashMap<u32, usize>,
    sources: HashMap<u32, AudioSource>,
    bindings: Vec<LibraryBinding>,
    /// The library folders of the last folder scan, in order, including the
    /// ones in `unloaded_roots`.
    wav_roots: Vec<PathBuf>,
    /// Library folders that the scan could not load, with the reason.
    unloaded_roots: Vec<(PathBuf, String)>,
    /// Native identities whose WAV audio matched the catalog digest.
    verified: HashSet<u32>,
    /// Native identities that only a WAV with the right name and duration but
    /// different audio could serve. Songs report these instead of guessing.
    unverified: HashMap<u32, PathBuf>,
    /// Folders whose original audio or durations establish tempo: folder, BPM, count.
    folder_tempos: Vec<(PathBuf, f64, usize)>,
    /// WAVs in the scanned folder that could not be read, with the reason.
    skipped_waves: Vec<(PathBuf, String)>,
    /// WAVs in a later library folder whose audio is identical to a native
    /// identity that an earlier folder already serves. They are not listed as
    /// separate sounds.
    duplicate_copies: Vec<PathBuf>,
    reserved_import_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum AudioSource {
    Archive {
        path: PathBuf,
    },
    T1Packed {
        path: PathBuf,
    },
    Wave {
        path: PathBuf,
        origin: Option<Product>,
    },
}

/// Where a sound's audio is read from, with its file's size and last change
/// when the key was taken. Two sounds with equal keys read the same audio, in
/// any catalog.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AudioKey {
    source: AudioSource,
    offset: u64,
    length: u64,
    frame_count: u32,
    file_len: u64,
    modified: Option<std::time::SystemTime>,
}

impl Library {
    /// The key of sound `id`'s audio as its file is now. Reads the file's
    /// metadata, never its audio. `None` when the sound has no source or its
    /// file cannot be read.
    pub(crate) fn audio_key(&self, id: u32) -> Option<AudioKey> {
        let sample = self.sample_by_id(id)?;
        let source = self.sources.get(&id)?;
        let (AudioSource::Archive { path }
        | AudioSource::T1Packed { path }
        | AudioSource::Wave { path, .. }) = source;
        let metadata = fs::metadata(path).ok()?;
        Some(AudioKey {
            source: source.clone(),
            offset: sample.offset,
            length: sample.length,
            frame_count: sample.frame_count,
            file_len: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture(samples: Vec<Sample>) -> Self {
        let sources = samples_sources(&samples);
        let mut library = Self {
            samples,
            sources,
            ..Self::empty()
        };
        library.reindex();
        library
    }

    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            archive_path: PathBuf::new(),
            by_id: HashMap::new(),
            sources: HashMap::new(),
            bindings: Vec::new(),
            wav_roots: Vec::new(),
            unloaded_roots: Vec::new(),
            verified: HashSet::new(),
            unverified: HashMap::new(),
            folder_tempos: Vec::new(),
            skipped_waves: Vec::new(),
            duplicate_copies: Vec::new(),
            reserved_import_id: IMPORTED_ID_START - 1,
        }
    }

    /// WAVs left out of the sounds because their audio is identical to a
    /// native identity that an earlier library folder already serves.
    pub fn duplicate_copies(&self) -> &[PathBuf] {
        &self.duplicate_copies
    }

    /// Every known WAV path, including exact copies omitted from the palette.
    /// Song reference resolution must use the complete source inventory.
    pub(crate) fn wav_paths(&self) -> impl Iterator<Item = &Path> {
        self.samples
            .iter()
            .filter_map(|sample| sample.wav_path.as_deref())
            .chain(self.duplicate_copies.iter().map(PathBuf::as_path))
    }

    /// Native identities whose WAV audio matched the catalog digest.
    pub fn verified_count(&self) -> usize {
        self.verified.len()
    }

    /// Folders whose original audio or durations establish tempo: folder, BPM, count.
    pub fn folder_tempos(&self) -> &[(PathBuf, f64, usize)] {
        &self.folder_tempos
    }

    /// WAVs in the scanned folder that could not be read, with the reason.
    pub fn skipped_waves(&self) -> &[(PathBuf, String)] {
        &self.skipped_waves
    }

    /// WAVs that carry a native sample's name and duration but different audio.
    pub fn unverified_waves(&self) -> impl Iterator<Item = (u32, &Path)> {
        let mut waves: Vec<_> = self
            .unverified
            .iter()
            .map(|(id, path)| (*id, path.as_path()))
            .collect();
        waves.sort();
        waves.into_iter()
    }

    /// Explain an unresolved native identity in a song import error.
    pub fn describe_missing(&self, product: Product, native_id: u32) -> String {
        let label = product.label();
        let installed = self.has_original_provider(product);
        match product
            .global_sample_id(native_id)
            .ok()
            .and_then(|id| self.unverified.get(&id))
        {
            Some(path) => format!(
                "{label} native sample {native_id} is missing from its library: {} has its name and duration, but its audio differs from the original sample.",
                path.display()
            ),
            None if !installed && crate::catalog::identity(product, native_id).is_none() => {
                format!(
                    "No loaded {label} sample catalog lists native sample {native_id}, so no WAV can be identified as it."
                )
            }
            None => format!("{label} native sample {native_id} is missing from its library."),
        }
    }

    pub fn bindings(&self) -> &[LibraryBinding] {
        &self.bindings
    }

    /// The library folders of the last folder scan, in order. It includes
    /// the folders in [`Self::unloaded_roots`].
    pub fn wav_roots(&self) -> &[PathBuf] {
        &self.wav_roots
    }

    /// Library folders that the last folder scan could not load, with the reason.
    pub fn unloaded_roots(&self) -> &[(PathBuf, String)] {
        &self.unloaded_roots
    }

    /// The library folder that holds `path`, if any.
    pub fn wav_root_of(&self, path: &Path) -> Option<&Path> {
        self.wav_roots
            .iter()
            .find(|root| path.starts_with(root))
            .map(PathBuf::as_path)
    }

    pub fn is_wav_product(&self, product: Product) -> bool {
        self.sources.iter().any(|(id, source)| {
            Product::sample_key(*id).is_some_and(|(owner, _)| owner == product)
                && matches!(source, AudioSource::Wave { .. })
        })
    }

    /// Whether this product has audio from an original packed provider.
    /// Supplemental WAV identities do not change the provider's kind.
    pub fn has_original_provider(&self, product: Product) -> bool {
        self.sources.iter().any(|(id, source)| {
            Product::sample_key(*id).is_some_and(|(owner, _)| owner == product)
                && matches!(
                    source,
                    AudioSource::Archive { .. } | AudioSource::T1Packed { .. }
                )
        })
    }

    pub fn imported_samples(&self) -> Vec<crate::engine::ImportedSample> {
        self.samples
            .iter()
            .filter_map(|sample| {
                if sample.id < IMPORTED_ID_START {
                    return None;
                }
                let AudioSource::Wave { path, origin } = self.sources.get(&sample.id)? else {
                    return None;
                };
                Some(crate::engine::ImportedSample {
                    id: sample.id,
                    name: sample.name.clone(),
                    group_name: sample.group_name.clone(),
                    path: path.clone(),
                    source_bpm: sample.source_bpm,
                    source_sample_rate: Some(sample.sample_rate),
                    origin: *origin,
                })
            })
            .collect()
    }

    /// Keep unresolved project identities out of later file and palette imports.
    pub(crate) fn reserve_missing_samples(&mut self, samples: &[crate::engine::MissingSample]) {
        self.reserved_import_id = Self::last_missing_id(samples);
    }

    /// Whether `reserve_missing_samples` would change nothing.
    pub(crate) fn has_reserved_missing_samples(
        &self,
        samples: &[crate::engine::MissingSample],
    ) -> bool {
        self.reserved_import_id == Self::last_missing_id(samples)
    }

    fn last_missing_id(samples: &[crate::engine::MissingSample]) -> u32 {
        samples
            .iter()
            .map(|sample| sample.id)
            .max()
            .unwrap_or(0)
            .max(IMPORTED_ID_START - 1)
    }

    pub fn sample_by_id(&self, id: u32) -> Option<&Sample> {
        self.by_id
            .get(&id)
            .and_then(|index| self.samples.get(*index))
    }
}

#[cfg(test)]
fn samples_sources(samples: &[Sample]) -> HashMap<u32, AudioSource> {
    samples
        .iter()
        .filter_map(|sample| {
            sample.wav_path.as_ref().map(|path| {
                (
                    sample.id,
                    AudioSource::Wave {
                        path: path.clone(),
                        origin: None,
                    },
                )
            })
        })
        .collect()
}
