//! Sample identity catalogs: runtime data files in the application's
//! `assets/catalog` folder, one file per release of an original product. A
//! file names its product and release, lists the size and SHA-256 of each
//! original file it was generated from, and gives the identity of every
//! sample. A catalog holds numbers, digests, archive codes and generic
//! categories only: no original names, file paths or audio. Nothing here is
//! compiled into the executable.
use super::WaveMetadata;
use crate::products::Product;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// The catalog folder under the application's `assets`.
pub const FOLDER: &str = "catalog";
/// The file of extra source tempos in the catalog folder. It is not a catalog.
pub const SOURCE_TEMPOS: &str = "source-tempos.json";
/// The catalog file format that this application reads and writes.
pub const SCHEMA: u32 = 3;
const RELEASE_NAME_LIMIT: usize = 64;

/// One original file that a catalog was generated from, by size and content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub bytes: u64,
    pub sha256: String,
}

/// The saved form of one catalog file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFile {
    pub schema: u32,
    pub product: Product,
    /// A short name of the release, chosen when the catalog is generated.
    pub release: String,
    /// The [`fingerprint`] of `sources`.
    pub fingerprint: String,
    pub sources: Vec<SourceFile>,
    pub samples: Vec<WaveMetadata>,
}

/// Identify a release by its original files: SHA-256 over each file's
/// SHA-256 (in lower case) and size, in that order. The same files give the
/// same fingerprint, whatever their order in the catalog.
pub fn fingerprint(sources: &[SourceFile]) -> String {
    let mut files: Vec<_> = sources
        .iter()
        .map(|file| (file.sha256.to_ascii_lowercase(), file.bytes))
        .collect();
    files.sort();
    let mut digest = Sha256::new();
    for (sha256, bytes) in files {
        digest.update(sha256.as_bytes());
        digest.update(bytes.to_le_bytes());
        digest.update(b"\n");
    }
    format!("{:x}", digest.finalize())
}

/// The catalog folder's JSON files as last seen: name, size and time.
type Stamp = Vec<(String, u64, Option<SystemTime>)>;

fn stamp(folder: &Path) -> Stamp {
    let mut files: Stamp = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata().ok()?;
            (metadata.is_file() && name.to_ascii_lowercase().ends_with(".json"))
                .then(|| (name, metadata.len(), metadata.modified().ok()))
        })
        .collect();
    files.sort();
    files
}

/// A loaded catalog file.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Release {
    pub product: Product,
    pub release: String,
    pub fingerprint: String,
    /// The file name in the catalog folder.
    pub file: String,
    pub samples: usize,
}

impl Release {
    pub fn describe(&self) -> String {
        format!(
            "{} release \"{}\" ({})",
            self.product.label(),
            self.release,
            self.file
        )
    }
}

/// The catalogs of one folder. Releases of one product merge into one list of
/// identities when they agree on every native ID they share. A release with
/// conflicting audio or timing is not loaded and is listed as a problem.
#[derive(Debug, Default)]
pub struct Catalogs {
    folder: Option<PathBuf>,
    stamp: Stamp,
    releases: Vec<Release>,
    identities: [Vec<WaveMetadata>; Product::ALL.len()],
    /// Alternative names from compatible releases, keyed by native ID.
    aliases: [BTreeMap<u32, Vec<WaveMetadata>>; Product::ALL.len()],
    source_tempos: Vec<([u8; 16], f64)>,
    problems: Vec<(String, String)>,
}

fn slot(product: Product) -> usize {
    Product::ALL
        .iter()
        .position(|known| *known == product)
        .expect("Product::ALL lists every product.")
}

