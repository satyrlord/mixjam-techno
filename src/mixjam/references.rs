//! Resolve preserved MixJam Sample Folder-relative paths after folder moves.
//! Every library folder of the sound browser is searched.
use crate::library::Library;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

// The measured curated/original tree contains 101,874 entries. Each scan
// covers the parent of one `_original` library folder: the bound keeps a
// missing reference from becoming an unbounded scan of another disk, far
// above any sample library that a parent folder holds.
const MAX_SCAN_ENTRIES: usize = 2_000_000;
type Index = HashMap<String, Vec<PathBuf>>;

pub(crate) struct ReferenceResolver<'a> {
    library: &'a Library,
    selected: Option<Index>,
    /// The scan of each legacy parent folder, made when a reference needs it.
    parents: HashMap<PathBuf, Result<Index, String>>,
    cache: HashMap<String, Result<Vec<PathBuf>, String>>,
}

impl<'a> ReferenceResolver<'a> {
    pub(crate) fn new(library: &'a Library) -> Self {
        Self {
            library,
            selected: None,
            parents: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn resolve(&mut self, reference: &str) -> Result<PathBuf, String> {
        self.resolve_candidates(reference)
            .map(|paths| paths[0].clone())
    }

    pub(crate) fn resolve_candidates(&mut self, reference: &str) -> Result<Vec<PathBuf>, String> {
        if let Some(result) = self.cache.get(reference) {
            return result.clone();
        }
        let result = self.resolve_uncached(reference);
        self.cache.insert(reference.into(), result.clone());
        result
    }

    /// The parent of each `_original` library folder, once, in library order.
    fn legacy_parents(&self) -> Vec<PathBuf> {
        let mut parents: Vec<PathBuf> = Vec::new();
        for root in self.library.wav_roots() {
            if let Some(parent) = legacy_parent(root)
                && !parents.iter().any(|known| known == parent)
            {
                parents.push(parent.to_path_buf());
            }
        }
        parents
    }

    fn resolve_uncached(&mut self, reference: &str) -> Result<Vec<PathBuf>, String> {
        let normalized = reference.replace('\\', "/");
        let absolute = Path::new(reference).is_absolute();
        let parts: Vec<_> = normalized
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        if reference.trim() != reference
            || reference.chars().any(char::is_control)
            || parts.is_empty()
            || parts.iter().any(|part| *part == "." || *part == "..")
            || (!absolute && (normalized.contains(':') || normalized.split('/').any(str::is_empty)))
            || !Path::new(&normalized)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        {
            return Err("The MixJam WAV reference is invalid or contains a parent folder.".into());
        }
        if absolute && regular_file(Path::new(reference)) {
            return fs::canonicalize(reference)
                .map(|path| vec![path])
                .map_err(|error| error.to_string());
        }
        let parents = self.legacy_parents();
        if !absolute {
            let relative = parts.iter().collect::<PathBuf>();
            for roots in [self.library.wav_roots(), parents.as_slice()] {
                let mut matches: Vec<_> = roots
                    .iter()
                    .filter_map(|root| exact_file(root, &relative))
                    .collect();
                matches.sort();
                matches.dedup();
                if !matches.is_empty() {
                    return Ok(matches);
                }
            }
        }
        let name = parts.last().unwrap().to_lowercase();
        let selected = self.selected.get_or_insert_with(|| {
            let mut index = Index::new();
            for sample in &self.library.samples {
                if let Some(path) = &sample.wav_path {
                    insert(&mut index, path.clone());
                }
            }
            index
        });
        // An absolute path that moved, even to another library folder,
        // matches by its longest path suffix.
        let mut candidates = selected.get(&name).cloned().unwrap_or_default();
        let mut scan_error = None;
        for root in parents {
            match self
                .parents
                .entry(root)
                .or_insert_with_key(|root| scan(root))
            {
                Ok(index) => candidates.extend(index.get(&name).into_iter().flatten().cloned()),
                // The library folders' own matches remain usable without the
                // containing-directory scan.
                Err(error) => {
                    scan_error.get_or_insert_with(|| error.clone());
                }
            }
        }
        if candidates.is_empty()
            && let Some(error) = scan_error
        {
            return Err(error);
        }
        let mut best_score = 0;
        let mut best = HashSet::new();
        for path in candidates {
            if !regular_file(&path) {
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
            if score > best_score {
                best.clear();
                best_score = score;
            }
            best.insert(fs::canonicalize(&path).map_err(|error| error.to_string())?);
        }
        let mut best: Vec<_> = best.into_iter().collect();
        best.sort();
        if best.is_empty() {
            return Err(format!(
                "MixJam WAV sample {reference} is missing from the available sample folders."
            ));
        }
        Ok(best)
    }
}

fn legacy_parent(root: &Path) -> Option<&Path> {
    if !root.file_name()?.eq_ignore_ascii_case("_original") {
        return None;
    }
    let parent = root.parent()?;
    // A drive root or UNC share root is not a curated sample directory.
    parent.parent()?;
    Some(parent)
}

fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file() && !linked(&metadata))
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn exact_file(root: &Path, relative: &Path) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if linked(&fs::symlink_metadata(&path).ok()?) {
            return None;
        }
    }
    regular_file(&path)
        .then(|| fs::canonicalize(path).ok())
        .flatten()
}

