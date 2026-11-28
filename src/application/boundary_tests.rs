mod commands;
mod frontend;
mod history;
mod library_scan;
mod saving;

use super::*;
use crate::test_folder::TestFolder;

/// A new, empty folder for one test; it removes itself at the end.
fn fixture_dir() -> TestFolder {
    TestFolder::new("mixjam-session")
}