/// Whether two releases identify the same sound and timing under one native ID.
fn same_sound(a: &WaveMetadata, b: &WaveMetadata) -> bool {
    a.frame_count == b.frame_count
        && a.bars == b.bars
        && a.source_bpm == b.source_bpm
        && match (a.pcm_identity(), b.pcm_identity()) {
            (Some(a), Some(b)) => a == b,
            _ => a.code.eq_ignore_ascii_case(&b.code),
        }
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Catalogs {
    /// The catalogs of `folder`. `None` means that there is no catalog folder.
    pub fn open(folder: Option<PathBuf>) -> Self {
        let mut catalogs = Self {
            folder,
            ..Default::default()
        };
        catalogs.load();
        catalogs
    }

    fn load(&mut self) {
        let Some(folder) = self.folder.clone() else {
            return;
        };
        self.stamp = stamp(&folder);
        let listing = match std::fs::read_dir(&folder) {
            Ok(listing) => listing,
            Err(error) => {
                self.problems.push((
                    folder.display().to_string(),
                    format!("Cannot read the catalog folder: {error}"),
                ));
                return;
            }
        };
        let mut files: Vec<PathBuf> = listing
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                    && path.is_file()
            })
            .collect();
        files.sort();
        for path in files {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let loaded = if name.eq_ignore_ascii_case(SOURCE_TEMPOS) {
                read_source_tempos(&path).map(|tempos| self.source_tempos = tempos)
            } else {
                read_catalog(&path).and_then(|file| self.merge(&name, file))
            };
            if let Err(error) = loaded {
                self.problems.push((name, error));
            }
        }
    }

    fn merge(&mut self, name: &str, file: CatalogFile) -> Result<(), String> {
        if let Some(same) = self.releases.iter().find(|release| {
            release.product == file.product && release.fingerprint == file.fingerprint
        }) {
            return Err(format!(
                "It describes the same release as {}, which is already loaded.",
                same.file
            ));
        }
        let identities = &mut self.identities[slot(file.product)];
        let mut conflicts = Vec::new();
        let samples = file.samples.len();
        for entry in &file.samples {
            if let Ok(index) =
                identities.binary_search_by_key(&entry.native_id, |known| known.native_id)
                && !same_sound(&identities[index], entry)
            {
                conflicts.push(entry.native_id);
            }
        }
        if let Some(first) = conflicts.first() {
            let loaded: Vec<_> = self
                .releases
                .iter()
                .filter(|release| release.product == file.product)
                .map(|release| release.file.as_str())
                .collect();
            return Err(format!(
                "Its {} release \"{}\" has different audio or timing than {} under {} (first: {first}). Keep \
                 the catalog of only one of these releases.",
                file.product.label(),
                file.release,
                loaded.join(", "),
                count(conflicts.len(), "native ID", "native IDs"),
            ));
        }
        let aliases = &mut self.aliases[slot(file.product)];
        let mut added = Vec::new();
        for entry in file.samples {
            match identities.binary_search_by_key(&entry.native_id, |known| known.native_id) {
                Ok(index) => {
                    let known = &mut identities[index];
                    if known.pcm_digest.is_none() {
                        known.pcm_digest.clone_from(&entry.pcm_digest);
                    }
                    if !known.code.eq_ignore_ascii_case(&entry.code)
                        || known.label_key != entry.label_key
                        || !known.group_name.eq_ignore_ascii_case(&entry.group_name)
                    {
                        aliases.entry(entry.native_id).or_default().push(entry);
                    }
                }
                Err(_) => added.push(entry),
            }
        }
        identities.extend(added);
        identities.sort_by_key(|entry| entry.native_id);
        self.releases.push(Release {
            product: file.product,
            release: file.release,
            fingerprint: file.fingerprint,
            file: name.to_owned(),
            samples,
        });
        Ok(())
    }

    /// The catalog folder, or `None` when the application has none.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Whether a JSON file of the folder was added, changed or removed since
    /// the folder was read.
    pub fn changed_on_disk(&self) -> bool {
        self.folder
            .as_deref()
            .is_some_and(|folder| stamp(folder) != self.stamp)
    }

    /// The loaded releases, in file name order.
    pub fn releases(&self) -> &[Release] {
        &self.releases
    }

    /// The identities of every loaded release of `product`, by native ID.
    pub fn identities(&self, product: Product) -> &[WaveMetadata] {
        &self.identities[slot(product)]
    }

    pub fn identity(&self, product: Product, native_id: u32) -> Option<&WaveMetadata> {
        let entries = self.identities(product);
        entries
            .binary_search_by_key(&native_id, |entry| entry.native_id)
            .ok()
            .map(|index| &entries[index])
    }

    /// Other names for an identity from compatible releases.
    pub(crate) fn aliases(&self, product: Product, native_id: u32) -> &[WaveMetadata] {
        self.aliases[slot(product)]
            .get(&native_id)
            .map_or(&[], Vec::as_slice)
    }

    /// Tempos of other original sounds, by audio digest, from `source-tempos.json`.
    pub fn source_tempos(&self) -> &[([u8; 16], f64)] {
        &self.source_tempos
    }

    /// Files that did not load, each with the reason.
    pub fn problems(&self) -> &[(String, String)] {
        &self.problems
    }

    fn location(&self) -> String {
        self.folder.as_ref().map_or_else(
            || format!("an assets/{FOLDER} folder beside the application"),
            |folder| folder.display().to_string(),
        )
    }

    /// One notice for each product whose catalogs do not list some of a
    /// song's `missing` native samples: the release that saved the song has
    /// no catalog here, so those sounds cannot be identified at all.
    pub fn missing_notices(
        &self,
        missing: impl IntoIterator<Item = (Product, u32)>,
    ) -> Vec<String> {
        let mut unknown: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        for (product, native_id) in missing {
            if self.identity(product, native_id).is_none() {
                unknown.entry(slot(product)).or_default().insert(native_id);
            }
        }
        unknown
            .into_iter()
            .map(|(index, ids)| {
                let product = Product::ALL[index];
                let label = product.label();
                let n = ids.len();
                let releases: Vec<_> = self
                    .releases
                    .iter()
                    .filter(|release| release.product == product)
                    .map(Release::describe)
                    .collect();
                if releases.is_empty() {
                    format!(
                        "At import, no {label} sample catalog was loaded for this song's {label} \
                         sounds ({}). If sounds remain missing, add the {label} catalog to {} \
                         and open the song again.",
                        count(n, "native ID", "native IDs"),
                        self.location()
                    )
                } else {
                    format!(
                        "At import, {} {} absent from the loaded catalogs ({}). The song may \
                         come from a {label} release without a catalog here. If sounds remain \
                         missing, add that release's catalog to {} and open the song again.",
                        count(
                            n,
                            &format!("{label} native ID"),
                            &format!("{label} native IDs")
                        ),
                        if n == 1 { "was" } else { "were" },
                        releases.join(", "),
                        self.location()
                    )
                }
            })
            .collect()
    }
}

