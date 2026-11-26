//! Importing WAV files into a library: the folder imports, the project
//! palette that a song restores, the moved-file lookups, and the checks that
//! keep a failed import from changing the library.

use super::{
    AudioSource, Library, Sample,
    naming::{agrees_on_tempo, infer_folder_tempo, named_tempo, octave_apart, whole_bars},
    scan::{ScanCache, ScanProgress, WavListing, WavProbe, list_wav_folder, probe_all, probe_wav},
};
use crate::products::{IMPORTED_ID_START, Product};
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
        let mut paths = HashSet::new();
        let mut measured = HashMap::new();
        for sample in &self.samples {
            if let Some(AudioSource::Wave { path, origin }) = self.sources.get(&sample.id) {
                paths.insert(import_key(path, &sample.group_name, *origin));
                if sample.id >= IMPORTED_ID_START {
                    measured.insert((path.as_path(), *origin == Some(Product::T1)), sample);
                }
            }
        }
        let mut next = self.next_import_id();
        let mut additions = Vec::new();
        for sample in &palette.samples {
            if let Some((restored, metadata)) = restorable_palette_entry(
                sample,
                &palette.sources,
                &measured,
                &mut paths,
                &mut next,
            )? {
                additions.push((restored, metadata));
            }
        }
        let mut restored_metadata = Vec::with_capacity(additions.len());
        for (sample, metadata) in additions {
            self.store_import(&metadata, sample, None);
            restored_metadata.push(metadata);
        }
        Ok(restored_metadata)
    }

    fn next_import_id(&self) -> u32 {
        self.samples
            .iter()
            .map(|sample| sample.id)
            .filter(|id| *id >= IMPORTED_ID_START)
            .max()
            .unwrap_or(IMPORTED_ID_START - 1)
            .max(self.reserved_import_id)
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
        let (listing, probes) = collect_listed_probes(listing, probes)?;
        let WavListing {
            root,
            paths,
            stated,
            skipped,
        } = listing;
        let mut tempos = infer_listed_tempos(&root, &paths, &probes, source_bpm, &stated);
        let authenticated = apply_evidence_tempos(&root, &paths, &probes, evidence, &mut tempos);
        let (file_tempos, folder_tempos) =
            resolve_file_tempos(&root, &paths, &tempos, &authenticated);
        let imported =
            self.prepare_and_store_listed(&root, paths, probes, file_tempos, source_bpm)?;
        Ok((imported, folder_tempos, skipped))
    }

    fn prepare_and_store_listed(
        &mut self,
        root: &Path,
        paths: Vec<PathBuf>,
        probes: Vec<WavProbe>,
        file_tempos: Vec<Option<f64>>,
        source_bpm: f64,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        // Check every record before the first one is stored.
        let mut allocator = ImportAllocator::new(self);
        let mut prepared = Vec::with_capacity(paths.len());
        for ((path, probe), tempo) in paths.into_iter().zip(probes).zip(file_tempos) {
            let folder = path.parent().unwrap_or(root);
            let group = Some(folder)
                .filter(|parent| *parent != root)
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Wave".into());
            let (id, existing) = allocator.allocate(&path, &group)?;
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
                source_bpm: tempo.unwrap_or(source_bpm),
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
        Ok(imported)
    }

    /// Store a probed import under its ID, replacing an entry at `existing`.
    fn adopt_import(
        &mut self,
        imported: crate::engine::ImportedSample,
        source: &Path,
        probe: &WavProbe,
        existing: Option<usize>,
    ) -> Result<crate::engine::ImportedSample, String> {
        let (imported, sample) = prepare_import(imported, probe)?;
        // A Techno 1 import decodes the same bytes with the original loader.
        // Check it before changing any entry, including during a staged save.
        let (t1_aliases, ordinary_aliases) = self.wav_alias_indices(&imported.path);
        if !t1_aliases.is_empty() {
            let t1_probe = probe_wav(source, Some(Product::T1))?;
            for index in t1_aliases {
                t1_probe.refresh(&mut self.samples[index]);
            }
        }
        for index in ordinary_aliases {
            probe.refresh(&mut self.samples[index]);
        }
        self.store_import(&imported, sample, existing);
        Ok(imported)
    }

    fn wav_alias_indices(&self, path: &Path) -> (Vec<usize>, Vec<usize>) {
        self.samples
            .iter()
            .enumerate()
            .filter(|(_, sample)| {
                sample.id >= IMPORTED_ID_START && sample.wav_path.as_deref() == Some(path)
            })
            .map(|(index, _)| index)
            .partition(|index| {
                matches!(
                    self.sources.get(&self.samples[*index].id),
                    Some(AudioSource::Wave {
                        origin: Some(Product::T1),
                        ..
                    })
                )
            })
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
        let path = fs::canonicalize(path).map_err(|e| {
            format!(
                "Cannot open the imported sample {}. {}",
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy(),
                crate::atomic_file::error_text(e)
            )
        })?;
        self.import_wav_at(&path, &path, name, group, source_bpm)
    }

    /// Read the audio bytes from `source` and register its canonical final path.
    pub(crate) fn import_wav_at(
        &mut self,
        source: &Path,
        path: &Path,
        name: Option<&str>,
        group: &str,
        source_bpm: f64,
    ) -> Result<crate::engine::ImportedSample, String> {
        let (id, existing) = ImportAllocator::new(self).allocate(path, group.trim())?;
        let default_name = path.file_stem().unwrap_or_default().to_string_lossy();
        let imported = crate::engine::ImportedSample {
            id,
            name: name.unwrap_or(&default_name).trim().to_string(),
            group_name: group.trim().to_string(),
            path: path.to_path_buf(),
            source_bpm,
            source_sample_rate: None,
            origin: None,
        };
        let probe = probe_wav(source, imported.origin)?;
        self.adopt_import(imported, source, &probe, existing)
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
            let sample = self.resolve_import(imported, checked.get(&imported.id))?;
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

    fn resolve_import(
        &self,
        imported: &crate::engine::ImportedSample,
        checked: Option<&Sample>,
    ) -> Result<Sample, String> {
        if let Some(checked) = checked {
            // Resolution can change the tempo after the WAV was measured.
            let path = checked
                .wav_path
                .clone()
                .ok_or("The imported sample has no path.")?;
            return imported_sample(imported, &WavProbe::from(checked), path);
        }
        if let Some(sample) = self.unchanged_import(imported) {
            return Ok(sample.clone());
        }
        Self::check_import(imported)
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
    /// Catalog index and ID of each imported file already in the library,
    /// keyed by canonical path and category group so importing a folder sound
    /// into another group keeps the library entry intact.
    existing: HashMap<ImportKey, (usize, u32)>,
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
            if let Some(AudioSource::Wave { path, origin }) = library.sources.get(&sample.id) {
                existing.insert(
                    import_key(path, &sample.group_name, *origin),
                    (index, sample.id),
                );
            }
        }
        Self { existing, last_id }
    }

    fn allocate(&mut self, path: &Path, group: &str) -> Result<(u32, Option<usize>), String> {
        let key = import_key(path, group, None);
        if let Some(&(index, id)) = self.existing.get(&key) {
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
    Ok(Sample {
        id: imported.id,
        code: format!("IMPORT{}", imported.id),
        name: imported.name.clone(),
        bars: probe.bars_at(imported.source_bpm),
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

impl From<&Sample> for WavProbe {
    fn from(sample: &Sample) -> Self {
        Self {
            frame_count: sample.frame_count,
            sample_rate: sample.sample_rate,
            channels: sample.channels,
            length: sample.length,
            digest: sample.digest,
        }
    }
}

impl WavProbe {
    fn bars_at(&self, source_bpm: f64) -> u32 {
        let seconds = f64::from(self.frame_count) / f64::from(self.sample_rate);
        let exact = seconds * source_bpm / 240.0;
        // A loop a few frames over a whole bar is that many bars, not one more.
        let bars = if whole_bars(seconds, source_bpm).is_some() {
            exact.round()
        } else {
            exact.ceil()
        };
        (bars as u32).max(1)
    }

    fn refresh(&self, sample: &mut Sample) {
        sample.frame_count = self.frame_count;
        sample.sample_rate = self.sample_rate;
        sample.channels = self.channels;
        sample.length = self.length;
        sample.digest = self.digest;
        sample.bars = self.bars_at(sample.source_bpm);
    }
}

type ImportKey = (PathBuf, String, Option<Product>);

/// Group filters ignore ASCII case. The origin selects the product and the WAV decoder.
fn import_key(path: &Path, group: &str, origin: Option<Product>) -> ImportKey {
    (path.to_path_buf(), group.to_ascii_lowercase(), origin)
}

fn restorable_palette_entry(
    sample: &Sample,
    sources: &HashMap<u32, AudioSource>,
    measured: &HashMap<(&Path, bool), &Sample>,
    paths: &mut HashSet<ImportKey>,
    next: &mut u32,
) -> Result<Option<(Sample, crate::engine::ImportedSample)>, String> {
    if sample.id < IMPORTED_ID_START {
        return Ok(None);
    }
    let Some(AudioSource::Wave { path, origin }) = sources.get(&sample.id) else {
        return Ok(None);
    };
    if !paths.insert(import_key(path, &sample.group_name, *origin)) {
        return Ok(None);
    }
    *next = next
        .checked_add(1)
        .ok_or("Imported sample ID limit reached.")?;
    let mut restored = sample.clone();
    if let Some(&current) = measured.get(&(path.as_path(), *origin == Some(Product::T1))) {
        WavProbe::from(current).refresh(&mut restored);
    }
    restored.id = *next;
    restored.code = format!("IMPORT{}", *next);
    let metadata = crate::engine::ImportedSample {
        id: *next,
        name: restored.name.clone(),
        group_name: restored.group_name.clone(),
        path: path.clone(),
        source_bpm: restored.source_bpm,
        source_sample_rate: Some(restored.sample_rate),
        origin: *origin,
    };
    Ok(Some((restored, metadata)))
}

fn collect_listed_probes(
    mut listing: WavListing,
    probes: Vec<Result<WavProbe, String>>,
) -> Result<(WavListing, Vec<WavProbe>), String> {
    if listing.paths.is_empty() {
        return Err(listing.skipped.first().map_or_else(
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
    let (paths, probes): (Vec<_>, Vec<_>) = listing
        .paths
        .into_iter()
        .zip(probes)
        .filter_map(|(path, probe)| match probe {
            Ok(probe) => Some((path, probe)),
            Err(error) => {
                let prefix = format!("Cannot read {}: ", path.display());
                let reason = error.strip_prefix(&prefix).unwrap_or(&error).to_string();
                listing.skipped.push((path, reason));
                None
            }
        })
        .unzip();
    if paths.is_empty() {
        let (path, error) = &listing.skipped[0];
        return Err(format!(
            "The selected folder contains no readable WAV samples. {}: {error}",
            path.display()
        ));
    }
    listing.paths = paths;
    Ok((listing, probes))
}

fn infer_listed_tempos(
    root: &Path,
    paths: &[PathBuf],
    probes: &[WavProbe],
    source_bpm: f64,
    stated: &BTreeMap<PathBuf, f64>,
) -> BTreeMap<PathBuf, f64> {
    // Loop packs keep one tempo per folder, and their files are whole bars long.
    let mut by_folder: BTreeMap<PathBuf, Vec<(u64, u32)>> = BTreeMap::new();
    for (path, probe) in paths.iter().zip(probes) {
        by_folder
            .entry(path.parent().unwrap_or(root).to_path_buf())
            .or_default()
            .push((u64::from(probe.frame_count), probe.sample_rate));
    }
    let mut tempos: BTreeMap<PathBuf, f64> = by_folder
        .iter()
        .filter_map(|(folder, durations)| {
            Some((folder.clone(), infer_folder_tempo(durations, source_bpm)?))
        })
        .collect();
    promote_octave_tempos(&by_folder, &mut tempos, source_bpm);
    apply_stated_pack_tempos(root, stated, &by_folder, &mut tempos);
    tempos
}

fn octave_promotion(durations: &[(u64, u32)], tempo: f64, source_bpm: f64) -> Option<f64> {
    let octave = if tempo >= 176.0 {
        tempo / 2.0
    } else {
        tempo * 2.0
    };
    let usable = !(88.0..176.0).contains(&tempo)
        && tempo != source_bpm
        && (88.0..176.0).contains(&octave)
        && agrees_on_tempo(durations, octave);
    usable.then_some(octave)
}

/// Short files can put one of a pack's folders an octave away from the others;
/// bring it back when its loops allow.
fn promote_octave_tempos(
    by_folder: &BTreeMap<PathBuf, Vec<(u64, u32)>>,
    tempos: &mut BTreeMap<PathBuf, f64>,
    source_bpm: f64,
) {
    let settled = tempos.clone();
    for (folder, tempo) in tempos {
        let Some(octave) = octave_promotion(&by_folder[folder], *tempo, source_bpm) else {
            continue;
        };
        if settled
            .iter()
            .any(|(sibling, other)| sibling.parent() == folder.parent() && *other == octave)
        {
            *tempo = octave;
        }
    }
}

fn pack_stated<'a>(
    root: &Path,
    stated: &'a BTreeMap<PathBuf, f64>,
    folder: &Path,
) -> Option<(&'a PathBuf, &'a f64)> {
    folder
        .ancestors()
        .take_while(|ancestor| ancestor.starts_with(root))
        .find_map(|ancestor| stated.get_key_value(ancestor))
}

fn collect_pack_octaves<'a>(
    root: &Path,
    stated: &'a BTreeMap<PathBuf, f64>,
    by_folder: &BTreeMap<PathBuf, Vec<(u64, u32)>>,
    tempos: &BTreeMap<PathBuf, f64>,
) -> BTreeMap<&'a Path, BTreeMap<u64, usize>> {
    let mut octaves: BTreeMap<&Path, BTreeMap<u64, usize>> = BTreeMap::new();
    for (folder, durations) in by_folder {
        let Some((pack, stated_bpm)) = pack_stated(root, stated, folder) else {
            continue;
        };
        if let Some(tempo) = tempos.get(folder)
            && octave_apart(*tempo, *stated_bpm)
        {
            *octaves
                .entry(pack)
                .or_default()
                .entry(tempo.to_bits())
                .or_default() += durations.len();
        }
    }
    octaves
}

/// A pack's BPM.INF states the tempo of every folder below it. Keep readings an
/// octave away and give untimed folders the settled octave.
fn apply_stated_pack_tempos(
    root: &Path,
    stated: &BTreeMap<PathBuf, f64>,
    by_folder: &BTreeMap<PathBuf, Vec<(u64, u32)>>,
    tempos: &mut BTreeMap<PathBuf, f64>,
) {
    let octaves = collect_pack_octaves(root, stated, by_folder, tempos);
    for folder in by_folder.keys() {
        let Some((pack, stated_bpm)) = pack_stated(root, stated, folder) else {
            continue;
        };
        if tempos
            .get(folder)
            .is_some_and(|tempo| octave_apart(*tempo, *stated_bpm))
        {
            continue;
        }
        let settled = octaves
            .get(pack.as_path())
            .and_then(|counts| counts.iter().max_by_key(|(_, count)| **count))
            .map_or(*stated_bpm, |(bits, _)| f64::from_bits(*bits));
        tempos.insert(folder.clone(), settled);
    }
}

/// Authenticated original audio resolves duration-octave ambiguity.
fn apply_evidence_tempos(
    root: &Path,
    paths: &[PathBuf],
    probes: &[WavProbe],
    evidence: &crate::source_tempo::SourceTempos,
    tempos: &mut BTreeMap<PathBuf, f64>,
) -> HashSet<PathBuf> {
    let mut folder_probes: BTreeMap<&Path, Vec<&WavProbe>> = BTreeMap::new();
    for (path, probe) in paths.iter().zip(probes) {
        folder_probes
            .entry(path.parent().unwrap_or(root))
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
    authenticated
}

type FolderTempoCount = (PathBuf, f64, usize);

fn resolve_file_tempos(
    root: &Path,
    paths: &[PathBuf],
    tempos: &BTreeMap<PathBuf, f64>,
    authenticated: &HashSet<PathBuf>,
) -> (Vec<Option<f64>>, Vec<FolderTempoCount>) {
    let file_tempos: Vec<Option<f64>> = paths
        .iter()
        .map(|path| {
            let folder = path.parent().unwrap_or(root);
            let inferred = tempos.get(folder).copied();
            if authenticated.contains(folder) {
                inferred
            } else {
                named_tempo(path).or(inferred)
            }
        })
        .collect();
    let mut counted: BTreeMap<(&Path, u64), usize> = BTreeMap::new();
    for (path, tempo) in paths.iter().zip(&file_tempos) {
        if let Some(tempo) = tempo {
            *counted
                .entry((path.parent().unwrap_or(root), tempo.to_bits()))
                .or_default() += 1;
        }
    }
    let folder_tempos = counted
        .into_iter()
        .map(|((folder, bits), count)| (folder.to_path_buf(), f64::from_bits(bits), count))
        .collect();
    (file_tempos, folder_tempos)
}

pub(super) fn unquote(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(text)
}
