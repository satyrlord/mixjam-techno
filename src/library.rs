pub(crate) use crate::catalog::decode_1252;
use crate::products::{IMPORTED_ID_START, LibraryBinding, Product};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

// Every WAV can become a project import when this palette is adopted.
const MAX_WAV_FOLDER_ITEMS: usize = crate::engine::MAX_IMPORTED_SAMPLES;

/// Retain a file choice once per saved reference and link every imported source.
pub(crate) fn record_wav_conflict(
    project: &mut crate::engine::Project,
    reference: &str,
    sample_id: u32,
    candidates: Vec<PathBuf>,
) {
    if candidates.len() < 2 {
        return;
    }
    let normalized = reference.replace('\\', "/").to_lowercase();
    if let Some(conflict) = project.sample_conflicts.iter_mut().find(|conflict| {
        conflict.reference.replace('\\', "/").to_lowercase() == normalized
            && conflict.candidates == candidates
    }) {
        if !conflict.sample_ids.contains(&sample_id) {
            conflict.sample_ids.push(sample_id);
            conflict.sample_ids.sort_unstable();
        }
    } else {
        project
            .sample_conflicts
            .push(crate::engine::SampleConflict {
                reference: reference.into(),
                sample_ids: vec![sample_id],
                candidates,
                selected: 0,
                confirmed: false,
                source_spans: BTreeMap::new(),
            });
    }
}

/// Progress and cancellation shared with a folder scan on another thread.
#[derive(Debug, Default)]
pub struct ScanProgress {
    files_done: AtomicUsize,
    files_total: AtomicUsize,
    cancelled: AtomicBool,
}

impl ScanProgress {
    /// Files probed so far and the total once the folder has been listed.
    pub fn counts(&self) -> (usize, usize) {
        (
            self.files_done.load(Ordering::Relaxed),
            self.files_total.load(Ordering::Relaxed),
        )
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// What decoding a WAV establishes about it. Everything else about an imported
/// sample is metadata that the file does not change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WavProbe {
    pub frame_count: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub length: u64,
    pub digest: Option<[u8; 16]>,
}

/// Probe results keyed by canonical path, size, and modification time. A
/// folder that was scanned before loads without decoding its files again.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanCache {
    entries: HashMap<String, CachedProbe>,
    #[serde(skip)]
    changed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedProbe {
    length: u64,
    modified: u128,
    probe: WavProbe,
}

impl ScanCache {
    const MAX_ENTRIES: usize = 1_000_000;

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn has_changes(&self) -> bool {
        self.changed
    }

    fn stamp(path: &Path) -> Option<(u64, u128)> {
        let metadata = fs::metadata(path).ok()?;
        let modified = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        Some((metadata.len(), modified))
    }

    /// A damaged or edited cache file must not supply what a decode would reject.
    fn get(&self, path: &Path, stamp: (u64, u128)) -> Option<&WavProbe> {
        let entry = self.entries.get(path.to_str()?)?;
        let probe = &entry.probe;
        (entry.length == stamp.0
            && entry.modified == stamp.1
            && probe.frame_count > 0
            && (1..=384_000).contains(&probe.sample_rate)
            && (1..=2).contains(&probe.channels))
        .then_some(probe)
    }

    fn insert(&mut self, path: &Path, stamp: (u64, u128), probe: WavProbe) {
        let Some(key) = path.to_str() else {
            return;
        };
        if self.entries.len() >= Self::MAX_ENTRIES && !self.entries.contains_key(key) {
            self.entries.clear();
        }
        self.changed = true;
        self.entries.insert(
            key.to_owned(),
            CachedProbe {
                length: stamp.0,
                modified: stamp.1,
                probe,
            },
        );
    }
}

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
    wav_root: Option<PathBuf>,
    /// Native identities whose WAV audio matched the catalog digest.
    verified: HashSet<u32>,
    /// Native identities that only a WAV with the right name and duration but
    /// different audio could serve. Songs report these instead of guessing.
    unverified: HashMap<u32, PathBuf>,
    /// Folders whose original audio or durations establish tempo: folder, BPM, count.
    folder_tempos: Vec<(PathBuf, f64, usize)>,
    /// WAVs in the scanned folder that could not be read, with the reason.
    skipped_waves: Vec<(PathBuf, String)>,
    reserved_import_id: u32,
}

#[derive(Clone, Debug)]
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

