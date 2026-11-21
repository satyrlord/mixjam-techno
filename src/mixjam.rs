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

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && matches!(bytes[*pos], b' ' | b'\t' | b'\r' | b'\n') {
        *pos += 1;
    }
}

fn scan_string_end(bytes: &[u8], pos: &mut usize) -> Option<usize> {
    if bytes.get(*pos) != Some(&b'"') {
        return None;
    }
    let start = *pos;
    *pos += 1;
    while *pos < bytes.len() {
        match bytes[*pos] {
            b'"' => {
                *pos += 1;
                return Some(start);
            }
            b'\\' => {
                *pos += 1;
                if *pos >= bytes.len() {
                    return None;
                }
                *pos += 1;
            }
            0x00..=0x1F => return None,
            _ => *pos += 1,
        }
    }
    None
}

fn skip_json_value(bytes: &[u8], pos: &mut usize) -> Option<()> {
    skip_ws(bytes, pos);
    let mut depth = 0usize;
    loop {
        skip_ws(bytes, pos);
        match *bytes.get(*pos)? {
            b'"' => {
                scan_string_end(bytes, pos)?;
            }
            b'{' | b'[' => {
                depth += 1;
                *pos += 1;
            }
            b'}' | b']' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                *pos += 1;
                if depth == 0 {
                    return Some(());
                }
            }
            _ => {
                while *pos < bytes.len()
                    && !matches!(
                        bytes[*pos],
                        b',' | b'}' | b']' | b'{' | b'[' | b'"' | b' ' | b'\t' | b'\r' | b'\n'
                    )
                {
                    *pos += 1;
                }
            }
        }
        if depth == 0 {
            return Some(());
        }
        skip_ws(bytes, pos);
        if matches!(bytes.get(*pos), Some(b',' | b':')) {
            *pos += 1;
        }
    }
}

/// Find the version in a song prefix. The full parser checks the whole file
/// before an open succeeds.
fn scan_header(bytes: &[u8]) -> Option<Header> {
    let mut pos = 0;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        pos = 3;
    }
    skip_ws(bytes, &mut pos);
    if bytes.get(pos) != Some(&b'{') {
        return None;
    }
    pos += 1;
    let mut format_version: Option<u32> = None;
    let mut created_at: Option<String> = None;
    loop {
        skip_ws(bytes, &mut pos);
        match *bytes.get(pos)? {
            b'}' => break,
            _ => {
                let key_start = scan_string_end(bytes, &mut pos)?;
                let key_slice = &bytes[key_start..pos];
                skip_ws(bytes, &mut pos);
                if bytes.get(pos) != Some(&b':') {
                    return None;
                }
                pos += 1;
                skip_ws(bytes, &mut pos);
                if key_slice == b"\"formatVersion\"" {
                    if format_version.is_some() {
                        return None;
                    }
                    let num_start = pos;
                    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                        pos += 1;
                    }
                    if pos == num_start || (pos - num_start > 1 && bytes[num_start] == b'0') {
                        return None;
                    }
                    let version: u32 = std::str::from_utf8(&bytes[num_start..pos])
                        .ok()?
                        .parse()
                        .ok()?;
                    format_version = Some(version);
                } else if key_slice == b"\"createdAt\"" {
                    if created_at.is_some() {
                        return None;
                    }
                    let str_start = scan_string_end(bytes, &mut pos)?;
                    let value: String = serde_json::from_slice(&bytes[str_start..pos]).ok()?;
                    created_at = Some(value);
                } else {
                    skip_json_value(bytes, &mut pos)?;
                }
                if let (Some(version), Some(created)) = (&format_version, &created_at)
                    && (7..=song::FORMAT_VERSION).contains(version)
                {
                    return Some(Header {
                        format_version: *version,
                        created_at: created.clone(),
                    });
                }
                skip_ws(bytes, &mut pos);
                match bytes.get(pos) {
                    Some(b',') => pos += 1,
                    Some(b'}') => break,
                    _ => return None,
                }
            }
        }
    }
    None
}

fn parse_header(bytes: &[u8]) -> Result<Header, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("Cannot read the MixJam project: {error}"))
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
    let header = match scan_header(&bytes) {
        Some(header) => header,
        None => parse_header(&bytes)?,
    };
    Ok((header, bytes))
}

/// Open a song from version 8 through the current native format, or import a
/// version 7 project. The flag is true for a song, which Save can replace;
/// a version 7 project is an import and stays unchanged. The file is read once.
pub fn load_song(path: &Path, library: &Library) -> Result<(Project, bool), String> {
    let (header, bytes) = read(path)?;
    let loaded = match header.format_version {
        8..=song::FORMAT_VERSION => song::parse(&bytes).map(|mut project| {
            // Songs saved before Save named them after their file kept the
            // default title.
            if (project.title.is_empty() || project.title == crate::engine::DEFAULT_TITLE)
                && let Some(stem) = path.file_stem()
            {
                project.title = stem.to_string_lossy().into_owned();
            }
            (project, true)
        }),
        7 => serde_json::from_slice::<Document>(&bytes)
            .map_err(|e| format!("Cannot read the MixJam project: {e}"))
            .and_then(|document| convert(document, path, library))
            .map(|project| (project, false)),
        _ => {
            return Err(format!(
                "Only MixJam format versions 7 to {} are supported.",
                song::FORMAT_VERSION
            ));
        }
    };
    loaded.or_else(|error| {
        // Report header and syntax errors the same way for either reader.
        parse_header(&bytes)?;
        Err(error)
    })
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
