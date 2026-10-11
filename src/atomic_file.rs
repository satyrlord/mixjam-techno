//! Replace a file only after its complete new content is on disk.
//!
//! Every writer stages a uniquely named sibling file and renames it over the
//! destination. A failed or interrupted write leaves the destination intact,
//! and a staging file left by a killed process never blocks a later write.
use std::{
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_with(path, |file| {
        file.write_all(bytes).map_err(|error| error.to_string())
    })
}

/// Stage the file through `fill`, then replace `path` with it.
pub(crate) fn write_with(
    path: &Path,
    fill: impl FnOnce(&mut fs::File) -> Result<(), String>,
) -> Result<(), String> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or("Select a file name.")?
        .to_string_lossy();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let staged = path.with_file_name(format!(
        ".{name}.{}-{stamp}-{}.tmp",
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)
        .map_err(|error| error.to_string())?;
    let result = fill(&mut file)
        .and_then(|()| file.sync_all().map_err(|error| error.to_string()))
        .and_then(|()| {
            drop(file);
            fs::rename(&staged, path).map_err(|error| error.to_string())
        });
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "mixjam-atomic-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn a_failed_write_keeps_the_destination_and_leaves_no_staging_file() {
        let folder = folder("failed");
        let path = folder.join("song.json");
        fs::write(&path, b"saved").unwrap();
        assert!(
            write_with(&path, |file| {
                file.write_all(b"partial").unwrap();
                Err("The disk is full.".into())
            })
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"saved");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_write_replaces_the_destination_beside_unrelated_staging_files() {
        let folder = folder("replaced");
        let path = folder.join("song.json");
        let stale = folder.join(format!(".song.json.{}.tmp", std::process::id()));
        fs::write(&path, b"old").unwrap();
        fs::write(&stale, b"left by a killed process").unwrap();
        write(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read(&stale).unwrap(), b"left by a killed process");
        fs::remove_dir_all(folder).unwrap();
    }
}
