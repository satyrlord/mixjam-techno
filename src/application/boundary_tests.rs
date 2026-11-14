mod commands;
mod frontend;
mod history;
mod library_scan;
mod saving;

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
fn fixture_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "mixjam-session-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
