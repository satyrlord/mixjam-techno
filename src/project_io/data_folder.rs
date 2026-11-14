//! The application's data folder and the settings files it holds.
use crate::{
    library::Library,
    products::{LibraryBinding, Product},
};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn data_root() -> PathBuf {
    std::env::var_os("MIXJAM_TECHNO_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("MixJam Techno"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("mixjam-techno"))
}

/// The library folders that the Sounds panel merges, in order, and the
/// fallback tempo of their scans. Stored in `sample-folder.json`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct WaveFolderConfig {
    pub roots: Vec<PathBuf>,
    pub source_bpm: f64,
}

/// The largest `sample-folder.json`: room for dozens of folder paths. The file
/// is a setting, and the bound stops a file that is not one from being read.
const MAX_WAVE_CONFIG_BYTES: u64 = 16 * 1024;

/// `sample-folder.json` as written by every version: earlier versions wrote
/// one `root`, later ones a `roots` list.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredWaveConfig {
    root: Option<PathBuf>,
    roots: Option<Vec<PathBuf>>,
    source_bpm: f64,
}

pub fn load_wave_config() -> Result<Option<WaveFolderConfig>, String> {
    load_wave_config_in(&data_root())
}

/// Read the library folders. A file with the earlier single `root` reads as
/// a list of that one folder; the next save writes the list.
pub fn load_wave_config_in(data_root: &Path) -> Result<Option<WaveFolderConfig>, String> {
    let path = data_root.join("sample-folder.json");
    if !path.exists() {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_WAVE_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WAVE_CONFIG_BYTES {
        return Err("The sample folder settings exceed the size limit.".into());
    }
    let stored: StoredWaveConfig = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read sample folder settings: {e}"))?;
    let roots = match (stored.root, stored.roots) {
        (Some(root), None) => vec![root],
        (None, Some(roots)) => roots,
        _ => {
            return Err(
                "The sample folder settings need either one root or a list of roots.".into(),
            );
        }
    };
    if roots.iter().any(|root| !root.is_absolute())
        || !stored.source_bpm.is_finite()
        || stored.source_bpm <= 0.0
    {
        return Err(
            "The sample folder settings need absolute paths and a positive source tempo.".into(),
        );
    }
    crate::library::check_library_folders(&roots)
        .map_err(|error| format!("Cannot use the sample folder settings: {error}"))?;
    Ok(Some(WaveFolderConfig {
        roots,
        source_bpm: stored.source_bpm,
    }))
}

/// The probe cache of earlier folder scans. A missing, oversized, or
/// unreadable cache is an empty one: scans then decode every file again.
pub fn load_scan_cache(data_root: &Path) -> crate::library::ScanCache {
    let path = data_root.join("wav-scan-cache.json");
    let Ok(file) = fs::File::open(path) else {
        return Default::default();
    };
    let mut bytes = Vec::new();
    // About 260 bytes an entry: room for every entry that the cache keeps.
    const LIMIT: u64 = 1024 * 1024 * 1024;
    if file.take(LIMIT + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > LIMIT {
        return Default::default();
    }
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_scan_cache(data_root: &Path, cache: &crate::library::ScanCache) -> Result<(), String> {
    let bytes = serde_json::to_vec(cache).map_err(|e| e.to_string())?;
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("wav-scan-cache.json"), &bytes)
}

/// Open library folders through the probe cache in `data_root`. The probes
/// of a failed or cancelled scan are kept for the next one; a cache that
/// cannot be written only costs the next scan time.
pub fn open_wav_folders_cached(
    data_root: &Path,
    roots: &[PathBuf],
    source_bpm: f64,
    progress: Option<&crate::library::ScanProgress>,
) -> Result<Library, String> {
    let mut cache = load_scan_cache(data_root);
    let result = Library::open_wav_folders_with(roots, source_bpm, progress, Some(&mut cache));
    if cache.has_changes() {
        let _ = save_scan_cache(data_root, &cache);
    }
    result
}

/// Save the library folders, in order. A folder that exists is saved as its
/// canonical path; one that is unavailable now keeps its absolute path.
pub fn save_wave_config_in(
    data_root: &Path,
    roots: &[PathBuf],
    source_bpm: f64,
) -> Result<(), String> {
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    crate::library::check_library_folders(roots)?;
    let roots = roots
        .iter()
        .map(|root| {
            fs::canonicalize(root)
                .or_else(|_| std::path::absolute(root))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::library::check_library_folders(&roots)?;
    let bytes = serde_json::to_vec_pretty(&WaveFolderConfig { roots, source_bpm })
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WAVE_CONFIG_BYTES {
        return Err("The sample folder settings exceed the size limit.".into());
    }
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("sample-folder.json"), &bytes)
}

/// The folder of the song last chosen in a file dialog, while that folder exists.
pub fn load_song_folder(data_root: &Path) -> Option<PathBuf> {
    let file = fs::File::open(data_root.join("song-folder.json")).ok()?;
    let mut bytes = Vec::new();
    // One folder path: a settings file that is longer is not one.
    file.take(1024 * 1024).read_to_end(&mut bytes).ok()?;
    let folder: PathBuf = serde_json::from_slice(&bytes).ok()?;
    folder.is_dir().then_some(folder)
}

pub fn save_song_folder(data_root: &Path, folder: &Path) -> Result<(), String> {
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(folder).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("song-folder.json"), &bytes)
}

/// Whether the arrangement shows section bookmarks. They show until the user
/// hides them; an unreadable setting shows them.
pub fn load_section_bookmarks(data_root: &Path) -> bool {
    let Ok(file) = fs::File::open(data_root.join("section-bookmarks.json")) else {
        return true;
    };
    let mut bytes = Vec::new();
    if file.take(1024).read_to_end(&mut bytes).is_err() {
        return true;
    }
    serde_json::from_slice(&bytes).unwrap_or(true)
}

/// The setting's earlier file, which only the migration test still writes.
#[cfg(test)]
pub(crate) fn save_section_bookmarks(data_root: &Path, visible: bool) -> Result<(), String> {
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&visible).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("section-bookmarks.json"), &bytes)
}

