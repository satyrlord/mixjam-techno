//! Scanning sample folders: the folder list checks, the WAV listing and probing
//! passes, the progress counters, and the probe cache that a rescan reuses.

use super::{MAX_LIBRARY_FOLDERS, naming::stated_pack_tempo, pcm::read_t1_wave};
use crate::products::Product;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Check a list of library folders: 1 to [`MAX_LIBRARY_FOLDERS`] folders,
/// none the same as or inside another. Existing ancestors are canonicalized
/// so unavailable folders compare with available roots and absolute aliases.
pub fn check_library_folders(roots: &[PathBuf]) -> Result<(), String> {
    if roots.is_empty() {
        return Err("Choose at least one library folder.".into());
    }
    if roots.len() > MAX_LIBRARY_FOLDERS {
        return Err(format!(
            "The Sounds panel holds up to {MAX_LIBRARY_FOLDERS} library folders."
        ));
    }
    let resolved: Vec<PathBuf> = roots
        .iter()
        .map(|root| library_folder_identity(root))
        .collect::<Result<_, _>>()?;
    for (index, root) in resolved.iter().enumerate() {
        for other in &resolved[index + 1..] {
            if root == other {
                return Err(format!(
                    "{} is already a library folder.",
                    roots[index].display()
                ));
            }
            let (inner, outer) = if other.starts_with(root) {
                (other, root)
            } else if root.starts_with(other) {
                (root, other)
            } else {
                continue;
            };
            return Err(format!(
                "{} is inside the library folder {}. Choose folders that do not contain each other.",
                inner.display(),
                outer.display()
            ));
        }
    }
    Ok(())
}

fn library_folder_identity(root: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(root)
        .map_err(|error| format!("Cannot resolve {}: {error}", root.display()))?;
    for ancestor in absolute.ancestors() {
        if let Ok(canonical) = fs::canonicalize(ancestor) {
            return Ok(canonical.join(absolute.strip_prefix(ancestor).unwrap()));
        }
    }
    Ok(absolute)
}

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
    #[serde(default)]
    decoder_version: u8,
    length: u64,
    modified: u128,
    probe: WavProbe,
}

impl ScanCache {
    /// The probes that the cache keeps. Past it the cache starts over, so a
    /// scan never fails; the cost is one more probe of each file. A cache
    /// file of this size is already far past any real library.
    const MAX_ENTRIES: usize = 1_000_000;

    // Advance this when decoding changes sample values or admitted layouts.
    const DECODER_VERSION: u8 = 1;

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn has_changes(&self) -> bool {
        self.changed
    }

    /// Drop the probes of files below the listed folders that their
    /// complete listings no longer have, so that moved or deleted files do
    /// not fill the cache. Probes of other folders stay.
    pub(super) fn forget_unlisted(&mut self, listings: &[WavListing]) {
        // Keys are the listed paths' text: compare text, once per entry.
        let roots: Vec<String> = listings
            .iter()
            .filter_map(|listing| listing.root.to_str())
            .map(|root| root.trim_end_matches(std::path::MAIN_SEPARATOR).to_owned())
            .collect();
        let listed: HashSet<&str> = listings
            .iter()
            .flat_map(|listing| &listing.paths)
            .filter_map(|path| path.to_str())
            .collect();
        let below = |key: &str| {
            roots.iter().any(|root| {
                key.strip_prefix(root.as_str())
                    .is_some_and(|rest| rest.starts_with(std::path::MAIN_SEPARATOR))
            })
        };
        let before = self.entries.len();
        self.entries
            .retain(|key, _| listed.contains(key.as_str()) || !below(key));
        self.changed |= self.entries.len() != before;
    }

    pub(super) fn stamp(path: &Path) -> Option<(u64, u128)> {
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
        (entry.decoder_version == Self::DECODER_VERSION
            && entry.length == stamp.0
            && entry.modified == stamp.1
            && probe.frame_count > 0
            && (1..=384_000).contains(&probe.sample_rate)
            && (1..=2).contains(&probe.channels))
        .then_some(probe)
    }

    pub(super) fn insert(&mut self, path: &Path, stamp: (u64, u128), probe: WavProbe) {
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
                decoder_version: Self::DECODER_VERSION,
                length: stamp.0,
                modified: stamp.1,
                probe,
            },
        );
    }
}

/// The WAVs below one library folder, sorted, and the pack tempos that its
/// `BPM.INF` files state.
pub(super) struct WavListing {
    pub(super) root: PathBuf,
    pub(super) paths: Vec<PathBuf>,
    pub(super) stated: BTreeMap<PathBuf, f64>,
    /// Subfolders and entries that could not be read, with the reason.
    pub(super) skipped: Vec<(PathBuf, String)>,
}

