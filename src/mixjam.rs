//! `.mixjam` songs: the application's native song format (`song`) and a
//! bounded import of the preserved MixJam JSON v7 format.
//!
//! The v7 reader imports an arrangement into the shared renderer; it is not MixJam DSP
//! emulation. The master bus maps onto this application's mastering pedals,
//! which are new designs with the same controls: its order, power switches,
//! trim, and settings carry over. The four return buses and the lane sends
//! carry over too: Echoform Delay and Aetherform Reverb modules play through
//! this application's own effects with their saved settings. Edge fades, and
//! any other module type, remain serialized in source notices and are not
//! rendered; the importer states the sound changes.
//! Lane mute and solo become track Mute and Solo; MixJam plays a soloed lane
//! even when it is muted, so such a lane imports unmuted. Later same-lane triggers truncate
//! earlier clips. Playback tempo comes from the measured WAV duration and saved
//! placement span, as in MixJam's stretchRatioForDuration scheduling. Descriptive
//! nativeBPM is not the arrangement playback-rate authority. UI names, palette,
//! generator metadata and bypassed processor settings are not native controls.
pub(crate) use references::ReferenceResolver;
mod references;
mod song;
#[cfg(test)]
mod tests;
mod v7;

use crate::{engine::Project, library::Library};
use serde::Deserialize;
use std::{fs, io::Read, path::Path};
use v7::{Document, convert};

/// Guards against a file that is not a song. A song holds no audio, and at
/// most `MAX_CLIPS` clips of a few hundred bytes each, so a valid one stays
/// far below it.
const MAX_SONG_BYTES: u64 = 1024 * 1024 * 1024;

/// The fields that all supported format versions share, read before the strict parse.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    format_version: u32,
    created_at: String,
}

fn read(path: &Path) -> Result<(Header, Vec<u8>), String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("Cannot open the MixJam project: {e}"))?
        .take(MAX_SONG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_SONG_BYTES {
        return Err("The MixJam project exceeds the 1 GiB limit.".into());
    }
    let header = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read the MixJam project: {e}"))?;
    Ok((header, bytes))
}

/// Open a song from version 8 through the current native format, or import a
/// version 7 project. The flag is true for a song, which Save can replace;
/// a version 7 project is an import and stays unchanged. The file is read once.
pub fn load_song(path: &Path, library: &Library) -> Result<(Project, bool), String> {
    let (header, bytes) = read(path)?;
    match header.format_version {
        8..=song::FORMAT_VERSION => {
            let mut project = song::parse(&bytes)?;
            // Songs saved before Save named them after their file kept the
            // default title.
            if (project.title.is_empty() || project.title == crate::engine::DEFAULT_TITLE)
                && let Some(stem) = path.file_stem()
            {
                project.title = stem.to_string_lossy().into_owned();
            }
            Ok((project, true))
        }
        7 => {
            let document: Document = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Cannot read the MixJam project: {e}"))?;
            convert(document, path, library).map(|project| (project, false))
        }
        _ => Err(format!(
            "Only MixJam format versions 7 to {} are supported.",
            song::FORMAT_VERSION
        )),
    }
}

/// Write the song in the current native format.
pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    song::save(project, path)
}

fn text(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && value >= min && value <= max
}

fn relative_wave(value: &str) -> bool {
    text(value)
        && !value.contains(['\\', ':'])
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
}
