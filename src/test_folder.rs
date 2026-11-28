//! Test folders in the system temporary folder that remove themselves.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A new, empty folder for one test. Dropping it removes the folder and all
/// its contents, also when the test panics, so failed and stopped runs leave
/// nothing behind.
#[derive(Debug)]
pub(crate) struct TestFolder(PathBuf);

impl TestFolder {
    /// A new folder `{prefix}-{process}-{serial}` in the system temporary
    /// folder.
    pub(crate) fn new(prefix: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self::create_in(&std::env::temp_dir(), prefix, &NEXT)
    }

    /// Skip a name that is already there: it can be a folder of an earlier
    /// run with the same process number, and its files are not ours.
    fn create_in(root: &Path, prefix: &str, next: &AtomicU64) -> Self {
        loop {
            let path = root.join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                next.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("Cannot create test folder {}: {error}", path.display()),
            }
        }
    }
}

/// Tests use the folder as the path it holds; `clone` gives that path.
impl std::ops::Deref for TestFolder {
    type Target = PathBuf;

    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<Path> for TestFolder {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestFolder {
    fn drop(&mut self) {
        // The test can have removed the folder already, and a background
        // worker of the test can still hold a file open. Neither is a test
        // failure, so the result is not checked.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_folder_skips_existing_folders_and_removes_itself() {
        let base = TestFolder::new("mixjam-test-folder");
        let stale = base.join(format!("fixture-{}-0", std::process::id()));
        std::fs::create_dir(&stale).unwrap();
        std::fs::write(stale.join("sound.wav"), b"existing fixture").unwrap();

        let fresh = TestFolder::create_in(&base, "fixture", &AtomicU64::new(0));
        assert_ne!(*fresh, *stale);
        assert!(std::fs::read_dir(&*fresh).unwrap().next().is_none());
        assert_eq!(
            std::fs::read(stale.join("sound.wav")).unwrap(),
            b"existing fixture"
        );

        let path = fresh.to_path_buf();
        std::fs::write(path.join("song.mixjam"), b"saved").unwrap();
        drop(fresh);
        assert!(!path.exists());
        assert!(stale.exists());

        let path = base.to_path_buf();
        std::fs::remove_dir_all(&*base).unwrap();
        drop(base);
        assert!(!path.exists());
    }

    #[test]
    fn a_panicking_test_still_removes_its_folder() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let outcome = std::thread::spawn(move || {
            let folder = TestFolder::new("mixjam-test-folder-panic");
            sender.send(folder.to_path_buf()).unwrap();
            panic!("the test fails");
        })
        .join();
        assert!(outcome.is_err());
        assert!(!receiver.recv().unwrap().exists());
    }
}
