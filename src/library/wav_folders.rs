//! Opening WAV folders as libraries: the scan entry points, the per-product
//! registration of the files found, and the identification of each product's
//! sounds from the catalog.

use super::{
    AudioSource, Library, Sample,
    naming::{canonical_group, compact_name, in_foreign_product_folder, product_folder_hint},
    scan::{
        SCAN_CANCELLED, ScanCache, ScanProgress, check_library_folders, list_wav_folder, probe_all,
    },
};
use crate::products::{IMPORTED_ID_START, LibraryBinding, Product};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

impl Library {
    #[cfg(test)]
    pub(crate) fn open_wav_folder(root: &Path, source_bpm: f64) -> Result<Self, String> {
        Self::open_wav_folder_with(root, source_bpm, None, None)
    }

    /// Open a WAV folder as a library, with progress reporting, cancellation,
    /// and a probe cache, for a scan that runs on another thread.
    pub fn open_wav_folder_with(
        root: &Path,
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        cache: Option<&mut ScanCache>,
    ) -> Result<Self, String> {
        Self::open_wav_folders_with(&[root.to_path_buf()], source_bpm, progress, cache)
    }

    /// Open up to [`MAX_LIBRARY_FOLDERS`] WAV folders as one library. One
    /// progress total covers every folder. A folder that cannot be read, or
    /// that has no readable WAV, is left out and listed in
    /// [`Self::unloaded_roots`]; the other folders load. The scan fails only
    /// when no folder loads, or when it is cancelled. A product's provider is
    /// the first folder, in list order, that identifies any of its samples;
    /// later folders fill only the identities that earlier folders lack. A
    /// later folder's WAV whose audio is identical to an identity that an
    /// earlier folder serves is counted in [`Self::duplicate_copies`], not
    /// listed as a separate sound.
    pub fn open_wav_folders_with(
        roots: &[PathBuf],
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        cache: Option<&mut ScanCache>,
    ) -> Result<Self, String> {
        // One reading of the catalogs serves the whole scan.
        let catalogs = crate::catalog::catalogs();
        Self::open_wav_folders_identified(
            roots,
            source_bpm,
            progress,
            cache,
            |product| catalogs.identities(product),
            |product, id| catalogs.aliases(product, id),
            crate::source_tempo::SourceTempos::for_catalogs(catalogs),
        )
    }