/// An optional folder of extracted WAVs with a `metadata.json`: `samples`
/// beside the executable, or the folder that `MIXJAM_TECHNO_REFERENCE_SAMPLES`
/// names. A provider registers without it.
pub fn reference_samples_root() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("samples")))
        .unwrap_or_default();
    [
        Some(beside.clone()),
        std::env::var_os("MIXJAM_TECHNO_REFERENCE_SAMPLES").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.join("metadata.json").is_file())
    .unwrap_or(beside)
}

pub fn load_library_config(data_root: &Path) -> Result<Vec<LibraryBinding>, String> {
    // A few hundred bytes a library: a file past a megabyte is not the settings.
    const LIMIT: u64 = 1024 * 1024;
    let file = match fs::File::open(data_root.join("libraries.json")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Cannot read the library settings: {error}")),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read the library settings: {error}"))?;
    if bytes.len() as u64 > LIMIT {
        return Err("The library settings exceed the 1 MiB limit.".into());
    }
    let bindings: Vec<LibraryBinding> = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read the library settings: {e}"))?;
    validate_library_config(&bindings)?;
    Ok(bindings)
}

fn validate_library_config(bindings: &[LibraryBinding]) -> Result<(), String> {
    let mut products = HashSet::new();
    if bindings.len() > Product::ALL.len()
        || bindings
            .iter()
            .any(|b| !products.insert(b.product) || !b.root.is_absolute())
    {
        return Err("Library settings require distinct products and absolute folder paths.".into());
    }
    Ok(())
}

pub fn save_library_config(data_root: &Path, bindings: &[LibraryBinding]) -> Result<(), String> {
    validate_library_config(bindings)?;
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(bindings).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("libraries.json"), &bytes)
}

/// Original installations for the ignored fixture tests. The application has no default provider.
#[cfg(test)]
pub(crate) fn default_root(product: Product) -> Option<PathBuf> {
    let installations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("local-data/installations");
    let media = std::env::var_os("MIXJAM_TECHNO_MEDIA_ROOT").map(PathBuf::from);
    let (local, setup) = match product {
        Product::D1 => ("\x64ance1", "\x65jay_\x64ance1_setup"),
        Product::D2 => ("\x64ance2", "\x65jay_\x64ance2_setup"),
        Product::D3 => ("\x64ance3", "\x65jay_\x64ance3_setup"),
        Product::D4 => ("\x64ance4", "\x65jay_\x64ance4_setup"),
        Product::H1 => ("\x68ouse", "\x65jay_\x68ouse_setup"),
        Product::T1 => ("\x72ave", "\x65jay_\x74echno1_setup"),
        Product::T2 => ("\x74echno2", "\x65jay_\x54echno2_setup"),
        Product::T3 => ("\x74echno3", "\x65jay_\x74echno3_setup"),
    };
    [
        Some(installations.join(local)),
        media.map(|media| media.join(setup)),
    ]
    .into_iter()
    .flatten()
    .find(|root| root.is_dir())
}
