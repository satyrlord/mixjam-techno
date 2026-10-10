//! Resolve old-project assets only through their retained hash baseline.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MANIFEST_BYTE_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Record {
    path: String,
    bytes: u64,
    sha256: String,
}

/// Keep other paths unchanged. Old-project paths require an authenticated copy.
pub fn resolve_asset(path: &Path) -> Result<PathBuf, String> {
    let Some(base) = std::env::var_os("MIXJAM_TECHNO_LEGACY_SOURCE_ROOT").map(PathBuf::from) else {
        return Ok(path.to_path_buf());
    };
    let old_roots = [
        "\x72ave-rea",
        "ravefold",
        "\x65jay-mix-reader",
        "mixjam-electron",
    ]
    .map(|name| (name, base.join(name)));
    resolve_asset_in(
        path,
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("local-data/legacy"),
        &old_roots,
    )
}

fn resolve_asset_in(
    path: &Path,
    archive_root: &Path,
    old_roots: &[(&str, PathBuf)],
) -> Result<PathBuf, String> {
    let Some(input) = normalized_path(path) else {
        return Ok(path.to_path_buf());
    };
    for (name, old_root) in old_roots {
        let root = normalized_path(old_root).ok_or("The old project path is not Unicode text.")?;
        let root = root.trim_end_matches('/');
        let Some(prefix) = input.get(..root.len()) else {
            continue;
        };
        if !prefix.eq_ignore_ascii_case(root)
            || !matches!(input.as_bytes().get(root.len()), None | Some(b'/'))
        {
            continue;
        }
        let relative = input[root.len()..].trim_start_matches('/');
        let input_parts = relative_parts(relative)?;
        let relative_key = input_parts.join("/").to_lowercase();
        let manifest_path = archive_root.join(format!("{name}-manifest.json"));
        let file = fs::File::open(&manifest_path)
            .map_err(|e| format!("Cannot read the {name} preservation manifest: {e}"))?;
        let mut bytes = Vec::new();
        file.take(MANIFEST_BYTE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MANIFEST_BYTE_LIMIT {
            return Err("The preservation manifest exceeds its size limit.".into());
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        let manifest: Manifest = serde_json::from_slice(bytes)
            .map_err(|e| format!("Cannot parse the {name} preservation manifest: {e}"))?;
        let mut matched = None;
        for record in &manifest.records {
            let parts = relative_parts(&record.path)?;
            if parts.join("/").to_lowercase() == relative_key
                && matched.replace((record, parts)).is_some()
            {
                return Err("The preservation manifest has duplicate asset paths.".into());
            }
        }
        let (record, parts) = matched.ok_or_else(|| {
            format!("The preserved asset is not in the {name} hash manifest: {relative}.")
        })?;
        if record.sha256.len() != 64 || !record.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("The preservation manifest has an invalid SHA256 value.".into());
        }
        let snapshot = archive_root.join(name);
        let archived = parts
            .iter()
            .fold(snapshot.clone(), |path, part| path.join(part));
        let canonical_snapshot = fs::canonicalize(&snapshot)
            .map_err(|e| format!("Cannot open the {name} preservation snapshot: {e}"))?;
        let canonical_asset = fs::canonicalize(&archived)
            .map_err(|e| format!("Cannot open preserved asset {relative}: {e}"))?;
        if !canonical_asset.starts_with(&canonical_snapshot) {
            return Err("The preserved asset is outside its project snapshot.".into());
        }
        authenticate(&canonical_asset, record, "preserved")?;
        let original = parts
            .iter()
            .fold(old_root.clone(), |path, part| path.join(part));
        match fs::metadata(&original) {
            Ok(_) => authenticate(&original, record, "original")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Cannot inspect the original asset {relative}: {e}")),
        }
        return Ok(canonical_asset);
    }
    Ok(path.to_path_buf())
}

fn normalized_path(path: &Path) -> Option<String> {
    let text = path.to_str()?.replace('\\', "/");
    Some(text.strip_prefix("//?/").unwrap_or(&text).to_owned())
}

fn relative_parts(path: &str) -> Result<Vec<&str>, String> {
    if path.is_empty() || path.starts_with(['/', '\\']) || path.contains([':', '\0']) {
        return Err("The preserved asset needs a relative path within its project.".into());
    }
    let parts: Vec<_> = path.split(['/', '\\']).collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || matches!(*part, "." | "..") || part.ends_with(['.', ' ']))
    {
        return Err("The preserved asset path contains an invalid path component.".into());
    }
    Ok(parts)
}

