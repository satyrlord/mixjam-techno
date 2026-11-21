//! Shared helpers of the T1 and D1 imports: the saved import paths and the
//! inferred bar length of an unresolved source.
//!
//! A song file is not trusted. Both importers refuse network and device path
//! forms before looking up the saved sample.
use crate::{engine::MissingReference, library::Library, products::Product};
use std::path::{Component, Path, PathBuf, Prefix};

/// A native import whose saved path requires an explicit WAV choice.
pub const UNRESOLVED_IMPORT_PATH: &str = "unresolved-import-path";

/// Keep the rejected path as data. An external reference would be tried
/// again by the common song opener, and an empty path identifies no file.
pub(super) fn rejected_reference(product: Product, path: &str, source_id: u16) -> MissingReference {
    let data = if path.is_empty() {
        serde_json::json!({"saved_path": path, "source_id": source_id})
    } else {
        serde_json::json!({"saved_path": path})
    };
    MissingReference::Generated {
        product,
        kind: UNRESOLVED_IMPORT_PATH.into(),
        data,
    }
}

/// Return the saved path text and the ID of an unnamed source, for manual resolution.
pub fn rejected_import_path(reference: &MissingReference) -> Option<(&str, Option<u16>)> {
    let MissingReference::Generated { kind, data, .. } = reference else {
        return None;
    };
    if kind != UNRESOLVED_IMPORT_PATH {
        return None;
    }
    let path = data.get("saved_path")?.as_str()?;
    let source_id = data
        .get("source_id")
        .and_then(serde_json::Value::as_u64)
        .and_then(|id| u16::try_from(id).ok());
    Some((path, source_id))
}

/// True when the path starts with a network or device prefix: `\\host\share`,
/// `\\.\device`, or a verbatim path that is not a plain drive.
fn has_unsafe_prefix(path: &Path) -> bool {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => {
            !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        }
        _ => false,
    }
}

/// True when `name` is a reserved Windows device name, such as `NUL`, `CON`,
/// or `COM1`. An extension, a stream name, or trailing spaces do not change
/// that. Some Windows versions open the device for such a name in any folder.
fn is_device_name(name: &str) -> bool {
    let stem = name.split(['.', ':']).next().unwrap_or(name);
    let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
    let port = |prefix: &str| {
        let mut rest = stem.strip_prefix(prefix).unwrap_or_default().chars();
        matches!(
            (rest.next(), rest.next()),
            (Some(digit), None) if digit.is_ascii_digit() || "\u{b9}\u{b2}\u{b3}".contains(digit)
        )
    };
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || port("COM")
        || port("LPT")
}

/// True when any component of the path is a reserved device name. Trailing
/// `..` segments or subfolder names must not hide a device component before
/// Windows normalizes the path.
fn names_device(path: &Path) -> bool {
    path.components().any(|part| match part {
        Component::Normal(name) => name.to_str().is_some_and(is_device_name),
        _ => false,
    })
}

/// Turn the saved text of an import path into a path. An absolute path must
/// be a plain drive path. A relative path is joined to the project folder
/// that `base` returns. The joined path must also stay on a drive. Other
/// forms and device names fail with `drive_error` before any file is opened.
pub(super) fn saved_path(
    text: &str,
    base: impl FnOnce() -> Result<PathBuf, String>,
    drive_error: &str,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(text.replace('\\', "/"));
    if names_device(&path) {
        return Err(drive_error.into());
    }
    if path.is_absolute() {
        if has_unsafe_prefix(&path) {
            return Err(drive_error.into());
        }
        return Ok(path);
    }
    if path.has_root()
        || path
            .components()
            .any(|part| matches!(part, Component::Prefix(_)))
    {
        return Err(drive_error.into());
    }
    let path = base()?.join(path);
    if has_unsafe_prefix(&path) {
        return Err(drive_error.into());
    }
    Ok(path)
}

/// Find the file of a saved import path. It is the saved path itself when
/// that file exists, else the best match among the library's WAV files.
pub(super) fn find_file(
    path: &Path,
    reference: &str,
    library: &Library,
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    if path.is_file() {
        return Ok((path.to_path_buf(), Vec::new()));
    }
    let candidates = library.find_wav_candidates(reference)?;
    let path = candidates
        .first()
        .cloned()
        .ok_or_else(|| format!("WAV sample {reference} is missing from the selected folder."))?;
    Ok((path, candidates))
}

