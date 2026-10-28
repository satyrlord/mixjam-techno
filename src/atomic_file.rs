//! Replace a file only after its complete new content is on disk.
//!
//! Every writer stages a uniquely named sibling file and renames it over the
//! destination. A single-file replacement is atomic. A batch keeps backups
//! until every replacement succeeds and attempts rollback on an error. A
//! process interruption can leave batch backups for manual recovery.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
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
    write_many(&[path.to_path_buf()], |files| fill(&mut files[0]))
}

/// Stage one file for each path through `fill`, then replace each path with
/// its file. Until every file is complete, no destination changes.
pub(crate) fn write_many(
    paths: &[PathBuf],
    fill: impl FnOnce(&mut [fs::File]) -> Result<(), String>,
) -> Result<(), String> {
    check_destinations(paths)?;
    let mut staged = Vec::with_capacity(paths.len());
    let mut files = Vec::with_capacity(paths.len());
    let mut result = Ok(());
    for path in paths {
        match sibling(path, "tmp") {
            Ok((stage, file)) => {
                staged.push(stage);
                files.push(file);
            }
            Err(error) => {
                result = Err(error);
                break;
            }
        }
    }
    let result = result
        .and_then(|()| fill(&mut files))
        .and_then(|()| {
            files
                .iter()
                .try_for_each(|file| file.sync_all().map_err(|error| error.to_string()))
        })
        .and_then(|()| {
            drop(files);
            commit_staged(paths, &staged)
        });
    if result.is_err() {
        for stage in &staged {
            let _ = fs::remove_file(stage);
        }
    }
    result
}

/// Reserve a sibling name before using it, including backup names.
fn sibling(path: &Path, extension: &str) -> Result<(PathBuf, fs::File), String> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().ok_or("Select a file name.")?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let sibling = path.with_file_name(format!(
        ".{}.{}-{stamp}-{}.{extension}",
        name.to_string_lossy(),
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&sibling)
        .map_err(|error| error.to_string())?;
    Ok((sibling, file))
}

/// Reject aliases and non-file destinations before any rendering or writes.
fn check_destinations(paths: &[PathBuf]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!("{} is not a regular file.", path.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let name = path.file_name().ok_or("Select a file name.")?;
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
        let canonical = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(_) => fs::canonicalize(parent.unwrap_or(Path::new(".")))
                .map_err(|error| error.to_string())?
                .join(name),
        };
        #[cfg(windows)]
        let canonical = canonical.to_string_lossy().to_lowercase();
        if !seen.insert(canonical) {
            return Err(format!(
                "The destination {} occurs more than once.",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Commit a completed batch. Backups remain recoverable if rollback fails.
fn commit_staged(paths: &[PathBuf], staged: &[PathBuf]) -> Result<(), String> {
    if paths.len() == 1 {
        return fs::rename(&staged[0], &paths[0]).map_err(|error| error.to_string());
    }
    let mut backups = vec![None; paths.len()];
    let mut committed = 0;
    let result = (|| {
        for (index, path) in paths.iter().enumerate() {
            match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.is_file() => {
                    let (backup, reservation) = sibling(path, "bak")?;
                    drop(reservation);
                    if let Err(error) = fs::rename(path, &backup) {
                        let _ = fs::remove_file(&backup);
                        return Err(error.to_string());
                    }
                    backups[index] = Some(backup);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => return Err(format!("{} is not a regular file.", path.display())),
                Err(error) => return Err(error.to_string()),
            }
        }
        for (stage, path) in staged.iter().zip(paths) {
            fs::rename(stage, path).map_err(|error| error.to_string())?;
            committed += 1;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let recovery = restore_originals(paths, &backups, committed);
        return Err(if recovery.is_empty() {
            error
        } else {
            format!("{error} {}", recovery.join(" "))
        });
    }
    for backup in backups.into_iter().flatten() {
        if let Err(error) = fs::remove_file(&backup) {
            return Err(format!(
                "All files were written, but the backup {} could not be removed: {error}",
                backup.display()
            ));
        }
    }
    Ok(())
}

fn restore_originals(
    paths: &[PathBuf],
    backups: &[Option<PathBuf>],
    committed: usize,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, (path, backup)) in paths.iter().zip(backups).enumerate().rev() {
        if let Some(backup) = backup {
            if let Err(error) = fs::rename(backup, path) {
                errors.push(format!(
                    "Cannot restore {}: {error}. The original file remains at {}.",
                    path.display(),
                    backup.display()
                ));
            }
        } else if index < committed
            && let Err(error) = fs::remove_file(path)
        {
            errors.push(format!(
                "Cannot remove the new file {}: {error}.",
                path.display()
            ));
        }
    }
    errors
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

    #[test]
    fn a_failed_batch_keeps_every_original_destination() {
        let folder = folder("failed-batch");
        let paths = [folder.join("first.wav"), folder.join("second.wav")];
        fs::write(&paths[0], b"original").unwrap();
        fs::create_dir(&paths[1]).unwrap();
        assert!(
            write_many(&paths, |files| {
                for file in files {
                    file.write_all(b"replacement").unwrap();
                }
                Ok(())
            })
            .is_err()
        );
        assert_eq!(fs::read(&paths[0]).unwrap(), b"original");
        assert!(paths[1].is_dir());
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn duplicate_batch_destinations_are_rejected_before_fill() {
        let folder = folder("duplicate-batch");
        let path = folder.join("song.wav");
        fs::write(&path, b"original").unwrap();
        let alias = folder.join(".").join("song.wav");
        let error = write_many(&[path.clone(), alias], |_| {
            panic!("Invalid batch reached fill")
        })
        .unwrap_err();
        assert!(error.contains("more than once"), "{error}");
        assert_eq!(fs::read(path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_commit_failure_restores_replaced_files_and_removes_new_files() {
        let folder = folder("rollback");
        let paths = [
            folder.join("old.wav"),
            folder.join("new.wav"),
            folder.join("last.wav"),
        ];
        fs::write(&paths[0], b"first original").unwrap();
        fs::write(&paths[2], b"last original").unwrap();
        let stages = [
            folder.join("stage1"),
            folder.join("stage2"),
            folder.join("absent-stage"),
        ];
        fs::write(&stages[0], b"first replacement").unwrap();
        fs::write(&stages[1], b"new file").unwrap();
        // The missing final stage forces failure after two successful commits.
        assert!(commit_staged(&paths, &stages).is_err());
        assert_eq!(fs::read(&paths[0]).unwrap(), b"first original");
        assert!(!paths[1].exists());
        assert_eq!(fs::read(&paths[2]).unwrap(), b"last original");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn failed_rollback_retains_the_original_and_reports_its_backup_path() {
        let folder = folder("rollback-recovery");
        let path = folder.join("song.wav");
        let backup = folder.join("saved.bak");
        fs::write(&backup, b"original").unwrap();
        // A destination replaced by a directory cannot receive the saved file.
        fs::create_dir(&path).unwrap();
        let errors = restore_originals(std::slice::from_ref(&path), &[Some(backup.clone())], 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains(&backup.display().to_string()));
        assert_eq!(fs::read(backup).unwrap(), b"original");
        assert!(path.is_dir());
        fs::remove_dir_all(folder).unwrap();
    }
}
