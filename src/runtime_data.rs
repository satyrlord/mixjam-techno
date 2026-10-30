//! Runtime data folders that ship beside the executable and are read at run
//! time, not compiled in. A folder lives under `assets/` at the application
//! root: the repository root for a build, or the installation folder.
use std::path::{Path, PathBuf};

/// How many folders above the executable's own folder are searched. A build
/// runs from `target/release` or `target/debug/deps`, three folders below the
/// repository root; an installation keeps the executable in its root.
const SEARCH_DEPTH: usize = 4;

/// The runtime data folder `assets/<name>` of this application, if one exists.
pub fn find(name: &str) -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    find_from(executable.parent()?, name)
}

/// The nearest `assets/<name>` folder in `start` or the folders above it.
pub fn find_from(start: &Path, name: &str) -> Option<PathBuf> {
    start
        .ancestors()
        .take(SEARCH_DEPTH + 1)
        .map(|folder| folder.join("assets").join(name))
        .find(|folder| folder.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("mixjam-runtime-data-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_build_finds_the_repository_folder_and_an_installation_its_own() {
        let root = fixture("layouts");
        let deps = root.join("target").join("debug").join("deps");
        std::fs::create_dir_all(&deps).unwrap();
        assert_eq!(find_from(&deps, "album-review"), None);
        std::fs::create_dir_all(root.join("assets").join("album-review")).unwrap();
        assert_eq!(
            find_from(&deps, "album-review"),
            Some(root.join("assets").join("album-review"))
        );
        assert_eq!(
            find_from(&root, "album-review"),
            Some(root.join("assets").join("album-review"))
        );
        // A file of that name is not a folder.
        std::fs::write(root.join("assets").join("catalogs"), b"").unwrap();
        assert_eq!(find_from(&deps, "catalogs"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_search_stops_a_few_folders_above_the_executable() {
        let root = fixture("depth");
        std::fs::create_dir_all(root.join("assets").join("album-review")).unwrap();
        let deep = root.join("a").join("b").join("c").join("d").join("e");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_from(&deep, "album-review"), None);
        assert!(find_from(deep.parent().unwrap(), "album-review").is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn this_build_finds_the_album_reviews_of_its_repository() {
        let found = find("album-review").expect("The repository ships assets/album-review.");
        assert!(found.join("vangelis-voices.json").is_file());
    }
}
