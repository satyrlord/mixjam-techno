mod commands;
mod frontend;
mod history;
mod library_scan;
mod saving;

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
fn fixture_dir() -> PathBuf {
    fresh_fixture_dir(&std::env::temp_dir(), &NEXT_FIXTURE)
}

fn fresh_fixture_dir(root: &std::path::Path, next: &AtomicU64) -> PathBuf {
    loop {
        let path = root.join(format!(
            "mixjam-session-{}-{}",
            std::process::id(),
            next.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("Cannot create test fixture {}: {error}", path.display()),
        }
    }
}

#[test]
fn fixture_directories_skip_existing_contents() {
    let root = fixture_dir();
    let stale = root.join(format!("mixjam-session-{}-0", std::process::id()));
    std::fs::create_dir(&stale).unwrap();
    let marker = stale.join("sound.wav");
    std::fs::write(&marker, b"existing fixture").unwrap();

    let fresh = fresh_fixture_dir(&root, &AtomicU64::new(0));
    assert_ne!(fresh, stale);
    assert!(std::fs::read_dir(&fresh).unwrap().next().is_none());
    assert_eq!(std::fs::read(&marker).unwrap(), b"existing fixture");
    std::fs::remove_dir_all(root).unwrap();
}