fn insert(index: &mut Index, path: PathBuf) {
    if let Some(name) = path.file_name() {
        index
            .entry(name.to_string_lossy().to_lowercase())
            .or_default()
            .push(path);
    }
}

fn scan(root: &Path) -> Result<Index, String> {
    let mut index = Index::new();
    let mut pending = vec![root.to_path_buf()];
    let mut count = 0;
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder)
            .map_err(|error| format!("Cannot read the legacy MixJam sample folder: {error}"))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            count += 1;
            if count > MAX_SCAN_ENTRIES {
                return Err(
                    "The legacy MixJam sample folder exceeds the 2,000,000-entry scan limit."
                        .into(),
                );
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if linked(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            {
                insert(&mut index, path);
            }
        }
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "mixjam-reference-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn wave(&self, relative: &str, value: f32) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            crate::audio::Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![value; 441],
            }
            .write_wav(&path)
            .unwrap();
            fs::canonicalize(path).unwrap()
        }
        fn library(&self, root: &str) -> Library {
            self.wave(&format!("{root}/fixture-seed.wav"), 0.01);
            Library::open_wav_folder(&self.0.join(root), 140.0).unwrap()
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn selected_exact_then_parent_exact_precede_relocated_files() {
        let folder = Folder::new();
        let selected = folder.wave("_original/Genre/kick.wav", 0.1);
        folder.wave("Genre/kick.wav", 0.2);
        let parent = folder.wave("Genre/snare.wav", 0.3);
        folder.wave("_original/other/snare.wav", 0.4);
        let library = folder.library("_original");
        let mut resolver = ReferenceResolver::new(&library);
        assert!(resolver.selected.is_none());
        assert_eq!(resolver.resolve("Genre/kick.wav").unwrap(), selected);
        assert_eq!(resolver.resolve("Genre/snare.wav").unwrap(), parent);
        assert!(resolver.parents.is_empty());
        assert!(resolver.selected.is_none());
    }

    #[test]
    fn moved_files_use_suffix_and_keep_all_conflicting_candidates() {
        let folder = Folder::new();
        let expected = folder.wave("New/Drum/kick.wav", 0.1);
        folder.wave("Other/kick.wav", 0.2);
        let duplicate = folder.wave("A/snare.wav", 0.3);
        folder.wave("B/snare.wav", 0.3);
        let first = folder.wave("A/clap.wav", 0.4);
        let second = folder.wave("B/clap.wav", 0.5);
        let library = folder.library("_original");
        let mut resolver = ReferenceResolver::new(&library);
        assert_eq!(resolver.resolve("Old/Drum/kick.wav").unwrap(), expected);
        assert_eq!(resolver.resolve("Old/snare.wav").unwrap(), duplicate);
        assert_eq!(resolver.resolve("Old/clap.wav").unwrap(), first);
        assert_eq!(
            resolver.resolve_candidates("Old/clap.wav").unwrap(),
            vec![first, second]
        );
        assert!(
            resolver
                .resolve("absent.wav")
                .unwrap_err()
                .contains("missing")
        );
        assert!(resolver.resolve("../A/snare.wav").is_err());
        assert!(resolver.resolve("./A/snare.wav").is_err());
        assert!(resolver.resolve("").is_err());
        assert!(!resolver.parents.is_empty());
    }

    #[test]
    fn exact_matches_in_multiple_libraries_keep_every_conflict_candidate() {
        let folder = Folder::new();
        let first = folder.wave("First/_original/Genre/kick.wav", 0.1);
        let second = folder.wave("Second/_original/Genre/kick.wav", 0.2);
        folder.wave("First/Genre/kick.wav", 0.3);
        let parent_first = folder.wave("First/Genre/snare.wav", 0.4);
        let parent_second = folder.wave("Second/Genre/snare.wav", 0.5);
        let library = Library::open_wav_folders_with(
            &[
                folder.0.join("Second/_original"),
                folder.0.join("First/_original"),
            ],
            140.0,
            None,
            None,
        )
        .unwrap();
        let mut resolver = ReferenceResolver::new(&library);
        // Exact selected-folder matches precede the parent tier. Ties use
        // stable path order, independent of the library order.
        assert_eq!(
            resolver.resolve_candidates("Genre/kick.wav").unwrap(),
            vec![first, second]
        );
        assert_eq!(
            resolver.resolve_candidates("Genre/snare.wav").unwrap(),
            vec![parent_first, parent_second]
        );
        assert!(resolver.parents.is_empty());
        assert!(resolver.selected.is_none());
    }

    #[test]
    fn references_resolve_in_every_library_folder() {
        let folder = Folder::new();
        folder.wave("First/fixture-seed.wav", 0.01);
        let relative = folder.wave("Second/Genre/hat.wav", 0.2);
        let moved = folder.wave("Second/Pack/Loops/kick.wav", 0.1);
        let library = Library::open_wav_folders_with(
            &[folder.0.join("First"), folder.0.join("Second")],
            140.0,
            None,
            None,
        )
        .unwrap();
        let mut resolver = ReferenceResolver::new(&library);
        // A folder-relative reference finds its file in the second folder.
        assert_eq!(resolver.resolve("Genre/hat.wav").unwrap(), relative);
        // An absolute path into the first folder, where the pack used to be,
        // resolves where it is now.
        let old = folder.0.join("First/Moved pack/Pack/Loops/kick.wav");
        assert_eq!(resolver.resolve(old.to_str().unwrap()).unwrap(), moved);
        // Neither folder is named _original: no parent folder is scanned.
        assert!(resolver.parents.is_empty());
    }

    #[test]
    fn parent_recovery_is_limited_to_original_layout() {
        let folder = Folder::new();
        folder.wave("Elsewhere/kick.wav", 0.1);
        let library = folder.library("Selected");
        let mut resolver = ReferenceResolver::new(&library);
        assert!(resolver.resolve("Elsewhere/kick.wav").is_err());
        assert!(resolver.parents.is_empty());
    }

    #[test]
    fn original_layout_never_scans_a_filesystem_root() {
        #[cfg(windows)]
        let roots = [
            r"E:\_original",
            r"\\server\share\_original",
            r"\\?\E:\_original",
        ];
        #[cfg(not(windows))]
        let roots = ["/_original"];
        for root in roots {
            assert!(legacy_parent(Path::new(root)).is_none(), "{root}");
        }
        let folder = Folder::new();
        assert_eq!(
            legacy_parent(&folder.0.join("_original")),
            Some(folder.0.as_path())
        );
    }

    #[test]
    fn selected_paths_work_without_folder_root_and_keep_existing_absolute_file() {
        let folder = Folder::new();
        let expected = folder.wave("Drum/kick.wav", 0.1);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        assert!(library.wav_roots().is_empty());
        let mut resolver = ReferenceResolver::new(&library);
        assert_eq!(resolver.resolve("Old/Drum/kick.wav").unwrap(), expected);
        assert_eq!(
            resolver.resolve(expected.to_str().unwrap()).unwrap(),
            expected
        );
    }
}