fn authenticate(path: &Path, record: &Record, label: &str) -> Result<(), String> {
    let mut file = fs::File::open(path)
        .map_err(|e| format!("Cannot read the {label} asset {}: {e}", record.path))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() != record.bytes {
        return Err(format!(
            "The {label} asset differs from its preservation manifest: {}.",
            record.path
        ));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut bytes_read = 0u64;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes_read += count as u64;
        hash.update(&buffer[..count]);
    }
    if bytes_read != record.bytes
        || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(&record.sha256)
    {
        return Err(format!(
            "The {label} asset differs from its preservation manifest: {}.",
            record.path
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const CONTENT: &[u8] = b"retained source audio";

    struct Fixture {
        root: PathBuf,
        archive: PathBuf,
        original: PathBuf,
        asset: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "mixjam-techno-preservation-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let archive = root.join("archive");
            let original = root.join("old/ravefold");
            let asset = archive.join("ravefold/Samples/voice.wav");
            fs::create_dir_all(asset.parent().unwrap()).unwrap();
            fs::create_dir_all(original.join("Samples")).unwrap();
            fs::write(&asset, CONTENT).unwrap();
            fs::write(original.join("Samples/voice.wav"), CONTENT).unwrap();
            fs::write(
                archive.join("ravefold-manifest.json"),
                serde_json::to_vec(&serde_json::json!({"records": [{
                    "path": "Samples/voice.wav", "bytes": CONTENT.len(),
                    "sha256": format!("{:x}", Sha256::digest(CONTENT))
                }]}))
                .unwrap(),
            )
            .unwrap();
            Self {
                root,
                archive,
                original,
                asset,
            }
        }

        fn resolve(&self, path: &Path) -> Result<PathBuf, String> {
            resolve_asset_in(path, &self.archive, &[("ravefold", self.original.clone())])
        }

        fn source(&self) -> PathBuf {
            self.original.join("Samples/voice.wav")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
            let root = fs::canonicalize(&self.root).unwrap();
            assert!(root.starts_with(&temp) && root != temp);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn canonical_mixed_case_and_slash_paths_use_the_same_authenticated_asset() {
        let fixture = Fixture::new();
        let expected = fs::canonicalize(&fixture.asset).unwrap();
        let canonical = fs::canonicalize(fixture.source()).unwrap();
        assert_eq!(fixture.resolve(&canonical).unwrap(), expected);
        let mixed = fixture
            .source()
            .to_str()
            .unwrap()
            .to_uppercase()
            .replace('\\', "/");
        assert_eq!(fixture.resolve(Path::new(&mixed)).unwrap(), expected);
    }

    #[test]
    fn absent_original_uses_the_manifest_and_tampering_does_not_replace_it() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.source()).unwrap();
        assert_eq!(
            fixture.resolve(&fixture.source()).unwrap(),
            fs::canonicalize(&fixture.asset).unwrap()
        );
        fs::write(&fixture.asset, b"different audio bytes").unwrap();
        assert!(fixture.resolve(&fixture.source()).is_err());
        fs::write(&fixture.asset, CONTENT).unwrap();
        assert!(
            fixture.resolve(&fixture.source()).is_ok(),
            "A previous error must not remain cached."
        );
    }

    #[test]
    fn original_and_archive_must_each_match_the_earlier_baseline() {
        let fixture = Fixture::new();
        fs::write(fixture.source(), b"different audio bytes").unwrap();
        assert!(
            fixture
                .resolve(&fixture.source())
                .unwrap_err()
                .contains("original asset")
        );
        fs::write(&fixture.asset, b"different audio bytes").unwrap();
        assert!(
            fixture
                .resolve(&fixture.source())
                .unwrap_err()
                .contains("preserved asset")
        );
    }

    #[test]
    fn adjacent_roots_are_unchanged_and_traversal_is_rejected() {
        let fixture = Fixture::new();
        let adjacent = fixture
            .original
            .with_file_name("ravefold-extra")
            .join("Samples/voice.wav");
        assert_eq!(fixture.resolve(&adjacent).unwrap(), adjacent);
        let relative = Path::new("ravefold/Samples/voice.wav");
        assert_eq!(fixture.resolve(relative).unwrap(), relative);
        for relative in [
            "../escape.wav",
            "Samples/../voice.wav",
            "Samples/voice.wav.",
        ] {
            assert!(fixture.resolve(&fixture.original.join(relative)).is_err());
        }
    }

    #[test]
    fn unregistered_assets_and_manifest_traversal_are_rejected() {
        let fixture = Fixture::new();
        assert!(
            fixture
                .resolve(&fixture.original.join("missing.wav"))
                .is_err()
        );
        fs::write(
            fixture.archive.join("ravefold-manifest.json"),
            br#"{"records":[{"path":"../escape.wav","bytes":0,"sha256":""}]}"#,
        )
        .unwrap();
        assert!(fixture.resolve(&fixture.source()).is_err());
    }
}