fn read_catalog(path: &Path) -> Result<CatalogFile, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    if text.trim_start().starts_with('[') {
        return Err(
            "This catalog has the earlier format, without its product and release. \
             Generate it again."
                .into(),
        );
    }
    let schema = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| value.get("schema")?.as_u64());
    if schema == Some(2) {
        return Err(
            "This catalog has format 2, which holds the original sound names and file paths. \
             Generate it again."
                .into(),
        );
    }
    let mut file: CatalogFile = serde_json::from_str(&text)
        .map_err(|error| format!("This is not a sample catalog: {error}"))?;
    if file.schema != SCHEMA {
        return Err(format!(
            "Catalog format {} is not supported; this application reads format {SCHEMA}.",
            file.schema
        ));
    }
    let release = file.release.trim();
    if release.is_empty()
        || release.chars().count() > RELEASE_NAME_LIMIT
        || release.chars().any(char::is_control)
    {
        return Err(format!(
            "The release needs a name of 1 to {RELEASE_NAME_LIMIT} characters."
        ));
    }
    file.release = release.to_owned();
    if file.sources.is_empty() {
        return Err("The catalog names no original source files.".into());
    }
    for source in &file.sources {
        if source.sha256.len() != 64 || !source.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("A catalog source SHA-256 is invalid.".into());
        }
    }
    if !file
        .fingerprint
        .eq_ignore_ascii_case(&fingerprint(&file.sources))
    {
        return Err("The fingerprint does not match the listed source files.".into());
    }
    file.fingerprint.make_ascii_lowercase();
    if file.samples.is_empty() {
        return Err("The catalog has no sample identities.".into());
    }
    let mut ids = HashSet::new();
    for entry in &file.samples {
        let valid = file.product.global_sample_id(entry.native_id).is_ok()
            && entry.bars > 0
            && entry.frame_count > 0
            && entry.source_bpm.is_finite()
            && entry.source_bpm > 0.0
            && entry.label_key.len() == 16
            && entry
                .label_key
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            && !entry.code.trim().is_empty()
            && (entry.pcm_digest.is_none() || entry.pcm_identity().is_some());
        if !valid {
            return Err(format!(
                "Native sample {} has invalid identity data.",
                entry.native_id
            ));
        }
        if !ids.insert(entry.native_id) {
            return Err(format!(
                "Native sample {} is listed twice.",
                entry.native_id
            ));
        }
    }
    file.samples.sort_by_key(|entry| entry.native_id);
    Ok(file)
}