impl Library {
    #[cfg(test)]
    pub(crate) fn fixture(samples: Vec<Sample>) -> Self {
        let by_id = samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        let sources = samples_sources(&samples);
        Self {
            samples,
            archive_path: PathBuf::new(),
            by_id,
            sources,
            bindings: Vec::new(),
            wav_root: None,
            verified: HashSet::new(),
            unverified: HashMap::new(),
            folder_tempos: Vec::new(),
            skipped_waves: Vec::new(),
            reserved_import_id: IMPORTED_ID_START - 1,
        }
    }

    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            archive_path: PathBuf::new(),
            by_id: HashMap::new(),
            sources: HashMap::new(),
            bindings: Vec::new(),
            wav_root: None,
            verified: HashSet::new(),
            unverified: HashMap::new(),
            folder_tempos: Vec::new(),
            skipped_waves: Vec::new(),
            reserved_import_id: IMPORTED_ID_START - 1,
        }
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
        match product
            .global_sample_id(native_id)
            .ok()
            .and_then(|id| self.unverified.get(&id))
        {
            Some(path) => format!(
                "{label} native sample {native_id} is missing from its library: {} has its name and duration, but its audio differs from the original sample.",
                path.display()
            ),
            None => format!("{label} native sample {native_id} is missing from its library."),
        }
    }

    pub fn bindings(&self) -> &[LibraryBinding] {
        &self.bindings
    }

    pub fn wav_root(&self) -> Option<&Path> {
        self.wav_root.as_deref()
    }

    pub fn is_wav_product(&self, product: Product) -> bool {
        self.sources.iter().any(|(id, source)| {
            Product::sample_key(*id).is_some_and(|(owner, _)| owner == product)
                && matches!(source, AudioSource::Wave { .. })
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

    /// Probe current bytes even when an older palette entry has the same path.
    pub(crate) fn check_import(imported: &crate::engine::ImportedSample) -> Result<Sample, String> {
        let path = fs::canonicalize(&imported.path)
            .map_err(|error| format!("Cannot open imported sample {}: {error}", imported.name))?;
        imported_sample(imported, &probe_wav(&path, imported.origin)?, path)
    }

    /// Restore unused, already validated palette WAVs after a project is opened.
    /// Project identities take precedence; palette ID conflicts receive fresh IDs.
    pub fn restore_wav_palette(
        &mut self,
        palette: &Library,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        let mut paths: HashSet<_> = self
            .samples
            .iter()
            .filter_map(|sample| sample.wav_path.clone())
            .collect();
        let mut next = self
            .samples
            .iter()
            .map(|sample| sample.id)
            .filter(|id| *id >= IMPORTED_ID_START)
            .max()
            .unwrap_or(IMPORTED_ID_START - 1)
            .max(self.reserved_import_id);
        let mut additions = Vec::new();
        for sample in &palette.samples {
            if sample.id < IMPORTED_ID_START {
                continue;
            }
            let Some(AudioSource::Wave { path, origin }) = palette.sources.get(&sample.id) else {
                continue;
            };
            if !paths.insert(path.clone()) {
                continue;
            }
            next = next
                .checked_add(1)
                .ok_or("Imported sample ID limit reached.")?;
            let mut restored = sample.clone();
            restored.id = next;
            restored.code = format!("IMPORT{next}");
            let metadata = crate::engine::ImportedSample {
                id: next,
                name: restored.name.clone(),
                group_name: restored.group_name.clone(),
                path: path.clone(),
                source_bpm: restored.source_bpm,
                source_sample_rate: Some(restored.sample_rate),
                origin: *origin,
            };
            additions.push((restored, metadata));
        }
        let mut restored_metadata = Vec::with_capacity(additions.len());
        for (sample, metadata) in additions {
            self.sources.insert(
                sample.id,
                AudioSource::Wave {
                    path: metadata.path.clone(),
                    origin: metadata.origin,
                },
            );
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample);
            restored_metadata.push(metadata);
        }
        Ok(restored_metadata)
    }

    pub fn open_wav_folder(root: &Path, source_bpm: f64) -> Result<Self, String> {
        Self::open_wav_folder_with(root, source_bpm, None, None)
    }

    /// [`Self::open_wav_folder`] with progress reporting, cancellation, and a
    /// probe cache, for a scan that runs on another thread.
    pub fn open_wav_folder_with(
        root: &Path,
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        cache: Option<&mut ScanCache>,
    ) -> Result<Self, String> {
        let mut library = Self::empty();
        library.import_wav_folder_with(root, source_bpm, progress, cache)?;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        library.wav_root = Some(root.clone());
        for product in Product::ALL {
            library.register_wav_product(
                product,
                &root,
                crate::catalog::bundled_wave_metadata(product)?,
            )?;
        }
        // A display label and duration cannot distinguish different native IDs.
        // Keep ambiguous files as ordinary WAVs until their path identifies them.
        // Identities whose audio digest matched may share one file: the audio
        // is identical under each of them.
        let mut owners: HashMap<PathBuf, Vec<u32>> = HashMap::new();
        for sample in &library.samples {
            if sample.id < IMPORTED_ID_START && !library.verified.contains(&sample.id) {
                owners
                    .entry(sample.wav_path.clone().unwrap())
                    .or_default()
                    .push(sample.id);
            }
        }
        let ambiguous: HashSet<_> = owners
            .values()
            .filter(|ids| ids.len() > 1)
            .flatten()
            .copied()
            .collect();
        library
            .samples
            .retain(|sample| !ambiguous.contains(&sample.id));
        library.bindings.retain(|binding| {
            library.samples.iter().any(|sample| {
                Product::sample_key(sample.id)
                    .is_some_and(|(product, _)| product == binding.product)
            })
        });
        let native_paths: HashSet<_> = library
            .samples
            .iter()
            .filter(|sample| sample.id < IMPORTED_ID_START)
            .filter_map(|sample| sample.wav_path.clone())
            .collect();
        library.samples.retain(|sample| {
            sample.id < IMPORTED_ID_START
                || sample
                    .wav_path
                    .as_ref()
                    .is_none_or(|path| !native_paths.contains(path))
        });
        library.by_id = library
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        library
            .sources
            .retain(|id, _| library.by_id.contains_key(id));
        Ok(library)
    }

    /// Attach factual native identities to already validated WAVs in this root.
    /// The catalog audio digest identifies a file regardless of its name or
    /// folder. Identities without a digest match fall back to a native code
    /// filename, or a label, category, and duration; a file whose audio is
    /// known to differ from the original stays unidentified and is reported.
    /// A partial folder is valid. A song reports the identities that it lacks.
    pub fn register_wav_product(
        &mut self,
        product: Product,
        root: &Path,
        metadata: &[crate::catalog::WaveMetadata],
    ) -> Result<usize, String> {
        const DIGEST_SCORE: u8 = 3;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        if self.source_root(product).is_some() {
            return Err(format!(
                "{} already has a sample provider.",
                product.label()
            ));
        }
        let mut by_name: HashMap<String, Vec<&Sample>> = HashMap::new();
        let mut by_digest: HashMap<([u8; 16], u32), Vec<&Sample>> = HashMap::new();
        // Old scan caches can contain digests for other rates or tiny files.
        // Their samples cannot establish identity against native 44.1 kHz PCM.
        let comparable_digest = |sample: &Sample| {
            sample.digest.filter(|_| {
                sample.sample_rate == 44_100 && sample.channels == 1 && sample.frame_count > 2
            })
        };
        let mut seen_paths = HashSet::new();
        for sample in &self.samples {
            let Some(path) = &sample.wav_path else {
                continue;
            };
            if !path.starts_with(&root) || !seen_paths.insert(path) {
                continue;
            }
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            by_name.entry(compact_name(&name)).or_default().push(sample);
            if let Some(digest) = comparable_digest(sample) {
                by_digest
                    .entry((digest, sample.frame_count))
                    .or_default()
                    .push(sample);
            }
        }
        // A copy in another product's folder serves only when this product has none.
        let foreign = |sample: &Sample| {
            sample
                .wav_path
                .as_ref()
                .is_some_and(|path| in_foreign_product_folder(path, &root, product))
        };
        for files in by_digest.values_mut() {
            files.sort_by_cached_key(|sample| (foreign(sample), sample.wav_path.clone()));
        }
        let mut seen_ids = HashSet::new();
        let mut entries = Vec::with_capacity(metadata.len());
        for entry in metadata {
            let id = product.global_sample_id(entry.native_id)?;
            if !seen_ids.insert(id)
                || entry.bars == 0
                || entry.frame_count == 0
                || !entry.source_bpm.is_finite()
                || entry.source_bpm <= 0.0
            {
                return Err(
                    "The bundled sample identity has invalid or duplicate metadata.".into(),
                );
            }
            entries.push((
                entry,
                id,
                entry.pcm_identity().filter(|_| entry.frame_count > 2),
            ));
        }
        let adopt = |entry: &crate::catalog::WaveMetadata, candidate: &Sample, id: u32| {
            let mut sample = candidate.clone();
            sample.id = id;
            sample.code = entry.code.clone();
            sample.name = entry.name.clone();
            sample.group_name = entry.group_name.clone();
            sample.bars = entry.bars;
            sample.source_bpm = entry.source_bpm;
            sample
        };
        let mut resolved = Vec::new();
        let mut claimed = HashSet::new();
        let mut verified = HashSet::new();
        // Identical audio recorded under several identities serves each of
        // them: pair identities and files in order, then share the last file.
        type PcmIdentity = ([u8; 16], u32);
        let mut by_identity: HashMap<PcmIdentity, Vec<(&crate::catalog::WaveMetadata, u32)>> =
            HashMap::new();
        for (entry, id, digest) in &entries {
            if let Some(digest) = digest
                && by_digest.contains_key(&(*digest, entry.frame_count))
            {
                by_identity
                    .entry((*digest, entry.frame_count))
                    .or_default()
                    .push((entry, *id));
            }
        }
        for (digest, identities) in &mut by_identity {
            identities.sort_by_key(|(_, id)| *id);
            let files = &by_digest[digest];
            for (index, (entry, id)) in identities.iter().enumerate() {
                let candidate = files[index.min(files.len() - 1)];
                let path = candidate.wav_path.clone().unwrap();
                resolved.push((adopt(entry, candidate, *id), path.clone(), DIGEST_SCORE));
                verified.insert(*id);
                claimed.insert(path);
            }
        }
        let mut unverified = HashMap::new();
        for (entry, id, expected) in &entries {
            if verified.contains(id) {
                continue;
            }
            let code = entry.code.replace('\\', "/");
            let code_name = Path::new(&code)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let mut candidates: HashMap<&PathBuf, (&Sample, u8)> = HashMap::new();
            for (name, score) in [
                (compact_name(&entry.name), 1),
                (compact_name(&code_name), 2),
            ] {
                for sample in by_name.get(&name).into_iter().flatten() {
                    let path = sample.wav_path.as_ref().unwrap();
                    if claimed.contains(path) {
                        continue;
                    }
                    if score < 2 && in_foreign_product_folder(path, &root, product) {
                        continue;
                    }
                    let category_matches = score == 2
                        || canonical_group(&sample.group_name)
                            == canonical_group(&entry.group_name)
                        || sample.group_name == "Wave"
                        || product_folder_hint(&sample.group_name).is_some();
                    let measured_frames =
                        f64::from(sample.frame_count) * 44_100.0 / f64::from(sample.sample_rate);
                    if !category_matches
                        || (measured_frames - f64::from(entry.frame_count)).abs() > 4.0
                    {
                        continue;
                    }
                    if let (Some(expected), Some(actual)) = (expected, comparable_digest(sample))
                        && *expected != actual
                    {
                        unverified.entry(*id).or_insert_with(|| path.clone());
                        continue;
                    }
                    candidates.insert(path, (sample, score));
                }
            }
            let best = candidates
                .values()
                .map(|(_, score)| *score)
                .max()
                .unwrap_or(0);
            candidates.retain(|_, (_, score)| *score == best);
            if candidates.len() > 1 {
                continue;
            }
            let Some((path, (candidate, score))) = candidates.into_iter().next() else {
                continue;
            };
            resolved.push((adopt(entry, candidate, *id), path.clone(), score));
        }
        let mut path_scores: HashMap<PathBuf, (u8, usize)> = HashMap::new();
        for (_, path, score) in &resolved {
            let (best, count) = path_scores.entry(path.clone()).or_default();
            if score > best {
                *best = *score;
                *count = 1;
            } else if score == best {
                *count += 1;
            }
        }
        resolved.retain(|(_, path, score)| {
            *score == DIGEST_SCORE || path_scores.get(path) == Some(&(*score, 1))
        });
        if resolved
            .iter()
            .any(|(sample, _, _)| self.by_id.contains_key(&sample.id))
        {
            return Err("The WAV folder contains a conflicting sample identity.".into());
        }
        let count = resolved.len();
        // Audio matches were grouped in a hash map. Keep the palette in
        // catalog order, the same on every scan.
        resolved.sort_by_key(|(sample, _, _)| sample.id);
        for (sample, path, _) in resolved {
            unverified.remove(&sample.id);
            self.sources
                .insert(sample.id, AudioSource::Wave { path, origin: None });
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample);
        }
        self.verified.extend(verified);
        self.unverified.extend(unverified);
        if count > 0 {
            self.bindings.push(LibraryBinding { product, root });
        }
        Ok(count)
    }

    /// Whether the sample folder supplies under half of this product's
    /// catalog, as when it only shares some audio with another product.
    pub fn is_sparse_wav_product(&self, product: Product) -> bool {
        if !self.is_wav_product(product) {
            return false;
        }
        let found = self
            .samples
            .iter()
            .filter(|sample| Product::sample_key(sample.id).is_some_and(|(p, _)| p == product))
            .count();
        crate::catalog::bundled_wave_metadata(product)
            .is_ok_and(|entries| found * 2 < entries.len())
    }

    /// The provider folder that a song records for a product. A product read
    /// from WAV files is recorded as the folder that holds the song's `used`
    /// files, so the song reopens it without scanning the rest of a large
    /// sample folder.
    pub fn song_binding(&self, binding: &LibraryBinding, used: &HashSet<u32>) -> LibraryBinding {
        if self.source_root(binding.product) != Some(binding.root.as_path())
            || !self.is_wav_product(binding.product)
        {
            return binding.clone();
        }
        let mut folders = used
            .iter()
            .filter(|id| Product::sample_key(**id).is_some_and(|(p, _)| p == binding.product))
            .filter_map(|id| self.sample_by_id(*id)?.wav_path.as_deref()?.parent());
        let Some(first) = folders.next() else {
            return binding.clone();
        };
        let mut common = first;
        for folder in folders {
            common = common
                .ancestors()
                .find(|ancestor| folder.starts_with(ancestor))
                .unwrap_or(&binding.root);
        }
        LibraryBinding {
            product: binding.product,
            root: common.to_path_buf(),
        }
    }

    pub fn source_root(&self, product: Product) -> Option<&Path> {
        self.bindings
            .iter()
            .find(|binding| binding.product == product)
            .map(|binding| binding.root.as_path())
    }

    pub fn remove_product(&mut self, product: Product) -> Result<(), String> {
        self.samples
            .retain(|sample| Product::sample_key(sample.id).is_none_or(|(p, _)| p != product));
        self.sources
            .retain(|id, _| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.bindings.retain(|binding| binding.product != product);
        self.verified
            .retain(|id| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.unverified
            .retain(|id, _| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.by_id = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        Ok(())
    }

    pub fn register(
        &mut self,
        product: Product,
        root: &Path,
        samples_root: &Path,
    ) -> Result<(), String> {
        let root = fs::canonicalize(root)
            .map_err(|e| format!("Cannot open {} library: {e}", product.label()))?;
        if let Some(existing) = self.source_root(product) {
            if existing == root {
                return Ok(());
            }
            return Err(format!(
                "{} already has a different library folder. Remove it before you select another folder.",
                product.label()
            ));
        }
        let other = if !crate::catalog::has_native_catalog(product, &root) {
            let mut wav = Self::empty();
            wav.import_wav_folder(&root, 140.0)?;
            if wav.register_wav_product(
                product,
                &root,
                crate::catalog::bundled_wave_metadata(product)?,
            )? == 0
            {
                return Err(format!(
                    "The WAV folder has no identified {} samples.",
                    product.label()
                ));
            }
            wav.samples.retain(|sample| sample.id < IMPORTED_ID_START);
            wav.sources.retain(|id, _| *id < IMPORTED_ID_START);
            wav.by_id = wav
                .samples
                .iter()
                .enumerate()
                .map(|(index, sample)| (sample.id, index))
                .collect();
            wav
        } else {
            match product {
                Product::T2 => {
                    Self::open_archive(product, &root, samples_root, "\x72ave20", 140.0)?
                }
                Product::T1 => Self::open_t1(&root)?,
                _ => Self::from_catalog(product, &root)?,
            }
        };
        self.add_provider(other)
    }

    /// Add one provider's samples. Conflicting identities leave this library unchanged.
    fn add_provider(&mut self, other: Self) -> Result<(), String> {
        if other
            .samples
            .iter()
            .any(|sample| self.by_id.contains_key(&sample.id))
        {
            return Err("The library contains a conflicting sample identity.".into());
        }
        for sample in other.samples {
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample);
        }
        self.sources.extend(other.sources);
        self.bindings.extend(other.bindings);
        self.verified.extend(other.verified);
        self.unverified.extend(other.unverified);
        if self.archive_path.as_os_str().is_empty() {
            self.archive_path = other.archive_path;
        }
        Ok(())
    }

    /// Take a product's provider from a library that already loaded it, in
    /// place of reading its catalog or decoding its WAV folder again.
    /// Returns false when `loaded` has no usable provider for the product.
    pub(crate) fn register_loaded(
        &mut self,
        product: Product,
        loaded: &Self,
    ) -> Result<bool, String> {
        let Some(binding) = loaded
            .bindings
            .iter()
            .find(|binding| binding.product == product && binding.root.is_dir())
        else {
            return Ok(false);
        };
        if self.source_root(product).is_some() {
            return Err(format!(
                "{} already has a sample provider.",
                product.label()
            ));
        }
        let owned = |id: u32| Product::sample_key(id).is_some_and(|(owner, _)| owner == product);
        let mut provider = Self::empty();
        provider.samples = loaded
            .samples
            .iter()
            .filter(|sample| owned(sample.id))
            .cloned()
            .collect();
        provider.sources = loaded
            .sources
            .iter()
            .filter(|(id, _)| owned(**id))
            .map(|(id, source)| (*id, source.clone()))
            .collect();
        provider.bindings = vec![binding.clone()];
        provider.verified = loaded
            .verified
            .iter()
            .copied()
            .filter(|id| owned(*id))
            .collect();
        provider.unverified = loaded
            .unverified
            .iter()
            .filter(|(id, _)| owned(**id))
            .map(|(id, path)| (*id, path.clone()))
            .collect();
        if loaded.archive_path.starts_with(&binding.root) {
            provider.archive_path = loaded.archive_path.clone();
        }
        self.add_provider(provider)?;
        Ok(true)
    }

    fn from_catalog(product: Product, root: &Path) -> Result<Self, String> {
        let mut library = Self::empty();
        for native in crate::catalog::load(product, root)? {
            let id = product.global_sample_id(native.native_id)?;
            let (source, offset, length) = match native.audio {
                crate::catalog::NativeAudio::Archive {
                    path,
                    offset,
                    length,
                } => {
                    if library.archive_path.as_os_str().is_empty() {
                        library.archive_path = path.clone();
                    }
                    (AudioSource::Archive { path }, offset, length)
                }
                crate::catalog::NativeAudio::Packed { path } => {
                    let length = fs::metadata(&path).map_err(|e| e.to_string())?.len();
                    (AudioSource::T1Packed { path }, 0, length)
                }
            };
            let sample = Sample {
                id,
                code: native.code,
                name: native.name,
                bars: native.bars,
                group_name: native.group_name.trim().to_owned(),
                offset,
                length,
                frame_count: native.frame_count,
                sample_rate: 44100,
                source_bpm: native.source_bpm,
                channels: 1,
                wav_path: None,
                digest: None,
            };
            library.sources.insert(id, source);
            library.by_id.insert(id, library.samples.len());
            library.samples.push(sample);
        }
        library.bindings.push(LibraryBinding {
            product,
            root: root.to_path_buf(),
        });
        Ok(library)
    }

    pub fn open_t1(root: &Path) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let native = legacy_format::library::Library::load(&root).map_err(|e| e.to_string())?;
        let mut library = Self::empty();
        for metadata in native.samples() {
            let path = root.join("\x50XD").join(&metadata.path);
            let mut file = fs::File::open(&path)
                .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            let mut prefix =
                vec![0; file.metadata().map_err(|e| e.to_string())?.len().min(512) as usize];
            file.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            let header = legacy_format::packed::parse_header(&prefix).map_err(|e| e.to_string())?;
            let id = Product::T1.global_sample_id(u32::from(metadata.id))?;
            let sample = Sample {
                id,
                code: metadata.path.clone(),
                name: metadata.labels.join(" ").trim().into(),
                bars: metadata.nominal_bars.unwrap_or(1).max(1),
                group_name: canonical_group(&metadata.category),
                offset: 0,
                length: file.metadata().map_err(|e| e.to_string())?.len(),
                frame_count: u32::try_from(header.decoded_samples)
                    .map_err(|_| "The native sample is too long.")?,
                sample_rate: 44100,
                source_bpm: 180.0,
                channels: 1,
                wav_path: None,
                digest: None,
            };
            library.sources.insert(id, AudioSource::T1Packed { path });
            library.by_id.insert(id, library.samples.len());
            library.samples.push(sample);
        }
        library.bindings.push(LibraryBinding {
            product: Product::T1,
            root,
        });
        Ok(library)
    }

    fn open_archive(
        product: Product,
        root: &Path,
        samples_root: &Path,
        archive_name: &str,
        source_bpm: f64,
    ) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let packed = root.join("\x65Jay").join("\x50XD");
        let archive_path = packed.join(archive_name);
        let index = fs::read(packed.join(format!("{archive_name}.inf")))
            .map_err(|e| format!("Cannot read the original sample index: {e}"))?;
        let text = decode_1252(&index);
        let lines: Vec<_> = text.lines().collect();
        let start = lines
            .iter()
            .position(|line| *line == "[SAMPLES]")
            .ok_or("The sample index has no SAMPLES section.")?
            + 1;
        if (lines.len() - start) % 12 != 0 {
            return Err("The sample index has an incomplete record.".into());
        }
        let mut archive = fs::File::open(&archive_path)
            .map_err(|e| format!("Cannot read the original sample archive: {e}"))?;
        let archive_size = archive.metadata().map_err(|e| e.to_string())?.len();
        let mut endpoint = 0u64;
        let mut samples = Vec::new();
        let mut by_id = HashMap::new();
        let mut embedded_names = Vec::new();
        for row in lines[start..].as_chunks::<12>().0 {
            let number = |i: usize| {
                row[i]
                    .parse::<u64>()
                    .map_err(|_| "The sample index has an invalid number.".to_string())
            };
            let id = product.global_sample_id(
                u32::try_from(number(0)?).map_err(|_| "The sample ID is too large.")?,
            )?;
            let offset = number(3)?;
            let length = number(4)?;
            if !matches!(number(1)?, 3 | 6) || offset != endpoint || length < 16 {
                return Err(format!("Sample {id} has an invalid archive range or type."));
            }
            endpoint = offset
                .checked_add(length)
                .filter(|end| *end <= archive_size)
                .ok_or("A sample range exceeds the archive.")?;
            archive
                .seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            let mut prefix = [0; 5];
            archive.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            if &prefix[..4] != b"t\x50xD" || prefix[4] < 2 {
                return Err(format!("Sample {id} has an invalid packed header."));
            }
            let boundary = prefix[4] as usize + 3;
            if (boundary + 9) as u64 > length {
                return Err(format!("Sample {id} has a truncated packed header."));
            }
            let mut header = vec![0; boundary + 9 - 5];
            archive.read_exact(&mut header).map_err(|e| e.to_string())?;
            let text_length = boundary - 5;
            if header[text_length..text_length + 3] != [0, 0, b'T'] {
                return Err(format!("Sample {id} has an unsupported packed payload."));
            }
            let embedded = decode_1252(&header[..text_length]);
            let fields: Vec<_> = embedded.split("\r\n").collect();
            if fields.len() != 8 {
                return Err(format!("Sample {id} has invalid packed metadata."));
            }
            let frame_count =
                u32::from_le_bytes(header[text_length + 3..text_length + 7].try_into().unwrap());
            if frame_count == 0 {
                return Err(format!("Sample {id} has no audio frames."));
            }
            let name = format!("{} {}", unquote(row[5]), unquote(row[6]))
                .trim()
                .to_string();
            embedded_names.push(format!("{} {}", fields[0], fields[1]).trim().to_string());
            let sample = Sample {
                id,
                code: unquote(row[2]).to_string(),
                name,
                bars: u32::try_from(number(7)?).map_err(|_| "The bar count is too large.")?,
                group_name: fields[4].to_string(),
                offset,
                length,
                frame_count,
                sample_rate: 44100,
                source_bpm,
                channels: 1,
                wav_path: None,
                digest: None,
            };
            let group = u8::try_from(number(8)?).map_err(|_| "The sample group is too large.")?;
            if group == 0 || sample.bars == 0 || by_id.insert(id, samples.len()).is_some() {
                return Err(format!(
                    "Sample {id} has an invalid group, bar count, or duplicate ID."
                ));
            }
            samples.push(sample);
        }
        if endpoint != archive_size || samples.is_empty() {
            return Err("The sample index does not cover the complete archive.".into());
        }
        if samples_root.join("metadata.json").exists() {
            map_waves(&mut samples, &embedded_names, samples_root)?;
        }
        let sources = samples
            .iter()
            .map(|sample| {
                (
                    sample.id,
                    AudioSource::Archive {
                        path: archive_path.clone(),
                    },
                )
            })
            .collect();
        Ok(Self {
            samples,
            archive_path,
            by_id,
            sources,
            bindings: vec![LibraryBinding { product, root }],
            wav_root: None,
            verified: HashSet::new(),
            unverified: HashMap::new(),
            folder_tempos: Vec::new(),
            skipped_waves: Vec::new(),
            reserved_import_id: IMPORTED_ID_START - 1,
        })
    }

    pub fn sample_by_id(&self, id: u32) -> Option<&Sample> {
        self.by_id
            .get(&id)
            .and_then(|index| self.samples.get(*index))
    }

    pub fn read_pcm(&self, id: u32) -> Result<crate::audio::Pcm, String> {
        let sample = self
            .sample_by_id(id)
            .ok_or("The sample is not in the library.")?;
        let source = self
            .sources
            .get(&id)
            .ok_or("The sample has no audio source.")?;
        let archive_path = match source {
            AudioSource::Wave {
                path,
                origin: Some(Product::T1),
            } => return read_t1_wave(path),
            AudioSource::Wave { path, .. } => return crate::audio::read_wav(path),
            AudioSource::T1Packed { path } => {
                if fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
                    return Err("The compressed sample is too large.".into());
                }
                let bytes = fs::read(path).map_err(|e| e.to_string())?;
                // Shared-library audio ends at a complete physical opcode, even
                // when a native header declares a longer nominal duration.
                let decoded = legacy_format::source::SampleSource::from_packed_file_audio(&bytes)
                    .map_err(|e| e.to_string())?;
                return Ok(crate::audio::Pcm {
                    sample_rate: 44100,
                    channels: 1,
                    samples: decoded
                        .samples()
                        .iter()
                        .map(|s| *s as f32 / 32768.0)
                        .collect(),
                });
            }
            AudioSource::Archive { path } => path,
        };
        if sample.length > 64 * 1024 * 1024 {
            return Err("The compressed sample is too large.".into());
        }
        let mut archive = fs::File::open(archive_path)
            .map_err(|e| format!("Cannot read the original sample archive: {e}"))?;
        archive
            .seek(SeekFrom::Start(sample.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; sample.length as usize];
        archive.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        let decoded = crate::packed::decode_record(&bytes)?;
        if decoded.samples.len() != sample.frame_count as usize {
            return Err("The decoded sample length does not match the catalog.".into());
        }
        Ok(crate::audio::Pcm {
            sample_rate: decoded.sample_rate,
            channels: 1,
            samples: decoded
                .samples
                .into_iter()
                .map(|sample| sample as f32 / 32768.0)
                .collect(),
        })
    }

    pub fn step_synth_sources(&self) -> Result<Vec<(PathBuf, String)>, String> {
        if self.wav_root.is_some() {
            return Ok(self
                .samples
                .iter()
                .filter_map(|sample| {
                    sample
                        .wav_path
                        .as_ref()
                        .map(|path| (path.clone(), sample.name.clone()))
                })
                .collect());
        }
        let packed = self
            .archive_path
            .parent()
            .ok_or("The original archive path is invalid.")?;
        let installation = packed
            .parent()
            .ok_or("The original archive path is invalid.")?;
        let text = decode_1252(
            &fs::read(installation.join("\x65jay").join("\x48ypsound"))
                .map_err(|e| e.to_string())?,
        );
        let mut lines: Vec<_> = text.lines().collect();
        while lines.last() == Some(&"") {
            lines.pop();
        }
        if !lines.len().is_multiple_of(3) {
            return Err("The generator source index has an incomplete record.".into());
        }
        let mut sources = Vec::new();
        for row in lines.as_chunks::<3>().0 {
            let relative = Path::new(row[0]);
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err("The generator source index contains an invalid path.".into());
            }
            let path = packed.join(relative);
            if !path.is_file() {
                return Err(format!("Generator source {} is missing.", path.display()));
            }
            sources.push((path, row[1].trim().to_string()));
        }
        Ok(sources)
    }

    /// Load a WAV-only folder without an original installation or a sidecar index.
    /// The caller supplies the tempo: a WAV file does not establish musical BPM.
    /// Keep the returned records in the project when these samples are used.
    pub fn import_wav_folder(
        &mut self,
        root: &Path,
        source_bpm: f64,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        self.import_wav_folder_with(root, source_bpm, None, None)
    }

    /// Import every WAV below `root`, decoding files on several threads.
    /// `progress` reports the file count and lets another thread cancel the
    /// scan; `cache` skips files whose size and modification time it knows.
    /// Any unreadable file or a cancellation leaves this library unchanged.
    pub fn import_wav_folder_with(
        &mut self,
        root: &Path,
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        cache: Option<&mut ScanCache>,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("The WAV folder needs a finite positive source tempo.".into());
        }
        let root =
            fs::canonicalize(root).map_err(|e| format!("Cannot open the WAV folder: {e}"))?;
        if !root.is_dir() {
            return Err("Select a folder that contains WAV samples.".into());
        }
        let cancelled = || progress.is_some_and(ScanProgress::is_cancelled);
        let mut pending = vec![(root.clone(), 0usize)];
        let mut visited = HashSet::new();
        let mut paths = Vec::new();
        let mut stated = BTreeMap::new();
        let mut entries_seen = 0usize;
        while let Some((folder, depth)) = pending.pop() {
            if !visited.insert(folder.clone()) {
                continue;
            }
            if depth > 64 {
                return Err("The WAV folder exceeds the 64-level folder limit.".into());
            }
            if cancelled() {
                return Err(SCAN_CANCELLED.into());
            }
            for entry in fs::read_dir(&folder)
                .map_err(|e| format!("Cannot read {}: {e}", folder.display()))?
            {
                let entry = entry.map_err(|e| format!("Cannot read {}: {e}", folder.display()))?;
                entries_seen += 1;
                if entries_seen > MAX_WAV_FOLDER_ITEMS {
                    return Err("The WAV folder exceeds the 200,000-item limit.".into());
                }
                let error =
                    |e: std::io::Error| format!("Cannot read {}: {e}", entry.path().display());
                // The listing gives each entry's type. Only a link needs the
                // file system again, to find and check its target. Opening
                // every entry made a cached scan of 100,000 files 3 times slower.
                let mut kind = entry.file_type().map_err(error)?;
                let mut path = entry.path();
                if kind.is_symlink() {
                    path = fs::canonicalize(&path).map_err(error)?;
                    if !path.starts_with(&root) {
                        return Err("A WAV folder link points outside the selected folder.".into());
                    }
                    kind = fs::metadata(&path).map_err(error)?.file_type();
                }
                if kind.is_dir() {
                    pending.push((path, depth + 1));
                } else if kind.is_file()
                    && path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
                {
                    paths.push(path);
                } else if kind.is_file()
                    && path
                        .file_name()
                        .is_some_and(|name| name.eq_ignore_ascii_case("bpm.inf"))
                {
                    // Optional pack metadata: an unreadable file states nothing.
                    let tempo = fs::read(&path)
                        .ok()
                        .and_then(|bytes| stated_pack_tempo(&String::from_utf8_lossy(&bytes)));
                    if let (Some(tempo), Some(pack)) = (tempo, path.parent()) {
                        stated.insert(pack.to_path_buf(), tempo);
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        if paths.is_empty() {
            return Err("The selected folder contains no WAV samples.".into());
        }
        // An unreadable file is reported and left out; the rest of the folder loads.
        let probes = probe_all(&paths, progress, cache)?;
        let mut skipped = Vec::new();
        let (paths, probes): (Vec<_>, Vec<_>) = paths
            .into_iter()
            .zip(probes)
            .filter_map(|(path, probe)| match probe {
                Ok(probe) => Some((path, probe)),
                Err(error) => {
                    // The path is listed beside the reason; do not repeat it.
                    let prefix = format!("Cannot read {}: ", path.display());
                    let reason = error.strip_prefix(&prefix).unwrap_or(&error).to_string();
                    skipped.push((path, reason));
                    None
                }
            })
            .unzip();
        if paths.is_empty() {
            let (path, error) = &skipped[0];
            return Err(format!(
                "The selected folder contains no readable WAV samples. {}: {error}",
                path.display()
            ));
        }
        // Loop packs keep one tempo per folder, and their files are whole bars
        // long. A folder whose durations agree on a tempo uses it; the caller's
        // tempo covers the rest. Identified native samples take catalog tempo later.
        let mut by_folder: BTreeMap<PathBuf, Vec<(u64, u32)>> = BTreeMap::new();
        for (path, probe) in paths.iter().zip(&probes) {
            by_folder
                .entry(path.parent().unwrap_or(&root).to_path_buf())
                .or_default()
                .push((u64::from(probe.frame_count), probe.sample_rate));
        }
        let mut tempos: BTreeMap<PathBuf, f64> = by_folder
            .iter()
            .filter_map(|(folder, durations)| {
                Some((folder.clone(), infer_folder_tempo(durations, source_bpm)?))
            })
            .collect();
        // A pack has one tempo. Short files can put one of its folders an
        // octave away from the others; bring it back when its loops allow.
        let settled = tempos.clone();
        for (folder, tempo) in &mut tempos {
            let octave = if *tempo >= 176.0 {
                *tempo / 2.0
            } else {
                *tempo * 2.0
            };
            if (88.0..176.0).contains(tempo)
                || *tempo == source_bpm
                || !(88.0..176.0).contains(&octave)
                || !agrees_on_tempo(&by_folder[folder], octave)
            {
                continue;
            }
            if settled
                .iter()
                .any(|(sibling, other)| sibling.parent() == folder.parent() && *other == octave)
            {
                *tempo = octave;
            }
        }
        // A pack's BPM.INF states the tempo of every folder below it. Three
        // bars at 120 are four at 160, so a duration reading that is not the
        // stated tempo in some octave is replaced. A reading an octave away
        // plays the same and stays, and the pack's untimed folders take the
        // octave that most of its files settled on.
        let pack_of = |folder: &Path| {
            folder
                .ancestors()
                .take_while(|ancestor| ancestor.starts_with(&root))
                .find_map(|ancestor| stated.get_key_value(ancestor))
        };
        let mut octaves: BTreeMap<&Path, BTreeMap<u64, usize>> = BTreeMap::new();
        for (folder, durations) in &by_folder {
            if let (Some((pack, stated)), Some(tempo)) = (pack_of(folder), tempos.get(folder))
                && octave_apart(*tempo, *stated)
            {
                *octaves
                    .entry(pack)
                    .or_default()
                    .entry(tempo.to_bits())
                    .or_default() += durations.len();
            }
        }
        for folder in by_folder.keys() {
            let Some((pack, stated)) = pack_of(folder) else {
                continue;
            };
            if tempos
                .get(folder)
                .is_some_and(|tempo| octave_apart(*tempo, *stated))
            {
                continue;
            }
            let settled = octaves
                .get(pack.as_path())
                .and_then(|counts| counts.iter().max_by_key(|(_, count)| **count))
                .map_or(*stated, |(bits, _)| f64::from_bits(*bits));
            tempos.insert(folder.clone(), settled);
        }
        // Authenticated original audio resolves duration-octave ambiguity.
        // Apply this after the heuristic so sibling guesses cannot change it.
        let evidence = crate::source_tempo::SourceTempos::bundled()?;
        let mut folder_probes: BTreeMap<&Path, Vec<&WavProbe>> = BTreeMap::new();
        for (path, probe) in paths.iter().zip(&probes) {
            folder_probes
                .entry(path.parent().unwrap_or(&root))
                .or_default()
                .push(probe);
        }
        let mut authenticated = HashSet::new();
        for (folder, probes) in folder_probes {
            if let Some(tempo) = evidence.folder(probes.into_iter()) {
                tempos.insert(folder.to_path_buf(), tempo);
                authenticated.insert(folder.to_path_buf());
            }
        }
        // A tempo written in a pack's file name is stated, not guessed: it
        // overrides the duration heuristic, but not authenticated original audio.
        let file_tempos: Vec<Option<f64>> = paths
            .iter()
            .map(|path| {
                let folder = path.parent().unwrap_or(&root);
                let inferred = tempos.get(folder).copied();
                if authenticated.contains(folder) {
                    inferred
                } else {
                    named_tempo(path).or(inferred)
                }
            })
            .collect();
        // Folder, tempo bits, and file count; positive tempos order by their bits.
        let mut counted: BTreeMap<(&Path, u64), usize> = BTreeMap::new();
        for (path, tempo) in paths.iter().zip(&file_tempos) {
            if let Some(tempo) = tempo {
                *counted
                    .entry((path.parent().unwrap_or(&root), tempo.to_bits()))
                    .or_default() += 1;
            }
        }
        let folder_tempos = counted
            .into_iter()
            .map(|((folder, bits), count)| (folder.to_path_buf(), f64::from_bits(bits), count))
            .collect();
        let mut candidate = self.clone();
        let mut allocator = ImportAllocator::new(&candidate);
        let mut imported = Vec::with_capacity(paths.len());
        for ((path, probe), tempo) in paths.into_iter().zip(probes).zip(file_tempos) {
            let folder = path.parent().unwrap_or(&root);
            let tempo = tempo.unwrap_or(source_bpm);
            let group = Some(folder)
                .filter(|parent| *parent != root)
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Wave".into());
            let (id, existing) = allocator.allocate(&path)?;
            let record = crate::engine::ImportedSample {
                id,
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .trim()
                    .to_string(),
                group_name: group,
                path,
                source_bpm: tempo,
                source_sample_rate: None,
                origin: None,
            };
            imported.push(candidate.adopt_import(record, &probe, existing)?);
        }
        candidate.folder_tempos = folder_tempos;
        candidate.skipped_waves = skipped;
        *self = candidate;
        Ok(imported)
    }

    /// Store a probed import under its ID, replacing an entry at `existing`.
    fn adopt_import(
        &mut self,
        mut imported: crate::engine::ImportedSample,
        probe: &WavProbe,
        existing: Option<usize>,
    ) -> Result<crate::engine::ImportedSample, String> {
        imported.source_sample_rate = Some(probe.sample_rate);
        let sample = imported_sample(&imported, probe, imported.path.clone())?;
        self.sources.insert(
            imported.id,
            AudioSource::Wave {
                path: imported.path.clone(),
                origin: imported.origin,
            },
        );
        if let Some(index) = existing {
            self.samples[index] = sample;
        } else {
            self.by_id.insert(imported.id, self.samples.len());
            self.samples.push(sample);
        }
        Ok(imported)
    }

    /// Resolve a moved WAV by its longest matching path suffix. Ties use the
    /// first canonical path; importers retain all candidates for user review.
    pub fn resolve_wav_reference(&self, reference: &str) -> Result<PathBuf, String> {
        self.find_wav_candidates(reference)?
            .into_iter()
            .next()
            .ok_or_else(|| format!("WAV sample {reference} is missing from the selected folder."))
    }

    /// Return canonical, unique best-suffix matches in stable path order.
    pub(crate) fn find_wav_candidates(&self, reference: &str) -> Result<Vec<PathBuf>, String> {
        let normalized = reference.replace('\\', "/");
        let parts: Vec<_> = normalized
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect();
        if parts.is_empty() || parts.contains(&"..") {
            return Err("The WAV reference is empty or contains a parent folder.".into());
        }
        let mut best_score = 0;
        let mut matches = HashSet::new();
        for sample in &self.samples {
            let Some(path) = &sample.wav_path else {
                continue;
            };
            // Most samples have another file name: skip them without allocating.
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(parts[parts.len() - 1]))
            {
                continue;
            }
            let candidate = path.to_string_lossy().replace('\\', "/");
            let score = parts
                .iter()
                .rev()
                .zip(candidate.split('/').rev())
                .take_while(|(left, right)| left.eq_ignore_ascii_case(right))
                .count();
            if score == 0 || score < best_score {
                continue;
            }
            let Ok(path) = fs::canonicalize(path) else {
                continue;
            };
            if score > best_score {
                best_score = score;
                matches.clear();
            }
            matches.insert(path);
        }
        let mut matches: Vec<_> = matches.into_iter().collect();
        matches.sort();
        Ok(matches)
    }

    pub fn import_wav_with_tempo(
        &mut self,
        path: &Path,
        name: Option<&str>,
        group: &str,
        source_bpm: f64,
    ) -> Result<crate::engine::ImportedSample, String> {
        let path =
            fs::canonicalize(path).map_err(|e| format!("Cannot open the imported sample: {e}"))?;
        let (id, existing) = ImportAllocator::new(self).allocate(&path)?;
        let default_name = path.file_stem().unwrap_or_default().to_string_lossy();
        let imported = crate::engine::ImportedSample {
            id,
            name: name.unwrap_or(&default_name).trim().to_string(),
            group_name: group.trim().to_string(),
            path,
            source_bpm,
            source_sample_rate: None,
            origin: None,
        };
        let probe = probe_wav(&imported.path, imported.origin)?;
        self.adopt_import(imported, &probe, existing)
    }

    /// An import whose metadata and file path match its current catalog entry
    /// keeps that validated entry. Undo and redo of a large WAV palette must not
    /// decode every file again.
    fn unchanged_import(&self, imported: &crate::engine::ImportedSample) -> Option<&Sample> {
        crate::engine::validate_import_metadata(imported).ok()?;
        let sample = self.sample_by_id(imported.id)?;
        let AudioSource::Wave { path, origin } = self.sources.get(&imported.id)? else {
            return None;
        };
        // The catalog holds canonical paths. A project can name the same file
        // without the verbatim prefix that canonical Windows paths carry.
        let same_file = |known: &Path| {
            known == imported.path
                || known
                    .to_str()
                    .and_then(|known| known.strip_prefix(r"\\?\"))
                    .is_some_and(|known| {
                        !known.starts_with("UNC\\") && Path::new(known) == imported.path
                    })
        };
        (sample.wav_path.as_deref().is_some_and(same_file)
            && same_file(path)
            && sample.name == imported.name
            && sample.group_name == imported.group_name
            && sample.source_bpm == imported.source_bpm
            && imported
                .source_sample_rate
                .is_none_or(|rate| sample.sample_rate == rate)
            && *origin == imported.origin)
            .then_some(sample)
    }

    /// Whether the catalog already holds exactly these imports, in this order.
    /// `apply_imports` would then change nothing.
    pub(crate) fn has_imports(&self, imports: &[crate::engine::ImportedSample]) -> bool {
        self.samples
            .iter()
            .filter(|sample| sample.id >= IMPORTED_ID_START)
            .map(|sample| sample.id)
            .eq(imports.iter().map(|imported| imported.id))
            && imports
                .iter()
                .all(|imported| self.unchanged_import(imported).is_some())
    }

    /// Resolve all files before replacing the imported part of the catalog.
    /// A missing or invalid file leaves the current library unchanged.
    pub fn apply_imports(
        &mut self,
        imports: &[crate::engine::ImportedSample],
    ) -> Result<(), String> {
        self.apply_checked_imports(imports, &HashMap::new())
    }

    /// Prefer probes made by project opening over cached palette metadata.
    /// History restoration can omit them and reuse its unchanged entries.
    pub(crate) fn apply_checked_imports(
        &mut self,
        imports: &[crate::engine::ImportedSample],
        checked: &HashMap<u32, Sample>,
    ) -> Result<(), String> {
        let mut resolved = Vec::with_capacity(imports.len());
        let mut ids = HashSet::new();
        let mut sources: HashMap<_, _> = self
            .sources
            .iter()
            .filter(|(id, _)| **id < IMPORTED_ID_START)
            .map(|(id, source)| (*id, source.clone()))
            .collect();
        for imported in imports {
            if !ids.insert(imported.id) {
                return Err("The project has duplicate imported sample IDs.".into());
            }
            let sample = if let Some(sample) = checked.get(&imported.id) {
                // Resolution can change the musical tempo after probing. Build
                // the entry from the final metadata and the measured audio.
                let probe = WavProbe {
                    frame_count: sample.frame_count,
                    sample_rate: sample.sample_rate,
                    channels: sample.channels,
                    length: sample.length,
                    digest: sample.digest,
                };
                imported_sample(
                    imported,
                    &probe,
                    sample
                        .wav_path
                        .clone()
                        .ok_or("The imported sample has no path.")?,
                )?
            } else {
                match self.unchanged_import(imported) {
                    Some(sample) => sample.clone(),
                    None => {
                        let path = fs::canonicalize(&imported.path).map_err(|e| {
                            format!("Cannot open imported sample {}: {e}", imported.name)
                        })?;
                        imported_sample(imported, &probe_wav(&path, imported.origin)?, path)?
                    }
                }
            };
            sources.insert(
                imported.id,
                AudioSource::Wave {
                    path: sample
                        .wav_path
                        .clone()
                        .ok_or("The imported sample has no path.")?,
                    origin: imported.origin,
                },
            );
            resolved.push(sample);
        }
        self.samples.retain(|sample| sample.id < IMPORTED_ID_START);
        self.samples.extend(resolved);
        self.sources = sources;
        self.by_id = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        Ok(())
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

fn read_t1_wave(path: &Path) -> Result<crate::audio::Pcm, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("The native WAV source exceeds the 128 MiB limit.".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let wave = legacy_format::playback::FileSample::from_wave(PathBuf::from("import.wav"), &bytes)
        .map_err(|e| e.to_string())?;
    let samples = wave
        .samples()
        .iter()
        .map(|sample| *sample as f32 / 32768.0)
        .collect();
    Ok(crate::audio::Pcm {
        sample_rate: 44100,
        channels: 1,
        samples,
    })
}

pub fn read_packed_file(path: &Path) -> Result<crate::audio::Pcm, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("The compressed sample is too large.".into());
    }
    let decoded = crate::packed::decode_record(&fs::read(path).map_err(|e| e.to_string())?)?;
    Ok(crate::audio::Pcm {
        sample_rate: decoded.sample_rate,
        channels: 1,
        samples: decoded
            .samples
            .into_iter()
            .map(|sample| sample as f32 / 32768.0)
            .collect(),
    })
}

const SCAN_CANCELLED: &str = "The sample folder scan was cancelled.";

/// Decode one WAV at a canonical path: its length, format, and audio digest.
fn probe_wav(path: &Path, origin: Option<Product>) -> Result<WavProbe, String> {
    let length = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if length > 128 * 1024 * 1024 {
        return Err("The WAV sample exceeds the 128 MiB limit.".into());
    }
    let mut identity = crate::catalog::PcmIdentity::default();
    let (sample_rate, channels, frame_count) = if origin == Some(Product::T1) {
        let pcm = read_t1_wave(path)?;
        for &sample in &pcm.samples {
            identity.push((sample * 32768.0).round() as i16);
        }
        (pcm.sample_rate, pcm.channels, pcm.frames() as u32)
    } else {
        let (spec, frames) = crate::audio::visit_wav_samples(path, |spec, sample| {
            if spec.channels == 1 && spec.sample_rate == 44_100 {
                identity.push((sample * 32768.0).round() as i16);
            }
        })?;
        (spec.sample_rate, spec.channels, frames)
    };
    if frame_count == 0 {
        return Err("The imported sample contains no audio.".into());
    }
    let digest =
        (channels == 1 && sample_rate == 44_100 && frame_count > 2).then(|| identity.finish());
    Ok(WavProbe {
        frame_count,
        sample_rate,
        channels,
        length,
        digest,
    })
}

/// Probe every path, in parallel for the files the cache does not know.
/// Results come back in path order, one per file: an unreadable file does not
/// fail the scan. The cache keeps every probe that succeeded.
fn probe_all(
    paths: &[PathBuf],
    progress: Option<&ScanProgress>,
    mut cache: Option<&mut ScanCache>,
) -> Result<Vec<Result<WavProbe, String>>, String> {
    if let Some(progress) = progress {
        progress.files_total.store(paths.len(), Ordering::Relaxed);
        progress.files_done.store(0, Ordering::Relaxed);
    }
    // Reading each file's size and time is most of a cached scan; the file
    // system answers several requests at once.
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, 8);
    let stamps: Vec<Option<(u64, u128)>> = std::thread::scope(|scope| {
        let jobs: Vec<_> = paths
            .chunks(paths.len().div_ceil(threads).max(1))
            .map(|chunk| {
                scope.spawn(|| {
                    chunk
                        .iter()
                        .map(|p| ScanCache::stamp(p))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter()
            .flat_map(|job| {
                job.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    let mut probes: Vec<Option<Result<WavProbe, String>>> = paths
        .iter()
        .zip(&stamps)
        .map(|(path, stamp)| {
            let hit = cache
                .as_deref()
                .zip(*stamp)
                .and_then(|(cache, stamp)| cache.get(path, stamp).cloned());
            if hit.is_some()
                && let Some(progress) = progress
            {
                progress.files_done.fetch_add(1, Ordering::Relaxed);
            }
            hit.map(Ok)
        })
        .collect();
    let pending: Vec<usize> = (0..paths.len()).filter(|i| probes[*i].is_none()).collect();
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, 8)
        .min(pending.len().max(1));
    let next = AtomicUsize::new(0);
    // Each worker streams validated samples and retains only its reader buffer.
    let results = std::sync::Mutex::new(Vec::with_capacity(pending.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    if progress.is_some_and(ScanProgress::is_cancelled) {
                        return;
                    }
                    let slot = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&index) = pending.get(slot) else {
                        return;
                    };
                    let probe = probe_wav(&paths[index], None);
                    if let Some(progress) = progress {
                        progress.files_done.fetch_add(1, Ordering::Relaxed);
                    }
                    results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push((index, probe));
                }
            });
        }
    });
    for (index, probe) in results
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        if let (Some(cache), Some(stamp), Ok(probe)) = (cache.as_deref_mut(), stamps[index], &probe)
        {
            cache.insert(&paths[index], stamp, probe.clone());
        }
        probes[index] = Some(probe);
    }
    if progress.is_some_and(ScanProgress::is_cancelled) {
        return Err(SCAN_CANCELLED.into());
    }
    probes
        .into_iter()
        .map(|probe| probe.ok_or_else(|| SCAN_CANCELLED.to_string()))
        .collect()
}

/// Imported sample IDs: an existing entry keeps its ID; new files continue
/// after the highest one. One pass over the catalog serves a whole folder.
struct ImportAllocator {
    /// Catalog index and ID of each imported file already in the library.
    existing: HashMap<PathBuf, (usize, u32)>,
    last_id: u32,
}

impl ImportAllocator {
    fn new(library: &Library) -> Self {
        let mut existing = HashMap::new();
        let mut last_id = library.reserved_import_id;
        for (index, sample) in library.samples.iter().enumerate() {
            if sample.id < IMPORTED_ID_START {
                continue;
            }
            last_id = last_id.max(sample.id);
            if let Some(path) = &sample.wav_path {
                existing.insert(path.clone(), (index, sample.id));
            }
        }
        Self { existing, last_id }
    }

    fn allocate(&mut self, path: &Path) -> Result<(u32, Option<usize>), String> {
        if let Some(&(index, id)) = self.existing.get(path) {
            return Ok((id, Some(index)));
        }
        self.last_id = self
            .last_id
            .checked_add(1)
            .ok_or("Imported sample ID limit reached.")?;
        Ok((self.last_id, None))
    }
}

/// Build the catalog entry for an import whose file at the canonical `path`
/// has been probed.
fn imported_sample(
    imported: &crate::engine::ImportedSample,
    probe: &WavProbe,
    path: PathBuf,
) -> Result<Sample, String> {
    crate::engine::validate_import_metadata(imported)?;
    let seconds = probe.frame_count as f64 / probe.sample_rate as f64;
    let exact = seconds * imported.source_bpm / 240.0;
    // A loop a few frames over a whole bar is that many bars, not one more.
    let bars = if whole_bars(seconds, imported.source_bpm).is_some() {
        exact.round()
    } else {
        exact.ceil()
    };
    let bars = (bars as u32).max(1);
    Ok(Sample {
        id: imported.id,
        code: format!("IMPORT{}", imported.id),
        name: imported.name.clone(),
        bars,
        group_name: imported.group_name.clone(),
        offset: 0,
        length: probe.length,
        frame_count: probe.frame_count,
        sample_rate: probe.sample_rate,
        source_bpm: imported.source_bpm,
        channels: probe.channels,
        wav_path: Some(path),
        digest: probe.digest,
    })
}

fn unquote(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(text)
}

#[derive(Deserialize)]
struct WaveIndex {
    samples: Vec<WaveEntry>,
}
#[derive(Deserialize)]
struct WaveEntry {
    filename: String,
    alias: String,
    channel: String,
}
type WaveCandidate = (PathBuf, String, u32, u16, u32);

fn map_waves(samples: &mut [Sample], embedded_names: &[String], root: &Path) -> Result<(), String> {
    let file = fs::File::open(root.join("metadata.json")).map_err(|e| e.to_string())?;
    let index: WaveIndex = serde_json::from_reader(file)
        .map_err(|e| format!("Cannot read the WAV sample index: {e}"))?;
    let mut lookup: HashMap<(String, String), Vec<WaveCandidate>> = HashMap::new();
    for entry in index.samples {
        let relative = Path::new(&entry.filename);
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("The WAV sample index contains an invalid path.".into());
        }
        let path = root.join(relative);
        let Ok(reader) = hound::WavReader::open(&path) else {
            continue;
        };
        let spec = reader.spec();
        if spec.sample_rate == 0 || spec.channels == 0 {
            continue;
        }
        let key = (canonical_group(&entry.channel), compact_name(&entry.alias));
        lookup.entry(key).or_default().push((
            path,
            slug(&entry.alias),
            spec.sample_rate,
            spec.channels,
            reader.duration(),
        ));
    }
    let mut used_paths = HashSet::new();
    for (sample, embedded) in samples.iter_mut().zip(embedded_names) {
        let mut candidates: Vec<(&WaveCandidate, u8)> = Vec::new();
        for name in [&sample.name, embedded] {
            let key = (canonical_group(&sample.group_name), compact_name(name));
            if let Some(entries) = lookup.get(&key) {
                for entry in entries {
                    if entry.2 != 44100 || entry.3 != 1 || entry.4 != sample.frame_count {
                        continue;
                    }
                    let score = u8::from(entry.1 == slug(name));
                    if !candidates
                        .iter()
                        .any(|(candidate, _)| candidate.0 == entry.0)
                    {
                        candidates.push((entry, score));
                    }
                }
            }
        }
        let best = candidates.iter().map(|(_, score)| *score).max();
        let mut best_candidates = candidates.iter().filter(|(_, score)| Some(*score) == best);
        let selected = best_candidates.next().map(|(entry, _)| *entry);
        if best_candidates.next().is_some() {
            continue;
        }
        if let Some(entry) = selected {
            if !used_paths.insert(entry.0.clone()) {
                continue;
            }
            sample.wav_path = Some(entry.0.clone());
            sample.sample_rate = entry.2;
            sample.channels = entry.3;
        }
    }
    Ok(())
}

/// The tempo in a loop pack's BPM.INF: the `BPM` key of its `[Tempo]` section.
/// Magix Soundpools put one such file in each pack folder.
fn stated_pack_tempo(text: &str) -> Option<f64> {
    let mut section = "";
    text.lines().map(str::trim).find_map(|line| {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim();
            return None;
        }
        let (key, value) = line.split_once('=')?;
        let tempo: f64 = (section.eq_ignore_ascii_case("tempo")
            && key.trim().eq_ignore_ascii_case("bpm"))
        .then(|| value.trim().parse().ok())??;
        (40.0..=300.0).contains(&tempo).then_some(tempo)
    })
}

/// Whether two tempos are the same up to whole octaves, such as 80 and 160.
fn octave_apart(a: f64, b: f64) -> bool {
    let octaves = (a / b).log2();
    (octaves - octaves.round()).abs() < 1e-6
}

/// The tempo that a loop pack states in a file name, such as 180 in
/// `HARDCORE_BASS001_180_C_SL3.WAV`: a two- or three-digit BPM token followed by
/// a key token, which is a note name with an optional sharp, flat, or minor
/// mark, or X for a sound without a key.
fn named_tempo(path: &Path) -> Option<f64> {
    let stem = path.file_stem()?.to_str()?;
    let tokens: Vec<&str> = stem.split('_').collect();
    tokens.windows(2).find_map(|pair| {
        let (bpm, key) = (pair[0], pair[1].to_ascii_lowercase());
        let note = key
            .strip_prefix(|c: char| ('a'..='g').contains(&c))
            .is_some_and(|rest| ["", "#", "b", "m", "#m", "bm"].contains(&rest));
        let tempo: f64 =
            (bpm.len() >= 2 && bpm.len() <= 3 && bpm.bytes().all(|b| b.is_ascii_digit()))
                .then(|| bpm.parse().ok())??;
        ((note || key == "x") && (40.0..=300.0).contains(&tempo)).then_some(tempo)
    })
}

/// The number of whole bars, up to 32, a duration spans at a tempo, within
/// four frames at 44.1 kHz plus 0.05 percent (at most 5 ms) for trimmed
/// loops. Longer files are recordings, not loops, and fit no tempo.
fn whole_bars(seconds: f64, tempo: f64) -> Option<u32> {
    let bars = seconds * tempo / 240.0;
    let nearest = bars.round();
    let slack = 4.0 / 44_100.0 + (seconds / 2000.0).min(0.005);
    ((1.0..=32.0).contains(&nearest) && (bars - nearest).abs() * 240.0 / tempo <= slack)
        .then_some(nearest as u32)
}

/// Whether at least three files and three quarters of a folder are whole bars
/// at this tempo.
fn agrees_on_tempo(durations: &[(u64, u32)], tempo: f64) -> bool {
    let whole = durations
        .iter()
        .filter(|(frames, rate)| {
            *rate > 0 && whole_bars(*frames as f64 / f64::from(*rate), tempo).is_some()
        })
        .count();
    whole >= 3 && whole * 4 >= durations.len() * 3
}

/// The tempo at which a folder's WAV durations are whole bars, when at least
/// three quarters of them (and three files) agree on one. Candidates are whole
/// BPM from 60 to 200 and the fallback.
///
/// Durations alone fit several tempos: two-bar loops at 140 are one bar at 70
/// and three at 210. Tempos within five percent of the best fit stay in the
/// running, so that one odd file does not decide. Among those, tempos that give
/// 1, 2, 4, 8, or 16 bars are preferred; then the fallback itself; then the
/// octave from 88 up to 176 BPM, which holds exactly one of a tempo, its half,
/// and its double; then the tempo nearest the fallback by ratio.
pub(crate) fn infer_folder_tempo(durations: &[(u64, u32)], fallback: f64) -> Option<f64> {
    if !fallback.is_finite() || fallback <= 0.0 {
        return None;
    }
    let seconds: Vec<f64> = durations
        .iter()
        .filter(|(_, rate)| *rate > 0)
        .map(|(frames, rate)| *frames as f64 / f64::from(*rate))
        .collect();
    if seconds.len() < 3 {
        return None;
    }
    // Whole-bar files, and those among them with a power-of-two bar count.
    let fits: Vec<(f64, usize, usize)> = (60..=200)
        .map(f64::from)
        .chain(std::iter::once(fallback))
        .map(|tempo| {
            let bars = seconds.iter().filter_map(|s| whole_bars(*s, tempo));
            let (mut whole, mut even) = (0, 0);
            for count in bars {
                whole += 1;
                even += usize::from(count <= 16 && count.is_power_of_two());
            }
            (tempo, whole, even)
        })
        .collect();
    let near = |count: usize, best: usize| count * 20 >= best * 19;
    let most_whole = fits.iter().map(|fit| fit.1).max()?;
    if most_whole < 3 || most_whole * 4 < seconds.len() * 3 {
        return None;
    }
    let agreeing = fits.iter().filter(|fit| near(fit.1, most_whole));
    let most_even = agreeing.clone().map(|fit| fit.2).max()?;
    let rank = |tempo: f64| {
        (
            tempo != fallback,
            !(88.0..176.0).contains(&tempo),
            (tempo / fallback).ln().abs(),
        )
    };
    agreeing
        .filter(|fit| near(fit.2, most_even))
        .map(|fit| fit.0)
        .min_by(|a, b| {
            rank(*a)
                .partial_cmp(&rank(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn canonical_group(group: &str) -> String {
    match group.to_ascii_lowercase().as_str() {
        "sequence" => "seq".into(),
        "sphere" => "layer".into(),
        "voice" => "rap".into(),
        "extra" => "xtra".into(),
        "effects" => "effect".into(),
        "guitars" => "guitar".into(),
        other => other.into(),
    }
}

/// Whether the nearest product-named folder, from the file up to the
/// selected folder, names another product.
fn in_foreign_product_folder(path: &Path, root: &Path, product: Product) -> bool {
    path.ancestors()
        .skip(1)
        .take_while(|folder| folder.starts_with(root))
        .filter_map(Path::file_name)
        .find_map(|name| product_folder_hint(&name.to_string_lossy()))
        .is_some_and(|hint| hint != product)
}

fn product_folder_hint(name: &str) -> Option<Product> {
    match compact_name(name).as_str() {
        "\x64ance1" | "\x64ance\x65jay1" => Some(Product::D1),
        "\x64ance2" | "\x64ance\x65jay2" => Some(Product::D2),
        "\x64ance3" | "\x64ance\x65jay3" => Some(Product::D3),
        "\x64ance4" | "\x64ance\x65jay4" => Some(Product::D4),
        "\x68ouse\x65jay" => Some(Product::H1),
        "\x72ave\x65jay" | "\x74echno1" | "\x74echno\x65jay1" => Some(Product::T1),
        "\x74echno2" | "\x74echno\x65jay" | "\x74echno\x65jay2" => Some(Product::T2),
        "\x74echno3" | "\x74echno\x65jay3" => Some(Product::T3),
        _ => None,
    }
}

fn slug(name: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

fn compact_name(name: &str) -> String {
    let mut result = String::new();
    let mut digits = String::new();
    fn finish(result: &mut String, digits: &mut String) {
        if !digits.is_empty() {
            let trimmed = digits.trim_start_matches('0');
            result.push_str(if trimmed.is_empty() { "0" } else { trimmed });
            digits.clear();
        }
    }
    for c in name.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            finish(&mut result, &mut digits);
            if c.is_ascii_alphabetic() {
                result.push(c.to_ascii_lowercase());
            }
        }
    }
    finish(&mut result, &mut digits);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_full_wav_folder_palette_fits_the_project_import_budget() {
        use crate::engine::{ImportedSample, Project};
        let imported = |index: usize| ImportedSample {
            id: super::IMPORTED_ID_START + index as u32,
            name: "Sample".into(),
            group_name: "Wave".into(),
            path: std::path::PathBuf::from(format!("sample-{index}.wav")),
            source_bpm: 140.0,
            source_sample_rate: Some(44_100),
            origin: None,
        };
        let mut project = Project {
            imported_samples: (0..super::MAX_WAV_FOLDER_ITEMS).map(imported).collect(),
            ..Project::default()
        };
        project.validate().unwrap();
        project
            .imported_samples
            .push(imported(super::MAX_WAV_FOLDER_ITEMS));
        assert_eq!(
            project.validate().unwrap_err(),
            "The project has too many imported samples."
        );
    }

    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_FOLDER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn product_folder_hints_come_from_the_selected_folder_and_below() {
        let hinted = |path: &str, root: &str| {
            in_foreign_product_folder(Path::new(path), Path::new(root), Product::D1)
        };
        // A product name above the selected folder says nothing about its files.
        assert!(!hinted(r"C:\Techno2\packs\kick.wav", r"C:\Techno2\packs"));
        assert!(hinted(r"C:\packs\Techno2\kick.wav", r"C:\packs"));
        assert!(hinted(r"C:\packs\Techno2\kick.wav", r"C:\packs\Techno2"));
        // The nearest product folder decides.
        assert!(!hinted(r"C:\packs\Techno2\Dance1\kick.wav", r"C:\packs"));
    }
    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "standalone-wavs-{}-{}",
                std::process::id(),
                NEXT_FOLDER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn wave(&self, relative: &str, rate: u32, frames: usize) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            crate::audio::Pcm {
                sample_rate: rate,
                channels: 2,
                samples: vec![0.25; frames * 2],
            }
            .write_wav(&path)
            .unwrap();
            fs::canonicalize(path).unwrap()
        }
        fn mono(&self, relative: &str, samples: &[i16]) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            crate::audio::Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: samples.iter().map(|s| f32::from(*s) / 32768.0).collect(),
            }
            .write_wav(&path)
            .unwrap();
            fs::canonicalize(path).unwrap()
        }
    }

    fn waveform(seed: i32, frames: usize) -> Vec<i16> {
        (0..frames as i32)
            .map(|i| ((i * 37 + seed * 11) % 200 - 100) as i16)
            .collect()
    }

    fn identity(
        native_id: u32,
        name: &str,
        group: &str,
        audio: &[i16],
    ) -> crate::catalog::WaveMetadata {
        crate::catalog::WaveMetadata {
            native_id,
            code: format!("CODE{native_id}"),
            name: name.into(),
            group_name: group.into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: audio.len() as u32,
            pcm_digest: Some(crate::catalog::pcm_identity_hex(audio)),
        }
    }

    #[test]
    fn audio_digests_identify_wavs_regardless_of_name_folder_offset_or_final_frame() {
        let folder = Folder::new();
        let original = waveform(1, 100);
        let mut shifted: Vec<i16> = original.iter().map(|s| s + 5).collect();
        shifted[99] = 0;
        let other = waveform(2, 100);
        let first = folder.mono("Misc/whatever-07.wav", &original);
        let second = folder.mono("Other/other.wav", &shifted);
        let named = folder.mono("Bass/hihat-hard.wav", &other);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        let metadata = [
            identity(1, "hihat hard", "Bass", &original),
            identity(2, "something else", "Loop", &original),
            identity(3, "hihat hard", "Bass", &waveform(3, 100)),
        ];
        assert_eq!(
            library
                .register_wav_product(Product::T2, &folder.0, &metadata)
                .unwrap(),
            2
        );
        // Identical audio under two identities takes one file each, in order.
        assert_eq!(
            library.sample_by_id(1).unwrap().wav_path.as_ref(),
            Some(&first)
        );
        assert_eq!(
            library.sample_by_id(2).unwrap().wav_path.as_ref(),
            Some(&second)
        );
        assert_eq!(library.sample_by_id(1).unwrap().name, "hihat hard");
        assert_eq!(library.verified_count(), 2);
        // The name, category, and duration agree, but the audio does not.
        assert!(library.sample_by_id(3).is_none());
        assert_eq!(
            library.unverified_waves().collect::<Vec<_>>(),
            vec![(3, named.as_path())]
        );
        let message = library.describe_missing(Product::T2, 3);
        assert!(message.contains("hihat-hard.wav") && message.contains("differs"));
        assert!(!library.describe_missing(Product::T2, 4).contains("differs"));
        assert_eq!(library.read_pcm(2).unwrap().frames(), 100);
    }

    #[test]
    fn digest_matching_requires_comparable_rate_and_enough_frames() {
        let original = waveform(3, 75_600);
        for (name, rate, audio, expected_count) in [
            ("wrong-rate", 22_050, original.clone(), 0),
            (
                "resampled",
                22_050,
                original.iter().step_by(2).copied().collect(),
                1,
            ),
        ] {
            let folder = Folder::new();
            crate::audio::Pcm {
                sample_rate: rate,
                channels: 1,
                samples: audio.iter().map(|s| f32::from(*s) / 32768.0).collect(),
            }
            .write_wav(&folder.0.join("original.wav"))
            .unwrap();
            let mut library = Library::empty();
            library.import_wav_folder(&folder.0, 140.0).unwrap();
            // Older caches can contain a digest for a non-native sample rate.
            library.samples[0].digest = Some(crate::catalog::pcm_identity(&audio));
            assert_eq!(
                library
                    .register_wav_product(
                        Product::T2,
                        &folder.0,
                        &[identity(1, "original", "Wave", &original)],
                    )
                    .unwrap(),
                expected_count,
                "{name}",
            );
            assert_eq!(library.verified_count(), 0, "{name}");
        }
        let folder = Folder::new();
        folder.mono("unrelated.wav", &[10, 20]);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        library.samples[0].digest = Some(crate::catalog::pcm_identity(&[10, 20]));
        assert_eq!(
            library
                .register_wav_product(
                    Product::T2,
                    &folder.0,
                    &[identity(1, "original", "Wave", &[100])],
                )
                .unwrap(),
            0,
        );
    }

    #[test]
    fn folder_tempo_comes_from_agreeing_bar_lengths_or_stays_with_the_fallback() {
        let bar = |bpm: f64, bars: f64| ((44_100.0 * 240.0 / bpm * bars).round() as u64, 44_100);
        let dance = [
            bar(140.0, 1.0),
            bar(140.0, 2.0),
            bar(140.0, 4.0),
            (75_640, 44_100),
        ];
        assert_eq!(infer_folder_tempo(&dance, 120.0), Some(140.0));
        let house = [
            bar(125.0, 1.0),
            bar(125.0, 2.0),
            bar(125.0, 2.0),
            bar(125.0, 4.0),
        ];
        assert_eq!(infer_folder_tempo(&house, 140.0), Some(125.0));
        let rave = [bar(180.0, 1.0), bar(180.0, 2.0), bar(180.0, 2.0)];
        assert_eq!(infer_folder_tempo(&rave, 140.0), Some(180.0));
        // Two-bar loops fit 140 and 70 equally; the fallback decides.
        let long = [bar(140.0, 2.0), bar(140.0, 2.0), bar(140.0, 2.0)];
        assert_eq!(infer_folder_tempo(&long, 140.0), Some(140.0));
        assert_eq!(infer_folder_tempo(&long, 70.0), Some(70.0));
        // Long loops are not halved, and three or five bars are not preferred.
        let phrases = [bar(128.0, 4.0), bar(128.0, 4.0), bar(128.0, 8.0)];
        assert_eq!(infer_folder_tempo(&phrases, 128.0), Some(128.0));
        assert_eq!(infer_folder_tempo(&phrases, 140.0), Some(128.0));
        let sequences = [bar(140.0, 2.0), bar(140.0, 4.0), bar(140.0, 8.0)];
        assert_eq!(infer_folder_tempo(&sequences, 140.0), Some(140.0));
        assert_eq!(infer_folder_tempo(&sequences, 120.0), Some(140.0));
        let four = [bar(120.0, 4.0), bar(120.0, 4.0), bar(120.0, 4.0)];
        assert_eq!(infer_folder_tempo(&four, 140.0), Some(120.0));
        // One half-bar file among hundreds does not double a slow pack.
        let mut slow = vec![bar(90.0, 1.0); 149];
        slow.extend(vec![bar(90.0, 2.0); 391]);
        slow.push(bar(90.0, 3.0));
        slow.push(bar(90.0, 0.5));
        assert_eq!(infer_folder_tempo(&slow, 140.0), Some(90.0));
        assert_eq!(infer_folder_tempo(&slow, 180.0), Some(180.0));
        // The fallback names the octave outright when it fits.
        assert_eq!(infer_folder_tempo(&rave, 180.0), Some(180.0));
        // Long recordings do not agree on a tempo by accident.
        let recordings = [
            (8_401_050, 44_100),
            (9_801_225, 44_100),
            (13_274_100, 44_100),
        ];
        assert_eq!(infer_folder_tempo(&recordings, 140.0), None);
        let takes = [
            (7_938_000, 44_100),
            (8_820_000, 44_100),
            (10_584_000, 44_100),
        ];
        assert_eq!(infer_folder_tempo(&takes, 140.0), None);
        // Half-rate WAVs count by duration, not frames.
        let resampled = [(37_800, 22_050), (75_600, 22_050), (151_200, 22_050)];
        assert_eq!(infer_folder_tempo(&resampled, 140.0), Some(140.0));
        // One-shots, mixed packs, and tiny folders keep the fallback.
        let hits = [
            (9_001, 44_100),
            (12_345, 44_100),
            (23_456, 44_100),
            (31_111, 44_100),
        ];
        assert_eq!(infer_folder_tempo(&hits, 140.0), None);
        let mixed = [
            bar(140.0, 1.0),
            bar(125.0, 1.0),
            bar(180.0, 1.0),
            bar(160.0, 1.0),
        ];
        assert_eq!(infer_folder_tempo(&mixed, 140.0), None);
        assert_eq!(infer_folder_tempo(&dance[..2], 140.0), None);
        assert_eq!(infer_folder_tempo(&dance, f64::NAN), None);
    }

    #[test]
    fn streaming_probes_match_normalized_audio_and_reject_invalid_samples() {
        let folder = Folder::new();
        for (bits, format) in [
            (8, hound::SampleFormat::Int),
            (16, hound::SampleFormat::Int),
            (24, hound::SampleFormat::Int),
            (32, hound::SampleFormat::Int),
            (32, hound::SampleFormat::Float),
        ] {
            let path = folder.0.join(format!("{bits}-{format:?}.wav"));
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 44_100,
                bits_per_sample: bits,
                sample_format: format,
            };
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for value in [-1.0_f32, -0.25, 0.0, 0.5, 0.9] {
                match format {
                    hound::SampleFormat::Float => writer.write_sample(value * 2.0).unwrap(),
                    hound::SampleFormat::Int => writer
                        .write_sample((value * (1_u64 << (bits - 1)) as f32) as i32)
                        .unwrap(),
                }
            }
            writer.finalize().unwrap();
            let pcm = crate::audio::read_wav(&path).unwrap();
            let samples: Vec<_> = pcm
                .samples
                .iter()
                .map(|v| (v * 32768.0).round() as i16)
                .collect();
            let probe = probe_wav(&path, None).unwrap();
            assert_eq!(probe.frame_count, 5);
            assert_eq!(probe.digest, Some(crate::catalog::pcm_identity(&samples)));
            // Truncated input must fail in both callers of the streaming reader.
            let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.set_len(file.metadata().unwrap().len() - 1).unwrap();
            assert!(crate::audio::read_wav(&path).is_err());
            assert!(probe_wav(&path, None).is_err());
        }
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let path = folder.0.join("nonfinite.wav");
            let mut writer = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44_100,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .unwrap();
            writer.write_sample(bad).unwrap();
            writer.finalize().unwrap();
            assert!(
                crate::audio::read_wav(&path)
                    .unwrap_err()
                    .contains("finite")
            );
            assert!(probe_wav(&path, None).unwrap_err().contains("finite"));
        }
    }

    #[test]
    fn folder_scans_report_progress_honour_cancellation_and_reuse_the_cache() {
        let folder = Folder::new();
        let loop_a = folder.mono("Loop/a.wav", &waveform(5, 300));
        folder.mono("Loop/b.wav", &waveform(6, 300));
        folder.mono("c.wav", &waveform(7, 120));
        let progress = ScanProgress::default();
        let mut cache = ScanCache::default();
        let first =
            Library::open_wav_folder_with(&folder.0, 140.0, Some(&progress), Some(&mut cache))
                .unwrap();
        assert_eq!(progress.counts(), (3, 3));
        assert_eq!(cache.len(), 3);
        assert!(cache.has_changes());
        // Loading persisted entries starts a clean cache.
        cache = serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
        assert!(!cache.has_changes());
        let probe = |library: &Library, path: &Path| {
            let sample = library
                .samples
                .iter()
                .find(|s| s.wav_path.as_deref() == Some(path))
                .unwrap();
            (sample.id, sample.frame_count, sample.digest)
        };
        // A second scan takes every file from the cache and builds the same catalog.
        let again = ScanProgress::default();
        let second =
            Library::open_wav_folder_with(&folder.0, 140.0, Some(&again), Some(&mut cache))
                .unwrap();
        assert_eq!(again.counts(), (3, 3));
        assert_eq!(probe(&second, &loop_a), probe(&first, &loop_a));
        assert_eq!(second.samples.len(), 3);
        assert!(!cache.has_changes());
        // A changed file is probed again; its stale entry does not survive.
        let replaced = waveform(8, 200);
        folder.mono("Loop/a.wav", &replaced);
        let third =
            Library::open_wav_folder_with(&folder.0, 140.0, None, Some(&mut cache)).unwrap();
        assert_eq!(probe(&third, &loop_a).1, 200);
        assert!(cache.has_changes());
        assert_eq!(
            probe(&third, &loop_a).2,
            Some(crate::catalog::pcm_identity(&replaced))
        );
        // Cancelling before the scan fails it and leaves the library untouched.
        let cancelled = ScanProgress::default();
        cancelled.cancel();
        let mut untouched = Library::empty();
        let error = untouched
            .import_wav_folder_with(&folder.0, 140.0, Some(&cancelled), None)
            .unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
        assert!(untouched.samples.is_empty());
        // A cache file with the wrong shape does not parse.
        let text = serde_json::to_string(&cache).unwrap();
        let restored: ScanCache = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.len(), cache.len());
        assert!(serde_json::from_str::<ScanCache>("{\"entries\":1}").is_err());
    }

    #[test]
    fn folders_import_with_their_own_tempo_and_report_it() {
        let folder = Folder::new();
        for (name, frames) in [("a", 75_600), ("b", 151_200), ("c", 75_600)] {
            folder.wave(&format!("Dance/{name}.wav"), 44_100, frames);
        }
        for (name, frames) in [("a", 58_800), ("b", 117_600), ("c", 117_600)] {
            folder.wave(&format!("Rave/{name}.wav"), 44_100, frames);
        }
        folder.wave("Hits/one.wav", 44_100, 9_001);
        folder.wave("Hits/two.wav", 44_100, 12_345);
        folder.wave("Hits/three.wav", 44_100, 23_456);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 120.0).unwrap();
        let tempo = |relative: &str| {
            let path = fs::canonicalize(folder.0.join(relative)).unwrap();
            library
                .samples
                .iter()
                .find(|s| s.wav_path.as_ref() == Some(&path))
                .unwrap()
                .source_bpm
        };
        assert_eq!(tempo("Dance/a.wav"), 140.0);
        assert_eq!(tempo("Rave/b.wav"), 180.0);
        assert_eq!(tempo("Hits/one.wav"), 120.0);
        let reported: Vec<_> = library
            .folder_tempos()
            .iter()
            .map(|(path, bpm, count)| {
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    *bpm,
                    *count,
                )
            })
            .collect();
        assert_eq!(
            reported,
            vec![
                ("Dance".to_string(), 140.0, 3),
                ("Rave".to_string(), 180.0, 3)
            ]
        );
    }

    #[test]
    fn tempos_stated_in_pack_file_names_override_duration_guesses() {
        let folder = Folder::new();
        // One bar at 90 is two at 180: durations alone cannot tell them apart.
        for index in 1..=3 {
            folder.wave(
                &format!("Hardcore/HARDCORE_BASS00{index}_180_C_SL3.WAV"),
                44_100,
                117_600,
            );
        }
        // One bar at 125; a pack may also mix tempos within one folder.
        folder.wave("House/HOUSE_KEYS001_125_A_SL1.wav", 44_100, 84_672);
        folder.wave("House/HOUSE_KEYS002_125_Am_SL1.wav", 44_100, 84_672);
        folder.wave("House/TECHHOUSE_SYNTH040_132_A_SL3.WAV", 44_100, 80_182);
        folder.wave("House/SPOON001_CLAPBREAK_128_X_SC4(L).wav", 44_100, 82_688);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        let tempo = |name: &str| {
            library
                .samples
                .iter()
                .find(|s| s.name == name)
                .unwrap()
                .source_bpm
        };
        assert_eq!(tempo("HARDCORE_BASS001_180_C_SL3"), 180.0);
        assert_eq!(tempo("HOUSE_KEYS002_125_Am_SL1"), 125.0);
        assert_eq!(tempo("TECHHOUSE_SYNTH040_132_A_SL3"), 132.0);
        assert_eq!(tempo("SPOON001_CLAPBREAK_128_X_SC4(L)"), 128.0);
        let reported: Vec<_> = library
            .folder_tempos()
            .iter()
            .map(|(path, bpm, count)| {
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    *bpm,
                    *count,
                )
            })
            .collect();
        assert_eq!(
            reported,
            vec![
                ("Hardcore".to_string(), 180.0, 3),
                ("House".to_string(), 125.0, 2),
                ("House".to_string(), 128.0, 1),
                ("House".to_string(), 132.0, 1),
            ]
        );
    }

    #[test]
    fn a_pack_bpm_inf_corrects_duration_guesses_in_its_folders() {
        let folder = Folder::new();
        let inf = |pack: &str, name: &str, bpm: u32| {
            let text =
                format!("[Tempo]\r\nBPM={bpm}\r\n\r\n[HarmonyTable]\r\nScale=c\r\nSample1=c\r\n");
            fs::write(folder.0.join(pack).join(name), text).unwrap();
        };
        // Three bars at 120 are four at 160; the durations prefer four.
        for index in 1..=3 {
            folder.wave(&format!("Flamenco/Guitar/g{index}.wav"), 44_100, 264_600);
        }
        inf("Flamenco", "BPM.inf", 120);
        // One bar at 80 reads as two at 160, which plays the same.
        for index in 1..=3 {
            folder.wave(&format!("Ambient/Pads/p{index}.wav"), 44_100, 132_300);
        }
        for (name, frames) in [("one", 9_001), ("two", 12_345), ("three", 23_456)] {
            folder.wave(&format!("Ambient/Fx/{name}.wav"), 44_100, frames);
        }
        inf("Ambient", "BPM.INF", 80);
        for (name, frames) in [("one", 9_001), ("two", 12_345), ("three", 23_456)] {
            folder.wave(&format!("Hits/{name}.wav"), 44_100, frames);
        }
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        let tempo = |name: &str| {
            library
                .samples
                .iter()
                .find(|s| s.name == name)
                .unwrap()
                .source_bpm
        };
        assert_eq!(tempo("g1"), 120.0);
        assert_eq!(tempo("p1"), 160.0);
        assert_eq!(tempo("two"), 160.0);
        let hits = fs::canonicalize(folder.0.join("Hits/one.wav")).unwrap();
        let hit = library
            .samples
            .iter()
            .find(|s| s.wav_path.as_ref() == Some(&hits));
        assert_eq!(hit.unwrap().source_bpm, 140.0);
    }

    #[test]
    fn bpm_inf_tempo_is_the_bpm_key_of_the_tempo_section() {
        let tempo = super::stated_pack_tempo;
        assert_eq!(tempo("[Tempo]\nBPM=125\n"), Some(125.0));
        assert_eq!(
            tempo("[HarmonyTable]\nBPM=99\n[ tempo ]\r\n bpm = 87.5 \r\n"),
            Some(87.5)
        );
        assert_eq!(tempo("BPM=125\n"), None);
        assert_eq!(tempo("[Tempo]\nBPM=fast\n"), None);
        assert_eq!(tempo("[Tempo]\nBPM=1250\n"), None);
        assert!(super::octave_apart(80.0, 160.0) && super::octave_apart(180.0, 90.0));
        assert!(!super::octave_apart(120.0, 160.0));
    }

    #[test]
    fn named_tempo_needs_a_bpm_token_followed_by_a_key_token() {
        let tempo = |name: &str| super::named_tempo(Path::new(name));
        assert_eq!(tempo("HARDCORE_BASS001_180_C_SL3.WAV"), Some(180.0));
        assert_eq!(tempo("DANCE_VOCALS001_140_X_SL1.wav"), Some(140.0));
        assert_eq!(tempo("SNTH_96_F#m_SC4(L).wav"), Some(96.0));
        assert_eq!(tempo("PAD_125_Bb_SL1.wav"), Some(125.0));
        assert_eq!(tempo("T2TRK0303  7.wav"), None);
        assert_eq!(tempo("r1bs001.wav"), None);
        assert_eq!(tempo("LOOP_140_SL1.wav"), None);
        assert_eq!(tempo("LOOP_1400_A_SL1.wav"), None);
        assert_eq!(tempo("LOOP_20_A_SL1.wav"), None);
        assert_eq!(tempo("LOOP_140_H_SL1.wav"), None);
    }

    #[test]
    fn identities_that_outnumber_identical_files_share_the_last_file() {
        let folder = Folder::new();
        let original = waveform(4, 80);
        let first = folder.mono("a.wav", &original);
        let second = folder.mono("b.wav", &original);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        let metadata = [
            identity(10, "one", "Loop", &original),
            identity(11, "two", "Loop", &original),
            identity(12, "three", "Loop", &original),
        ];
        assert_eq!(
            library
                .register_wav_product(Product::D3, &folder.0, &metadata)
                .unwrap(),
            3
        );
        let path = |native: u32| {
            library
                .sample_by_id(Product::D3.global_sample_id(native).unwrap())
                .unwrap()
                .wav_path
                .clone()
                .unwrap()
        };
        assert_eq!(path(10), first);
        assert_eq!(path(11), second);
        assert_eq!(path(12), second);
        assert_eq!(library.verified_count(), 3);
        assert!(library.unverified_waves().next().is_none());
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn standalone_folder_reads_nested_wavs_and_skips_unreadable_files() {
        let folder = Folder::new();
        folder.wave("Bass/my original.WAV", 22_050, 220);
        folder.wave("another.wav", 48_000, 480);
        let mut library = Library::open_wav_folder(&folder.0, 160.0).unwrap();
        assert!(library.archive_path.as_os_str().is_empty());
        assert!(library.bindings().is_empty());
        assert_eq!(
            library.wav_root(),
            Some(fs::canonicalize(&folder.0).unwrap().as_path())
        );
        assert_eq!(library.samples.len(), 2);
        let bass = library
            .samples
            .iter()
            .find(|s| s.name == "my original")
            .unwrap();
        assert_eq!(bass.group_name, "Bass");
        assert_eq!(bass.source_bpm, 160.0);
        assert_eq!(library.read_pcm(bass.id).unwrap().frames(), 220);
        assert_eq!(library.imported_samples().len(), 2);
        assert!(library.skipped_waves().is_empty());
        let before = library.imported_samples();
        fs::write(folder.0.join("invalid.wav"), b"not audio").unwrap();
        // A header without frames, as some sample packs ship.
        let mut empty = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
        empty.extend_from_slice(&44_100u32.to_le_bytes());
        empty.extend_from_slice(&88_200u32.to_le_bytes());
        empty.extend_from_slice(b"\x02\0\x10\0data\0\0\0\0");
        fs::write(folder.0.join("Bass/empty.wav"), empty).unwrap();
        library.import_wav_folder(&folder.0, 160.0).unwrap();
        assert_eq!(library.imported_samples(), before);
        let skipped: Vec<_> = library
            .skipped_waves()
            .iter()
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(skipped, ["empty.wav", "invalid.wav"]);
        assert!(library.import_wav_folder(&folder.0, f64::NAN).is_err());

        let unreadable = Folder::new();
        fs::write(unreadable.0.join("invalid.wav"), b"not audio").unwrap();
        let error = Library::open_wav_folder(&unreadable.0, 140.0).unwrap_err();
        assert!(error.contains("no readable WAV samples"), "{error}");
        assert!(error.contains("invalid.wav"), "{error}");
    }

    #[test]
    fn moved_wave_resolution_preserves_relative_paths_and_returns_stable_candidates() {
        let folder = Folder::new();
        let bass = folder.wave("Bass/sound.wav", 44_100, 10);
        let drum = folder.wave("Drum/sound.wav", 44_100, 10);
        let only = folder.wave("only.wav", 44_100, 10);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        assert_eq!(
            library
                .resolve_wav_reference(r"C:\old\Bass\SOUND.WAV")
                .unwrap(),
            bass
        );
        assert_eq!(
            library.resolve_wav_reference("old/place/only.wav").unwrap(),
            only
        );
        assert_eq!(
            library.find_wav_candidates("sound.wav").unwrap(),
            vec![bass.clone(), drum]
        );
        assert_eq!(library.resolve_wav_reference("sound.wav").unwrap(), bass);
        assert!(library.resolve_wav_reference("../only.wav").is_err());
        assert!(library.resolve_wav_reference("missing.wav").is_err());
    }

    #[test]
    fn palette_restore_keeps_project_ids_and_does_not_decode_unused_files_again() {
        let folder = Folder::new();
        let first = folder.wave("first.wav", 44_100, 10);
        let second = folder.wave("second.wav", 44_100, 10);
        let mut palette = Library::empty();
        palette.import_wav_folder(&folder.0, 160.0).unwrap();
        let mut project = Library::empty();
        let used = project
            .import_wav_with_tempo(&second, Some("Project name"), "Wave", 140.0)
            .unwrap();
        fs::remove_file(first).unwrap();
        let added = project.restore_wav_palette(&palette).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].id, used.id + 1);
        assert_eq!(added[0].source_bpm, 160.0);
        assert_eq!(project.sample_by_id(used.id).unwrap().name, "Project name");
        assert!(project.restore_wav_palette(&palette).unwrap().is_empty());
        assert!(project.read_pcm(added[0].id).is_err());
    }

    #[test]
    fn unchanged_imports_keep_their_catalog_entries_without_decoding_again() {
        let folder = Folder::new();
        let path = folder.wave("loop.wav", 44_100, 10);
        let mut library = Library::empty();
        let imported = library
            .import_wav_with_tempo(&path, Some("Loop"), "Wave", 140.0)
            .unwrap();
        // Undo and redo re-apply identical metadata: the file is not read again.
        fs::remove_file(&path).unwrap();
        library
            .apply_imports(std::slice::from_ref(&imported))
            .unwrap();
        assert_eq!(library.sample_by_id(imported.id).unwrap().frame_count, 10);
        // Changed metadata needs the file and fails atomically without it.
        let mut retimed = imported.clone();
        retimed.source_bpm = 120.0;
        assert!(library.apply_imports(&[retimed]).is_err());
        let mut renamed = imported;
        renamed.name = "Other".into();
        assert!(library.apply_imports(&[renamed]).is_err());
        assert_eq!(library.sample_by_id(1_000_000).unwrap().name, "Loop");
    }

    #[test]
    fn wave_product_uses_catalog_bars_and_tempo_without_original_files() {
        let folder = Folder::new();
        folder.wave("Bass/soft-dust-06.wav", 22_050, 100);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 160.0).unwrap();
        let metadata = crate::catalog::WaveMetadata {
            native_id: 731,
            code: "native-code".into(),
            name: "soft dust 6".into(),
            group_name: "bass".into(),
            bars: 4,
            source_bpm: 180.0,
            frame_count: 200,
            pcm_digest: None,
        };
        assert_eq!(
            library
                .register_wav_product(Product::T1, &folder.0, std::slice::from_ref(&metadata))
                .unwrap(),
            1
        );
        let id = Product::T1.global_sample_id(731).unwrap();
        let sample = library.sample_by_id(id).unwrap();
        assert_eq!(sample.bars, 4);
        assert_eq!(sample.source_bpm, 180.0);
        assert!(library.is_wav_product(Product::T1));
        assert_eq!(library.read_pcm(id).unwrap().frames(), 100);
        library.apply_imports(&[]).unwrap();
        assert_eq!(library.read_pcm(id).unwrap().frames(), 100);

        let duplicate = folder.wave("extra/Bass/soft-dust-6.wav", 22_050, 100);
        library
            .import_wav_with_tempo(&duplicate, None, "bass", 140.0)
            .unwrap();
        let count = library.samples.len();
        assert_eq!(
            library
                .register_wav_product(Product::D1, &folder.0, &[metadata])
                .unwrap(),
            0
        );
        assert_eq!(library.samples.len(), count);
        assert!(library.source_root(Product::D1).is_none());
    }

    #[test]
    fn songs_record_a_sample_folder_product_by_the_folder_that_holds_it() {
        let folder = Folder::new();
        folder.wave("_original/Rave/Bass/soft-dust-06.wav", 22_050, 100);
        folder.wave("_original/Rave/Drum/kick-01.wav", 22_050, 100);
        folder.wave("Hardcore/Bass/HARDCORE_BASS001_180_C_SL3.WAV", 44_100, 100);
        let mut library = Library::open_wav_folder(&folder.0, 140.0).unwrap();
        let entry = |native_id: u32, name: &str, group: &str| crate::catalog::WaveMetadata {
            native_id,
            code: format!("code-{native_id}"),
            name: name.into(),
            group_name: group.into(),
            bars: 1,
            source_bpm: 180.0,
            frame_count: 200,
            pcm_digest: None,
        };
        library.remove_product(Product::T1).unwrap();
        let root = library.wav_root().unwrap().to_path_buf();
        library
            .register_wav_product(
                Product::T1,
                &root,
                &[
                    entry(731, "soft dust 6", "bass"),
                    entry(851, "kick 1", "drum"),
                ],
            )
            .unwrap();
        let binding = LibraryBinding {
            product: Product::T1,
            root: root.clone(),
        };
        let bass = Product::T1.global_sample_id(731).unwrap();
        let kick = Product::T1.global_sample_id(851).unwrap();
        let rave = root.join("_original").join("Rave");
        let used = |ids: &[u32]| ids.iter().copied().collect::<HashSet<_>>();
        assert_eq!(
            library.song_binding(&binding, &used(&[bass, kick])).root,
            rave
        );
        // Only the files that the song uses decide the folder.
        assert_eq!(
            library.song_binding(&binding, &used(&[bass])).root,
            rave.join("Bass")
        );
        assert_eq!(library.song_binding(&binding, &used(&[])), binding);
        // A provider that is not this library's WAV provider is recorded as it is.
        let elsewhere = LibraryBinding {
            product: Product::T2,
            root: root.join("elsewhere"),
        };
        assert_eq!(
            library.song_binding(&elsewhere, &used(&[bass, kick])),
            elsewhere
        );
    }

    #[test]
    fn colliding_display_labels_do_not_guess_native_ids_and_codes_override_category() {
        let folder = Folder::new();
        folder.wave("Bass/hihat-hard.wav", 44_100, 100);
        folder.wave("unrelated/T1DC501.wav", 44_100, 100);
        let mut metadata = crate::catalog::WaveMetadata {
            native_id: 13776,
            code: "T1DC501".into(),
            name: "hihat hard".into(),
            group_name: "Bass".into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 100,
            pcm_digest: None,
        };
        let mut second = metadata.clone();
        second.native_id += 1;
        second.code = "T1DC502".into();
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        assert_eq!(
            library
                .register_wav_product(Product::T2, &folder.0, &[metadata.clone(), second.clone()])
                .unwrap(),
            2
        );
        assert!(
            library
                .sample_by_id(13776)
                .unwrap()
                .wav_path
                .as_ref()
                .unwrap()
                .ends_with("T1DC501.wav")
        );
        // With no code-named source, neither colliding display label is identified.
        let ambiguous_folder = Folder::new();
        ambiguous_folder.wave("Bass/hihat-hard.wav", 44_100, 100);
        let mut ambiguous = Library::empty();
        ambiguous
            .import_wav_folder(&ambiguous_folder.0, 140.0)
            .unwrap();
        assert_eq!(
            ambiguous
                .register_wav_product(
                    Product::T2,
                    &ambiguous_folder.0,
                    &[metadata.clone(), second]
                )
                .unwrap(),
            0
        );
        assert_eq!(ambiguous.imported_samples().len(), 1);
        metadata.native_id = 1;
        assert_eq!(
            ambiguous
                .register_wav_product(Product::D1, &ambiguous_folder.0, &[metadata])
                .unwrap(),
            1
        );
    }
    #[test]
    fn names_preserve_exact_slug_and_normalize_numeric_padding() {
        assert_eq!(compact_name("soft dust 06"), compact_name("soft-dust-6"));
        assert_ne!(slug("tech noR"), slug("alpha R"));
        assert_eq!(decode_1252(&[0x80, 0xe9]), "€é");
    }

    #[test]
    fn imported_wav_roundtrips_and_missing_file_is_atomic() {
        let folder =
            std::env::temp_dir().join(format!("mixjam-techno-import-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("sample.wav");
        crate::audio::Pcm {
            sample_rate: 22050,
            channels: 2,
            samples: vec![0.25; 440],
        }
        .write_wav(&path)
        .unwrap();
        let mut library = Library::empty();
        let imported = library
            .import_wav_with_tempo(&path, Some("My sample"), "Wave", 140.0)
            .unwrap();
        assert_eq!(imported.id, 1_000_000);
        let sample = library.sample_by_id(imported.id).unwrap();
        assert_eq!(sample.frame_count, 220);
        assert_eq!(sample.sample_rate, 22050);
        assert_eq!(sample.channels, 2);
        assert_eq!(library.read_pcm(imported.id).unwrap().frames(), 220);
        assert_eq!(
            library
                .import_wav_with_tempo(&path, None, "Wave", 140.0)
                .unwrap()
                .id,
            imported.id
        );
        let mut project = crate::engine::Project::default();
        project.imported_samples.push(imported.clone());
        let saved = serde_json::to_vec(&project).unwrap();
        let restored: crate::engine::Project = serde_json::from_slice(&saved).unwrap();
        library.apply_imports(&restored.imported_samples).unwrap();
        let mut missing = imported;
        missing.path = folder.join("missing.wav");
        assert!(library.apply_imports(&[missing]).is_err());
        assert_eq!(library.samples.len(), 1);
        assert_eq!(library.samples[0].name, "My sample");
        fs::remove_file(path).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn native_packed_reads_unicode_host_paths_without_native_ansi_limits() {
        let folder =
            std::env::temp_dir().join(format!("mixjam-techno-packed-host-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("音源.\x70xd");
        let mut bytes = b"t\x50xD\0T".to_vec();
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(37u16.to_le_bytes());
        bytes.extend([255, 128, 255, 128]);
        fs::write(&path, bytes).unwrap();
        let mut library = Library::empty();
        library.samples.push(Sample {
            id: 100_731,
            code: "source.\x70xd".into(),
            name: "Native source".into(),
            bars: 1,
            group_name: "Loop".into(),
            offset: 0,
            length: 16,
            frame_count: 2,
            sample_rate: 44100,
            source_bpm: 180.0,
            channels: 1,
            wav_path: None,
            digest: None,
        });
        library.by_id.insert(100_731, 0);
        library
            .sources
            .insert(100_731, AudioSource::T1Packed { path: path.clone() });
        let pcm = library.read_pcm(100_731).unwrap();
        assert_eq!(pcm.samples, vec![74.0 / 32768.0; 3]);
        fs::remove_file(path).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    #[ignore = "Requires all eight local native sample libraries; decodes the complete corpus."]
    fn every_registered_native_sample_decodes_with_its_native_length_rules() {
        let mut total = 0;
        let mut failures = Vec::new();
        for (product, expected) in [
            (Product::D1, 2845),
            (Product::D2, 3795),
            (Product::D3, 5049),
            (Product::D4, 5330),
            (Product::H1, 3563),
            (Product::T1, 3017),
            (Product::T2, 4015),
            (Product::T3, 4006),
        ] {
            let root = crate::project_io::default_root(product).expect("Native library fixture");
            let mut library = Library::empty();
            library
                .register(product, &root, &crate::project_io::reference_samples_root())
                .unwrap();
            assert_eq!(library.samples.len(), expected, "{}", product.label());
            let started = std::time::Instant::now();
            let mut frames = 0u64;
            let mut decoded = 0;
            for sample in &library.samples {
                let context = format!("{} / {} / {}", product.label(), sample.id, sample.name);
                let pcm = match library.read_pcm(sample.id) {
                    Ok(pcm) => pcm,
                    Err(error) => {
                        failures.push(format!("{context}: {error}"));
                        continue;
                    }
                };
                if let Err(error) = pcm.validate() {
                    failures.push(format!("{context}: {error}"));
                }
                let declared = u64::from(sample.frame_count);
                let length_matches = if let AudioSource::T1Packed { path } =
                    &library.sources[&sample.id]
                {
                    // Complete final runs and the metadata loader's pending
                    // initial sample differ from isolated declared-length PCM.
                    let mut prefix = [0; 512];
                    let read = fs::File::open(path).unwrap().read(&mut prefix).unwrap();
                    let header = legacy_format::packed::parse_header(&prefix[..read]).unwrap();
                    let initial =
                        u64::from(header.kind == legacy_format::packed::HeaderKind::Metadata);
                    if product == Product::D1 && sample.id == 205_043 {
                        // Authenticated factory file d4sp434.packed has 109,215
                        // physical codes and a nominal 151,200-frame header.
                        use sha2::Digest;
                        let hash = format!("{:x}", sha2::Sha256::digest(fs::read(path).unwrap()));
                        if declared != 151_200
                            || hash
                                != "07c40aaf4668c213c3b9f340c1db8d266b7fa55887897d0ba0be37186e5c649f"
                        {
                            failures.push(format!(
                                "{context}: the authenticated short-file fixture changed"
                            ));
                        }
                        pcm.frames() == 109_216
                    } else {
                        (declared.saturating_sub(4) + initial..=declared + 4 + initial)
                            .contains(&pcm.frames())
                    }
                } else {
                    pcm.frames() == declared
                };
                if !length_matches {
                    failures.push(format!(
                        "{context}: {} frames for declared {declared}",
                        pcm.frames()
                    ));
                }
                if pcm.sample_rate != sample.sample_rate || pcm.channels != sample.channels {
                    failures.push(format!(
                        "{context}: decoded format differs from the catalog"
                    ));
                }
                frames += pcm.frames();
                decoded += 1;
            }
            eprintln!(
                "Native decode: {}: {decoded}/{expected} samples, {frames} frames, {:.3} seconds",
                product.slug(),
                started.elapsed().as_secs_f64()
            );
            total += library.samples.len();
        }
        assert_eq!(total, 31_620);
        assert!(
            failures.is_empty(),
            "{} native decode failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    #[ignore = "Requires the local WAV sample corpus; validates every WAV without original providers."]
    fn standalone_local_wave_corpus_has_native_aliases_without_original_providers() {
        let root = std::env::var_os("MIXJAM_TECHNO_WAV_CORPUS")
            .map(PathBuf::from)
            .expect("Set MIXJAM_TECHNO_WAV_CORPUS");
        let started = std::time::Instant::now();
        let library = Library::open_wav_folder(&root, 140.0).unwrap();
        assert!(library.archive_path.as_os_str().is_empty());
        let native = library
            .samples
            .iter()
            .filter(|s| Product::sample_key(s.id).is_some_and(|(p, _)| p == Product::T2))
            .count();
        eprintln!(
            "Standalone WAV folder: {} palette entries, {native} T2 identities, {} generic WAVs",
            library.samples.len(),
            library.imported_samples().len()
        );
        eprintln!(
            "WAV scan elapsed: {:.3} seconds",
            started.elapsed().as_secs_f64()
        );
        assert!(native > 0);
        assert!(
            library
                .samples
                .iter()
                .all(|sample| sample.wav_path.is_some())
        );
    }

    #[test]
    #[ignore = "Requires the installed original and the supplied WAV corpus."]
    fn installed_catalog_has_complete_nonoverlapping_ranges() {
        let root = crate::project_io::default_root(Product::T2).expect("T2 fixtures");
        let mut library = Library::empty();
        library
            .register(
                Product::T2,
                &root,
                &crate::project_io::reference_samples_root(),
            )
            .unwrap();
        assert_eq!(library.samples.len(), 4015);
        assert_eq!(
            library.samples.last().unwrap().offset + library.samples.last().unwrap().length,
            429261617
        );
        assert!(
            library
                .samples
                .iter()
                .filter(|s| s.wav_path.is_some())
                .count()
                > 3800
        );
        assert_eq!(library.sample_by_id(14546).unwrap().frame_count, 75600);
        assert_eq!(library.read_pcm(14546).unwrap().frames(), 75600);
        assert_eq!(library.step_synth_sources().unwrap().len(), 504);
    }
}