/// The longest gap, in rows, between a placement of the source and the next
/// placement on the same track. The events are `(row, track, source ID)`.
pub(super) fn inferred_bars(events: &[(usize, usize, u16)], source_id: u16) -> u32 {
    events
        .iter()
        .filter(|&&(_, _, sample)| sample == source_id)
        .map(|&(row, track, _)| {
            events
                .iter()
                .find(|&&(next_row, next_track, _)| next_track == track && next_row > row)
                .map_or(1, |(next_row, _, _)| (next_row - row) as u32)
        })
        .max()
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRIVE_ERROR: &str = "drive path needed";

    fn base() -> Result<PathBuf, String> {
        Ok(PathBuf::from("C:/songs"))
    }

    fn unreachable_base() -> Result<PathBuf, String> {
        panic!("An absolute path must not read the project folder.");
    }

    #[test]
    fn accepts_plain_drive_and_relative_paths() {
        for text in [r"C:\music\a.wav", "D:/music/a.wav"] {
            assert_eq!(
                saved_path(text, unreachable_base, DRIVE_ERROR).unwrap(),
                PathBuf::from(text.replace('\\', "/"))
            );
        }
        assert_eq!(
            saved_path(r"drums\a.wav", base, DRIVE_ERROR).unwrap(),
            PathBuf::from("C:/songs/drums/a.wav")
        );
    }

    #[cfg(windows)]
    #[test]
    fn rejects_network_device_and_verbatim_prefixes() {
        for text in [
            r"\\attacker\share\a.wav",
            "//attacker/share/a.wav",
            r"\\.\pipe\a.wav",
            r"\\.\C:\a.wav",
            r"\\?\UNC\attacker\share\a.wav",
            r"\\?\GLOBALROOT\Device\a.wav",
            r"\\?\C:\a.wav",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\a.wav",
        ] {
            assert_eq!(
                saved_path(text, base, DRIVE_ERROR),
                Err(DRIVE_ERROR.to_owned()),
                "{text}"
            );
        }
    }

    #[test]
    fn rejects_a_device_name_in_any_folder() {
        for text in [
            r"C:\music\COM1",
            "C:/music/con.wav",
            "nul",
            r"drums\Lpt9.wav",
            "aux .wav",
            "prn:stream",
            "CONOUT$",
            "com\u{b2}.wav",
            r"C:\music\CON\x\..",
            r"C:\music\COM1\a\..",
            r"nul\x\..",
            r"C:\music\CON\a.wav",
        ] {
            assert_eq!(
                saved_path(text, base, DRIVE_ERROR),
                Err(DRIVE_ERROR.to_owned()),
                "{text}"
            );
        }
        // A name that only starts like a device name is a file.
        for text in ["console.wav", "com10.wav", "nulls/a.wav", "com.wav"] {
            assert!(saved_path(text, base, DRIVE_ERROR).is_ok(), "{text}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn rejects_rooted_and_drive_relative_paths() {
        for text in [r"\a.wav", "/a.wav", r"C:a.wav"] {
            assert_eq!(
                saved_path(text, base, DRIVE_ERROR),
                Err(DRIVE_ERROR.to_owned()),
                "{text}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn relative_paths_cannot_inherit_a_network_or_device_prefix() {
        for base in [r"\\server\share\songs", r"\\.\C:\songs"] {
            assert_eq!(
                saved_path("drums/a.wav", || Ok(PathBuf::from(base)), DRIVE_ERROR),
                Err(DRIVE_ERROR.to_owned()),
                "{base}"
            );
        }
        assert_eq!(
            saved_path(
                "D:/music/a.wav",
                || Ok(PathBuf::from(r"\\server\share\songs")),
                DRIVE_ERROR,
            )
            .unwrap(),
            PathBuf::from("D:/music/a.wav")
        );
    }

    #[cfg(windows)]
    #[test]
    fn prefix_rule_accepts_only_disk_and_verbatim_disk() {
        for (text, accepted) in [
            (r"C:\a.wav", true),
            (r"\\?\C:\a.wav", true),
            (r"\\host\share\a.wav", false),
            (r"\\.\C:\a.wav", false),
            (r"\\?\UNC\host\share\a.wav", false),
            (r"\\?\pipe\a.wav", false),
            (r"relative\a.wav", true),
        ] {
            assert_eq!(!has_unsafe_prefix(Path::new(text)), accepted, "{text}");
        }
    }

    #[test]
    fn inferred_bars_uses_the_longest_gap_to_the_next_row_of_the_track() {
        let events = [(0, 0, 5), (3, 0, 6), (1, 1, 5), (10, 0, 5)];
        assert_eq!(inferred_bars(&events, 5), 3);
        assert_eq!(inferred_bars(&events, 6), 7);
        assert_eq!(inferred_bars(&events, 9), 1);
    }
}
