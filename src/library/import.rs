//! Importing WAV files into a library: the folder imports, the project
//! palette that a song restores, the moved-file lookups, and the checks that
//! keep a failed import from changing the library.

use super::{
    AudioSource, Library, Sample,
    naming::{agrees_on_tempo, infer_folder_tempo, named_tempo, octave_apart, whole_bars},
    scan::{ScanCache, ScanProgress, WavListing, WavProbe, list_wav_folder, probe_all, probe_wav},
};
use crate::products::IMPORTED_ID_START;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

impl Library {
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

    /// Load a WAV-only folder without an original installation or a sidecar index.
    /// The caller supplies a fallback tempo for files without an authenticated,
    /// stated, or duration-derived tempo.
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
    /// Unreadable WAVs are skipped and reported. A failed or cancelled scan
    /// leaves this library unchanged.
    pub fn import_wav_folder_with(
        &mut self,
        root: &Path,
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        cache: Option<&mut ScanCache>,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        self.import_wav_folder_using(
            root,
            source_bpm,
            progress,
            cache,
            crate::source_tempo::SourceTempos::for_catalogs(crate::catalog::catalogs()),
        )
    }

    pub(super) fn import_wav_folder_using(
        &mut self,
        root: &Path,
        source_bpm: f64,
        progress: Option<&ScanProgress>,
        mut cache: Option<&mut ScanCache>,
        evidence: &crate::source_tempo::SourceTempos,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("The WAV folder needs a finite positive source tempo.".into());
        }
        let listing = list_wav_folder(root, progress)?;
        if let Some(cache) = cache.as_deref_mut() {
            cache.forget_unlisted(std::slice::from_ref(&listing));
        }
        let probes = probe_all(&listing.paths, progress, cache)?;
        let (imported, tempos, skipped) =
            self.import_listing(listing, probes, source_bpm, evidence)?;
        self.folder_tempos = tempos;
        self.skipped_waves = skipped;
        Ok(imported)
    }

    /// Import the probed WAVs of one listed folder, in listing order. Returns
    /// the imports, the folder tempos, and the WAVs that could not be read.
    /// An error leaves this library unchanged.
    #[allow(clippy::type_complexity)]
    pub(super) fn import_listing(
        &mut self,
        listing: WavListing,
        probes: Vec<Result<WavProbe, String>>,
        source_bpm: f64,
        evidence: &crate::source_tempo::SourceTempos,
    ) -> Result<
        (
            Vec<crate::engine::ImportedSample>,
            Vec<(PathBuf, f64, usize)>,
            Vec<(PathBuf, String)>,
        ),
        String,
    > {
        let WavListing {
            root,
            paths,
            stated,
            skipped,
        } = listing;
        if paths.is_empty() {
            return Err(skipped.first().map_or_else(
                || "The selected folder contains no WAV samples.".into(),
                |(path, error)| {
                    format!(
                        "The selected folder has no readable WAV samples. {}: {error}",
                        path.display()
                    )
                },
            ));
        }
        // An unreadable file is reported and left out; the rest of the folder loads.
        let mut skipped = skipped;
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
        // Check every record before the first one is stored.
        let mut allocator = ImportAllocator::new(self);
        let mut prepared = Vec::with_capacity(paths.len());
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
            let (record, sample) = prepare_import(record, &probe)?;
            prepared.push((record, sample, existing));
        }
        let mut imported = Vec::with_capacity(prepared.len());
        for (record, sample, existing) in prepared {
            self.store_import(&record, sample, existing);
            imported.push(record);
        }
        Ok((imported, folder_tempos, skipped))
    }

    /// Store a probed import under its ID, replacing an entry at `existing`.
    fn adopt_import(
        &mut self,
        imported: crate::engine::ImportedSample,
        probe: &WavProbe,
        existing: Option<usize>,
    ) -> Result<crate::engine::ImportedSample, String> {
        let (imported, sample) = prepare_import(imported, probe)?;
        self.store_import(&imported, sample, existing);
        Ok(imported)
    }

    /// Store a checked import under its ID, replacing an entry at `existing`.
    fn store_import(
        &mut self,
        imported: &crate::engine::ImportedSample,
        sample: Sample,
        existing: Option<usize>,
    ) {
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
        for path in self.wav_paths() {
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
        self.reindex();
        Ok(())
    }
}

/// Set an import's probed rate and build its catalog entry.
fn prepare_import(
    mut imported: crate::engine::ImportedSample,
    probe: &WavProbe,
) -> Result<(crate::engine::ImportedSample, Sample), String> {
    imported.source_sample_rate = Some(probe.sample_rate);
    let sample = imported_sample(&imported, probe, imported.path.clone())?;
    Ok((imported, sample))
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

pub(super) fn unquote(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(text)
}