    /// [`Self::open_wav_folders_with`] with the identity catalogs that
    /// `metadata` returns for each product.
    pub(super) fn open_wav_folders_identified<'m>(
        roots: &[PathBuf],
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        mut cache: Option<&mut ScanCache>,
        metadata: impl Fn(Product) -> &'m [crate::catalog::WaveMetadata],
        aliases: impl Fn(Product, u32) -> &'m [crate::catalog::WaveMetadata] + Copy,
        evidence: &crate::source_tempo::SourceTempos,
    ) -> Result<Self, String> {
        check_library_folders(roots)?;
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("The WAV folder needs a finite positive source tempo.".into());
        }
        let cancelled = || progress.is_some_and(ScanProgress::is_cancelled);
        let mut library = Self::empty();
        let mut unloaded = Vec::new();
        let mut listings = Vec::new();
        for root in roots {
            match list_wav_folder(root, progress) {
                Ok(listing) => {
                    library.wav_roots.push(listing.root.clone());
                    listings.push(listing);
                }
                Err(_) if cancelled() => return Err(SCAN_CANCELLED.into()),
                Err(error) => {
                    let root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
                    library.wav_roots.push(root.clone());
                    unloaded.push((root, error));
                }
            }
        }
        if let Some(cache) = cache.as_deref_mut() {
            cache.forget_unlisted(&listings);
        }
        let paths: Vec<PathBuf> = listings
            .iter()
            .flat_map(|listing| listing.paths.iter().cloned())
            .collect();
        let mut probes = probe_all(&paths, progress, cache)?.into_iter();
        drop(paths);
        for listing in listings {
            let root = listing.root.clone();
            let probes = probes.by_ref().take(listing.paths.len()).collect();
            match library.import_listing(listing, probes, source_bpm, evidence) {
                Ok((_, tempos, skipped)) => {
                    library.folder_tempos.extend(tempos);
                    library.skipped_waves.extend(skipped);
                }
                Err(error) => unloaded.push((root, error)),
            }
        }
        if unloaded.len() == roots.len() {
            return Err(match &unloaded[..] {
                [(_, error)] => error.clone(),
                _ => format!(
                    "No library folder could be loaded. {}",
                    unloaded
                        .iter()
                        .map(|(root, error)| format!("{}: {error}", root.display()))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            });
        }
        unloaded.sort_by_key(|(root, _)| library.wav_roots.iter().position(|known| known == root));
        library.unloaded_roots = unloaded;
        let loaded: Vec<PathBuf> = library
            .wav_roots
            .iter()
            .filter(|root| {
                !library
                    .unloaded_roots
                    .iter()
                    .any(|(failed, _)| failed == *root)
            })
            .cloned()
            .collect();
        for product in Product::ALL {
            library.register_wav_folders_with_aliases(
                product,
                &loaded,
                metadata(product),
                |id| aliases(product, id),
            )?;
        }
        // A folder later in the list can serve an identity that an earlier
        // folder only had a different recording of.
        library
            .unverified
            .retain(|id, _| !library.by_id.contains_key(id));
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
        // A copy that serves another identity is not a duplicate.
        let mut duplicates = std::mem::take(&mut library.duplicate_copies);
        duplicates.retain(|path| !native_paths.contains(path));
        duplicates.sort();
        duplicates.dedup();
        library.samples.retain(|sample| {
            sample.id < IMPORTED_ID_START
                || sample.wav_path.as_ref().is_none_or(|path| {
                    !native_paths.contains(path) && duplicates.binary_search(path).is_err()
                })
        });
        library.duplicate_copies = duplicates;
        library.reindex();
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
        if self.source_root(product).is_some() {
            return Err(format!(
                "{} already has a sample provider.",
                product.label()
            ));
        }
        self.identify_wav_product(product, root, metadata, false, |_| &[])
    }

    /// Fill the identities that this product's provider lacks from another
    /// library folder's WAVs. The provider and its folder stay the same, and
    /// identities the library already has keep their files.
    pub fn supplement_wav_product(
        &mut self,
        product: Product,
        root: &Path,
        metadata: &[crate::catalog::WaveMetadata],
    ) -> Result<usize, String> {
        if self.source_root(product).is_none() {
            return Err(format!("{} has no sample provider.", product.label()));
        }
        self.identify_wav_product(product, root, metadata, true, |_| &[])
    }

    /// Identify one product in library folders, in list order: the first
    /// folder that identifies any of its samples becomes its provider, and
    /// each later folder fills only the identities still missing.
    pub fn register_wav_folders(
        &mut self,
        product: Product,
        roots: &[PathBuf],
        metadata: &[crate::catalog::WaveMetadata],
    ) -> Result<(), String> {
        self.register_wav_folders_with_aliases(product, roots, metadata, |_| &[])
    }

    fn register_wav_folders_with_aliases<'m>(
        &mut self,
        product: Product,
        roots: &[PathBuf],
        metadata: &[crate::catalog::WaveMetadata],
        aliases: impl Fn(u32) -> &'m [crate::catalog::WaveMetadata] + Copy,
    ) -> Result<(), String> {
        for root in roots {
            if self.source_root(product).is_some() {
                self.identify_wav_product(product, root, metadata, true, aliases)?;
            } else {
                self.identify_wav_product(product, root, metadata, false, aliases)?;
            }
        }
        Ok(())
    }

    pub(super) fn identify_wav_product<'m>(
        &mut self,
        product: Product,
        root: &Path,
        metadata: &[crate::catalog::WaveMetadata],
        supplement: bool,
        aliases: impl Fn(u32) -> &'m [crate::catalog::WaveMetadata],
    ) -> Result<usize, String> {
        const DIGEST_SCORE: u8 = 3;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        // File names by compact form, for archive codes, and by label key,
        // for the original's sound names, which catalogs keep only as keys.
        let mut by_name: HashMap<String, Vec<&Sample>> = HashMap::new();
        let mut by_label: HashMap<String, Vec<&Sample>> = HashMap::new();
        let mut by_digest: HashMap<([u8; 16], u32), Vec<&Sample>> = HashMap::new();
        // Old scan caches can contain digests for other rates or tiny files.
        // Their samples cannot establish identity against native 44.1 kHz PCM.
        let comparable_digest = |sample: &Sample| {
            sample.digest.filter(|_| {
                sample.sample_rate == legacy_format::NATIVE_SAMPLE_RATE
                    && sample.channels == 1
                    && sample.frame_count > 2
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
            let compact = compact_name(&name);
            if !compact.is_empty() {
                by_label
                    .entry(crate::catalog::label_key(&name))
                    .or_default()
                    .push(sample);
            }
            by_name.entry(compact).or_default().push(sample);
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
        let mut duplicates = Vec::new();
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
            // An earlier library folder already serves this identity. A copy
            // of its verified audio here is an exact duplicate.
            if supplement && self.by_id.contains_key(&id) {
                if self.verified.contains(&id)
                    && let Some(digest) = entry.pcm_identity().filter(|_| entry.frame_count > 2)
                    && let Some(files) = by_digest.get(&(digest, entry.frame_count))
                {
                    duplicates.extend(files.iter().filter_map(|file| file.wav_path.clone()));
                }
                continue;
            }
            entries.push((
                entry,
                id,
                entry.pcm_identity().filter(|_| entry.frame_count > 2),
            ));
        }
        // The sound keeps the name that the user's file gives it: catalogs
        // hold no names.
        let adopt = |entry: &crate::catalog::WaveMetadata, candidate: &Sample, id: u32| {
            let mut sample = candidate.clone();
            sample.id = id;
            sample.code = entry.code.clone();
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
            let mut candidates: HashMap<&PathBuf, (&Sample, u8)> = HashMap::new();
            for name_entry in std::iter::once(*entry).chain(aliases(entry.native_id)) {
                let code = name_entry.code.replace('\\', "/");
                let code_name = Path::new(&code)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                for (files, score) in [
                    (by_label.get(&name_entry.label_key), 1),
                    (by_name.get(&compact_name(&code_name)), 2),
                ] {
                    for sample in files.into_iter().flatten() {
                        let path = sample.wav_path.as_ref().unwrap();
                        if claimed.contains(path) {
                            continue;
                        }
                        if score < 2 && in_foreign_product_folder(path, &root, product) {
                            continue;
                        }
                        let category_matches = score == 2
                            || canonical_group(&sample.group_name)
                                == canonical_group(&name_entry.group_name)
                            || sample.group_name == "Wave"
                            || product_folder_hint(&sample.group_name).is_some();
                        let measured_frames = f64::from(sample.frame_count)
                            * f64::from(legacy_format::NATIVE_SAMPLE_RATE)
                            / f64::from(sample.sample_rate);
                        if !category_matches
                            || (measured_frames - f64::from(entry.frame_count)).abs() > 4.0
                        {
                            continue;
                        }
                        if let (Some(expected), Some(actual)) =
                            (expected, comparable_digest(sample))
                            && *expected != actual
                        {
                            unverified.entry(*id).or_insert_with(|| path.clone());
                            continue;
                        }
                        candidates
                            .entry(path)
                            .and_modify(|(_, best)| *best = (*best).max(score))
                            .or_insert((sample, score));
                    }
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
        self.duplicate_copies.extend(duplicates);
        // The first folder that had a different recording is the one reported.
        for (id, path) in unverified {
            self.unverified.entry(id).or_insert(path);
        }
        if count > 0 && !supplement {
            self.bindings.push(LibraryBinding { product, root });
        }
        Ok(count)
    }

    /// Whether the sample folder supplies under half of this product's
    /// catalog, as when it only shares some audio with another product.
    pub fn is_sparse_wav_product(&self, product: Product) -> bool {
        self.is_sparse_wav_product_with_count(product, crate::catalog::identities(product).len())
    }

    pub(super) fn is_sparse_wav_product_with_count(
        &self,
        product: Product,
        catalog_count: usize,
    ) -> bool {
        if self.has_original_provider(product) || !self.is_wav_product(product) {
            return false;
        }
        let found = self
            .samples
            .iter()
            .filter(|sample| Product::sample_key(sample.id).is_some_and(|(p, _)| p == product))
            .count();
        found * 2 < catalog_count
    }
}