/// List the WAVs below `root`. An unreadable `root` fails the listing; an
/// unreadable subfolder or entry is skipped and reported, as a drive's system
/// folders are, so that the rest of the library loads.
pub(super) fn list_wav_folder(
    root: &Path,
    progress: Option<&ScanProgress>,
) -> Result<WavListing, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("Cannot open the WAV folder: {e}"))?;
    if !root.is_dir() {
        return Err("Select a folder that contains WAV samples.".into());
    }
    let cancelled = || progress.is_some_and(ScanProgress::is_cancelled);
    let mut pending = vec![(root.clone(), 0usize)];
    let mut visited = HashSet::new();
    let mut paths = Vec::new();
    let mut stated = BTreeMap::new();
    let mut skipped = Vec::new();
    while let Some((folder, depth)) = pending.pop() {
        if !visited.insert(folder.clone()) {
            continue;
        }
        if depth > MAX_FOLDER_DEPTH {
            return Err("The WAV folder exceeds the 64-level folder limit.".into());
        }
        if cancelled() {
            return Err(SCAN_CANCELLED.into());
        }
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(e) if folder == root => {
                return Err(format!("Cannot read {}: {e}", folder.display()));
            }
            Err(e) => {
                skipped.push((folder.clone(), format!("Cannot read the folder: {e}")));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    skipped.push((folder.clone(), format!("Cannot read the folder: {e}")));
                    continue;
                }
            };
            // The listing gives each entry's type. Only a link needs the
            // file system again, to find and check its target. Opening
            // every entry made a cached scan of 100,000 files 3 times slower.
            let mut path = entry.path();
            let kind = entry.file_type().and_then(|kind| {
                if !kind.is_symlink() {
                    return Ok(Some(kind));
                }
                path = fs::canonicalize(&path)?;
                // A link that leaves the selected folder is refused below.
                if !path.starts_with(&root) {
                    return Ok(None);
                }
                Ok(Some(fs::metadata(&path)?.file_type()))
            });
            let kind = match kind {
                Ok(Some(kind)) => kind,
                Ok(None) => {
                    return Err("A WAV folder link points outside the selected folder.".into());
                }
                Err(e) => {
                    skipped.push((entry.path(), e.to_string()));
                    continue;
                }
            };
            if kind.is_dir() {
                pending.push((path, depth + 1));
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
                // macOS copies leave `._NAME.wav` metadata beside each file; it holds no audio.
                && !entry.file_name().to_string_lossy().starts_with("._")
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
    Ok(WavListing {
        root,
        paths,
        stated,
        skipped,
    })
}

/// How deep a sample folder may nest. The walk follows folder links, and
/// `visited` holds the paths as they were reached, so a link that loops back
/// makes ever longer paths; this stops it. Real libraries nest a few levels.
const MAX_FOLDER_DEPTH: usize = 64;

pub(super) const SCAN_CANCELLED: &str = "The sample folder scan was cancelled.";

/// Decode one WAV at a canonical path: its length, format, and audio digest.
pub(super) fn probe_wav(path: &Path, origin: Option<Product>) -> Result<WavProbe, String> {
    let length = fs::metadata(path).map_err(|e| e.to_string())?.len();
    // A WAV is read as a stream, so its length bounds the work and the
    // format bounds its length. Only the original file loader's 2 GiB limit
    // applies to the first product's files; `read_t1_wave` enforces it.
    let mut identity = crate::catalog::PcmIdentity::default();
    let (sample_rate, channels, frame_count) = if origin == Some(Product::T1) {
        let pcm = read_t1_wave(path)?;
        for &sample in &pcm.samples {
            identity.push((sample * 32768.0).round() as i16);
        }
        let frames = u32::try_from(pcm.frames())
            .map_err(|_| "The WAV sample has too many frames.".to_string())?;
        (pcm.sample_rate, pcm.channels, frames)
    } else {
        let (spec, frames) = crate::audio::visit_wav_samples(path, |spec, sample| {
            if spec.channels == 1 && spec.sample_rate == legacy_format::NATIVE_SAMPLE_RATE {
                identity.push((sample * 32768.0).round() as i16);
            }
        })?;
        (spec.sample_rate, spec.channels, frames)
    };
    if frame_count == 0 {
        return Err("The imported sample contains no audio.".into());
    }
    let digest =
        (channels == 1 && sample_rate == legacy_format::NATIVE_SAMPLE_RATE && frame_count > 2)
            .then(|| identity.finish());
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
pub(super) fn probe_all(
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