fn read_source_tempos(path: &Path) -> Result<Vec<([u8; 16], f64)>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let tempos: Vec<([u8; 16], f64)> = serde_json::from_str(&text)
        .map_err(|error| format!("Cannot read the source tempos: {error}"))?;
    if tempos
        .iter()
        .any(|(_, bpm)| !bpm.is_finite() || *bpm <= 0.0)
    {
        return Err("A source tempo is not a finite positive tempo.".into());
    }
    Ok(tempos)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Folder(PathBuf);

    impl Folder {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("mixjam-catalogs-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, text: &str) {
            std::fs::write(self.0.join(name), text).unwrap();
        }

        fn catalog(&self, name: &str, file: &CatalogFile) {
            self.write(name, &serde_json::to_string(file).unwrap());
        }

        fn open(&self) -> Catalogs {
            Catalogs::open(Some(self.0.clone()))
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample(native_id: u32, audio: i16) -> WaveMetadata {
        WaveMetadata {
            native_id,
            code: format!("CODE{native_id}"),
            label_key: super::super::label_key(&format!("Sound {native_id}")),
            group_name: "Bass".into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 8,
            pcm_digest: Some(super::super::pcm_identity_hex(&[
                0, audio, 0, audio, 0, 1, 2, 3,
            ])),
        }
    }

    fn release(
        product: Product,
        name: &str,
        source: &str,
        samples: Vec<WaveMetadata>,
    ) -> CatalogFile {
        let sources = vec![SourceFile {
            bytes: 12,
            sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        }];
        CatalogFile {
            schema: SCHEMA,
            product,
            release: name.into(),
            fingerprint: fingerprint(&sources),
            sources,
            samples,
        }
    }

    #[test]
    fn releases_that_agree_merge_and_a_conflicting_release_is_not_loaded() {
        let folder = Folder::new("merge");
        // The download edition lists the CD edition's sounds and adds kits.
        folder.catalog(
            "d1-cd.json",
            &release(
                Product::D1,
                "cd",
                "cd",
                vec![sample(731, 1), sample(732, 2)],
            ),
        );
        folder.catalog(
            "d1-download.json",
            &release(
                Product::D1,
                "download",
                "download",
                vec![sample(3400, 9), sample(731, 1), sample(732, 2)],
            ),
        );
        folder.catalog(
            "d1-other.json",
            &release(
                Product::D1,
                "other",
                "other",
                vec![sample(731, 5), sample(9000, 6)],
            ),
        );
        folder.catalog(
            "t2.json",
            &release(Product::T2, "download", "t2", vec![sample(1, 1)]),
        );
        let catalogs = folder.open();
        let ids: Vec<_> = catalogs
            .identities(Product::D1)
            .iter()
            .map(|entry| entry.native_id)
            .collect();
        assert_eq!(ids, [731, 732, 3400]);
        assert_eq!(
            catalogs.identity(Product::D1, 3400).unwrap().label_key,
            super::super::label_key("Sound 3400")
        );
        assert!(catalogs.identity(Product::D1, 9000).is_none());
        assert_eq!(catalogs.identities(Product::T2).len(), 1);
        assert!(catalogs.identities(Product::H1).is_empty());
        assert_eq!(
            catalogs
                .releases()
                .iter()
                .map(|release| (release.file.as_str(), release.samples))
                .collect::<Vec<_>>(),
            [("d1-cd.json", 2), ("d1-download.json", 3), ("t2.json", 1)]
        );
        let [(file, reason)] = catalogs.problems() else {
            panic!("{:?}", catalogs.problems());
        };
        assert_eq!(file, "d1-other.json");
        assert!(
            reason.contains("than d1-cd.json, d1-download.json under 1 native ID"),
            "{reason}"
        );
        assert!(reason.contains("first: 731"), "{reason}");
    }

    #[test]
    fn shared_id_timing_conflicts_reject_the_whole_later_release_in_either_order() {
        for (name, changed) in [
            (
                "bars",
                WaveMetadata {
                    bars: 2,
                    ..sample(731, 1)
                },
            ),
            (
                "bpm",
                WaveMetadata {
                    source_bpm: 90.0,
                    ..sample(731, 1)
                },
            ),
        ] {
            for reverse in [false, true] {
                let folder = Folder::new(&format!("timing-{name}-{reverse}"));
                let first = release(Product::D1, "a", "a", vec![sample(731, 1)]);
                let second = release(Product::D1, "b", "b", vec![changed.clone(), sample(732, 2)]);
                let (a, b) = if reverse {
                    (&second, &first)
                } else {
                    (&first, &second)
                };
                folder.catalog("a.json", a);
                folder.catalog("b.json", b);
                let catalogs = folder.open();
                assert_eq!(catalogs.releases().len(), 1);
                assert_eq!(catalogs.problems().len(), 1);
                assert!(catalogs.problems()[0].1.contains("audio or timing"));
                if !reverse {
                    assert!(catalogs.identity(Product::D1, 732).is_none());
                }
            }
        }
    }

    #[test]
    fn compatible_releases_preserve_a_later_digest_and_other_names() {
        let folder = Folder::new("stronger-digest");
        let mut without_digest = sample(731, 1);
        without_digest.pcm_digest = None;
        let mut known = sample(731, 1);
        known.label_key = super::super::label_key("Other label");
        folder.catalog(
            "a.json",
            &release(Product::D1, "a", "a", vec![without_digest]),
        );
        folder.catalog(
            "b.json",
            &release(Product::D1, "b", "b", vec![known.clone()]),
        );
        let mut renamed = known.clone();
        renamed.code = "OTHER-CODE".into();
        folder.catalog("c.json", &release(Product::D1, "c", "c", vec![renamed]));
        let catalogs = folder.open();
        assert!(catalogs.problems().is_empty(), "{:?}", catalogs.problems());
        assert_eq!(
            catalogs.identity(Product::D1, 731).unwrap().pcm_identity(),
            known.pcm_identity()
        );
        assert_eq!(catalogs.aliases(Product::D1, 731).len(), 2);
        assert_eq!(catalogs.aliases(Product::D1, 731)[1].code, "OTHER-CODE");
    }

    #[test]
    fn invalid_source_hashes_and_label_keys_reject_even_with_matching_fingerprints() {
        let folder = Folder::new("bad-sources");
        let mut hash = release(Product::D1, "a", "a", vec![sample(731, 1)]);
        hash.sources[0].sha256 = "00".into();
        hash.fingerprint = fingerprint(&hash.sources);
        folder.catalog("hash.json", &hash);
        for (index, key) in ["", "0123456789ABCDEF", "Grp. 1 Vers1", "0123456789abcdef0"]
            .into_iter()
            .enumerate()
        {
            let mut entry = sample(731, 1);
            entry.label_key = key.into();
            folder.catalog(
                &format!("key-{index}.json"),
                &release(Product::D1, "a", &format!("key-{index}"), vec![entry]),
            );
        }
        let catalogs = folder.open();
        assert!(catalogs.releases().is_empty());
        assert_eq!(catalogs.problems().len(), 5);
        for (name, reason) in catalogs.problems() {
            assert!(reason.contains(if name == "hash.json" {
                "SHA-256"
            } else {
                "invalid identity data"
            }));
        }
    }

    #[test]
    fn a_format_2_catalog_with_names_and_paths_is_refused() {
        let folder = Folder::new("format-2");
        folder.write(
            "d1-download.json",
            r#"{"schema":2,"product":"d1","release":"download","fingerprint":"x",
                "sources":[{"path":"index.inf","bytes":1,"sha256":"00"}],
                "samples":[{"native_id":731,"code":"bm\\asjo.snd","name":"Grp. 1 Vers1",
                "group_name":"loop","bars":2,"source_bpm":140.0,"frame_count":151200}]}"#,
        );
        let catalogs = folder.open();
        assert!(catalogs.releases().is_empty());
        assert!(catalogs.problems()[0].1.contains("format 2"));
        assert!(catalogs.problems()[0].1.contains("Generate it again"));
    }

    #[test]
    fn a_folder_change_is_seen_without_reading_the_catalogs_again() {
        let folder = Folder::new("changes");
        folder.catalog(
            "d1.json",
            &release(Product::D1, "download", "d1", vec![sample(731, 1)]),
        );
        let catalogs = folder.open();
        assert!(!catalogs.changed_on_disk());
        folder.catalog(
            "d2.json",
            &release(Product::D2, "download", "d2", vec![sample(5, 1)]),
        );
        assert!(catalogs.changed_on_disk());
        let again = folder.open();
        assert!(!again.changed_on_disk());
        assert_eq!(again.identities(Product::D2).len(), 1);
        std::fs::remove_file(folder.0.join("d1.json")).unwrap();
        assert!(again.changed_on_disk());
        assert!(!Catalogs::open(None).changed_on_disk());
    }

    #[test]
    fn invalid_files_are_listed_and_the_others_load() {
        let folder = Folder::new("invalid");
        let good = release(Product::H1, "cd", "house", vec![sample(1, 1)]);
        folder.catalog("h1.json", &good);
        folder.catalog("h1-copy.json", &good);
        folder.write("d2.json", "[]");
        folder.write("provenance.json", r#"{"method":"x","products":[]}"#);
        let mut tampered = release(Product::D3, "a", "d3", vec![sample(1, 1)]);
        tampered.sources[0].bytes = 13;
        folder.catalog("d3.json", &tampered);
        let mut twice = release(Product::D4, "a", "d4", vec![sample(1, 1), sample(1, 2)]);
        folder.catalog("d4.json", &twice);
        twice.samples = vec![WaveMetadata {
            bars: 0,
            ..sample(1, 1)
        }];
        folder.catalog("d4-bars.json", &twice);
        let mut unnamed = release(Product::T3, " ", "t3", vec![sample(1, 1)]);
        folder.catalog("t3.json", &unnamed);
        unnamed.release = "a".into();
        unnamed.schema = 4;
        folder.catalog("t3-future.json", &unnamed);
        let catalogs = folder.open();
        assert_eq!(catalogs.releases().len(), 1);
        assert_eq!(catalogs.releases()[0].file, "h1-copy.json");
        let reasons: BTreeMap<_, _> = catalogs.problems().iter().cloned().collect();
        assert!(reasons["h1.json"].contains("same release as h1-copy.json"));
        assert!(reasons["d2.json"].contains("earlier format"));
        assert!(reasons["provenance.json"].contains("not a sample catalog"));
        assert!(reasons["d3.json"].contains("fingerprint"));
        assert!(reasons["d4.json"].contains("listed twice"));
        assert!(reasons["d4-bars.json"].contains("invalid identity data"));
        assert!(reasons["t3.json"].contains("needs a name"));
        assert!(reasons["t3-future.json"].contains("format 4 is not supported"));
        assert_eq!(reasons.len(), 8);
        let absent = Catalogs::open(Some(folder.0.join("absent")));
        assert!(
            absent.problems()[0]
                .1
                .contains("Cannot read the catalog folder")
        );
        assert!(Catalogs::open(None).problems().is_empty());
    }

    #[test]
    fn source_tempos_are_read_beside_the_catalogs_and_checked() {
        let folder = Folder::new("tempos");
        folder.write(SOURCE_TEMPOS, "[[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],0.0]]");
        let invalid = folder.open();
        assert!(invalid.source_tempos().is_empty());
        assert!(invalid.problems()[0].1.contains("finite positive"));
        folder.write(
            SOURCE_TEMPOS,
            "[[[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16],90.0]]",
        );
        let valid = folder.open();
        assert_eq!(
            valid.source_tempos(),
            [(
                [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                90.0
            )]
        );
        assert!(valid.problems().is_empty() && valid.releases().is_empty());
    }

    #[test]
    fn the_fingerprint_follows_the_source_files_not_their_order() {
        let files = |order: [(u64, &str); 2]| {
            order
                .map(|(bytes, sha256)| SourceFile {
                    bytes,
                    sha256: sha256.into(),
                })
                .to_vec()
        };
        let a = fingerprint(&files([(4, "AB"), (9, "cd")]));
        assert_eq!(a, fingerprint(&files([(9, "CD"), (4, "ab")])));
        assert_ne!(a, fingerprint(&files([(9, "cd"), (5, "ab")])));
        assert_ne!(a, fingerprint(&files([(9, "cd"), (4, "ef")])));
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn missing_notices_name_the_product_and_the_loaded_releases() {
        let folder = Folder::new("notices");
        folder.catalog(
            "d2.json",
            &release(Product::D2, "download", "d2", vec![sample(5, 1)]),
        );
        let catalogs = folder.open();
        let notices = catalogs.missing_notices([
            (Product::D2, 5),
            (Product::D2, 6),
            (Product::D2, 7),
            (Product::D2, 7),
            (Product::H1, 1),
        ]);
        assert_eq!(notices.len(), 2);
        assert!(notices[0].starts_with("At import, 2 D2 native IDs were absent"));
        assert!(notices[0].contains("D2 release \"download\" (d2.json)"));
        assert!(notices[0].contains(&folder.0.display().to_string()));
        assert!(notices[1].starts_with(
            "At import, no H1 sample catalog was loaded for this song's H1 sounds (1 native ID)"
        ));
        assert!(
            notices
                .iter()
                .all(|notice| notice.contains("If sounds remain missing"))
        );
        assert!(catalogs.missing_notices([(Product::D2, 5)]).is_empty());
        let none = Catalogs::open(None).missing_notices([(Product::T1, 1)]);
        assert!(none[0].contains("an assets/catalog folder beside the application"));
    }
}
