//! D1's observed 0x0a06 grids and its two shipped footer encodings.
//! The common project uses ticks directly; T1's 180 BPM frame clock is not used.
use crate::{
    engine::{
        Clip, ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR,
        normalize_import,
    },
    library::{Library, decode_1252},
    products::Product,
};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use legacy_format::mix::{CELLS, TRACKS, TRAILER_OFFSET};
const EDITABLE_ROWS: usize = 350;
const FACTORY_FIRST: u16 = 731;
const IMPORT_FIRST: u16 = 2083;
const KIT_FIRST: u16 = 3400;
const KIT_REGIONS: [(u16, u16); 3] = [(3400, 3899), (3900, 4499), (4500, 5199)];

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    let bytes = super::song_file::read_song_file(path)?;
    let wav_product = library.is_wav_product(Product::D1);
    convert(&bytes, path, library, |native_id| {
        let id = Product::D1.global_sample_id(u32::from(native_id))?;
        let sample = library
            .sample_by_id(id)
            .ok_or_else(|| library.describe_missing(Product::D1, u32::from(native_id)))?;
        if sample.source_bpm != 140.0
            || (!wav_product && sample.sample_rate != legacy_format::NATIVE_SAMPLE_RATE)
        {
            return Err("The D1 sample library has incompatible source timing.".into());
        }
        Ok((id, sample.bars, sample.code.clone()))
    })
}

#[derive(Clone, Copy)]
struct Event {
    row: usize,
    track: usize,
    sample: u16,
    offset: u16,
}
struct Footer {
    title: String,
    routes: [u16; TRACKS],
    regions: Vec<(u16, u16, String)>,
    imports: Vec<(u16, [String; 4])>,
}

fn convert(
    bytes: &[u8],
    path: &Path,
    library: &Library,
    resolve: impl FnMut(u16) -> Result<(u32, u32, String), String>,
) -> Result<Project, String> {
    convert_with_catalog(bytes, path, library, resolve, |native_id| {
        crate::catalog::identity(Product::D1, native_id)
    })
}

fn convert_with_catalog<'a>(
    bytes: &[u8],
    path: &Path,
    library: &Library,
    mut resolve: impl FnMut(u16) -> Result<(u32, u32, String), String>,
    identity: impl Fn(u32) -> Option<&'a crate::catalog::WaveMetadata>,
) -> Result<Project, String> {
    if bytes.len() < TRAILER_OFFSET || bytes.get(..2) != Some(&[6, 10]) {
        return Err("This file does not contain the supported D1 MIX profile.".into());
    }
    let mut events = Vec::new();
    for cell in 0..CELLS {
        let sample = u16::from_le_bytes(bytes[2 + cell * 2..4 + cell * 2].try_into().unwrap());
        if sample == 0 {
            continue;
        }
        let offset = u16::from_le_bytes(
            bytes[2 + CELLS * 2 + cell * 2..4 + CELLS * 2 + cell * 2]
                .try_into()
                .unwrap(),
        );
        let row = cell / TRACKS;
        if row >= EDITABLE_ROWS
            || !(FACTORY_FIRST..=i16::MAX as u16).contains(&sample)
            || offset >= 16
        {
            return Err(
                "The D1 MIX has an invalid sample ID, guard-row placement, or sixteenth offset."
                    .into(),
            );
        }
        events.push(Event {
            row,
            track: cell % TRACKS,
            sample,
            offset,
        });
    }
    let referenced_ids: BTreeSet<_> = events.iter().map(|event| event.sample).collect();
    let footer = parse_footer(&bytes[TRAILER_OFFSET..], &referenced_ids)?;
    let fallback_title = path.file_stem().map_or_else(
        || "Untitled".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let title = if footer.title.is_empty()
        || footer.title.contains("\x44ance\x4dachine")
        || footer.title == "\x44ance \x65Jay 1.01"
    {
        fallback_title
    } else {
        footer.title
    };
    let mut project = Project {
        title,
        bpm: 140.0,
        master_gain: 1.0,
        ..Project::default()
    };
    for (track, route) in project.tracks.iter_mut().zip(footer.routes) {
        track.left_enabled = matches!(route, 0 | 1);
        track.right_enabled = matches!(route, 0 | 2);
    }
    let mut sources = HashMap::new();
    for (source_id, fields) in footer.imports {
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let saved = reference_path(&fields[2], base);
        let rejected = saved.is_err();
        let (saved_path, resolved) = match saved {
            Ok(saved) => {
                let resolved =
                    resolve_path(&saved, &fields[2], library).and_then(|(path, candidates)| {
                        // The frame count and rate come from a streamed read that
                        // holds no samples.
                        let (spec, frames) = crate::audio::visit_wav_samples(&path, |_, _| {})?;
                        if frames == 0 {
                            return Err("The D1 imported sample contains no audio.".into());
                        }
                        let bars = (f64::from(frames) / f64::from(spec.sample_rate) * 140.0 / 240.0)
                            .ceil() as u32;
                        Ok((path, bars, spec.sample_rate, candidates))
                    });
                (saved, resolved)
            }
            // A bad saved path makes one placeholder, not a failed import.
            Err(reason) => (PathBuf::from(&fields[2]), Err(reason)),
        };
        let id = project.next_import_id()?;
        let name = format!("{} {}", fields[0], fields[1]).trim().to_owned();
        let name = if name.is_empty() {
            format!("Imported sample {source_id}")
        } else {
            name
        };
        let imported = ImportedSample {
            id,
            name,
            group_name: if fields[3].is_empty() {
                "Wave".into()
            } else {
                fields[3].clone()
            },
            path: saved_path,
            source_bpm: 140.0,
            source_sample_rate: None,
            origin: Some(Product::D1),
        };
        let bars = match resolved {
            Ok((path, bars, sample_rate, candidates)) => {
                crate::library::record_wav_conflict(&mut project, &fields[2], id, candidates);
                project.imported_samples.push(ImportedSample {
                    path,
                    source_sample_rate: Some(sample_rate),
                    ..imported
                });
                bars
            }
            Err(reason) => {
                let bars = inferred_bars(&events, source_id);
                let mut missing = MissingSample::external(&imported, bars * TICKS_PER_BAR, reason);
                if rejected {
                    missing.reference = super::import_support::rejected_reference(
                        Product::D1,
                        &fields[2],
                        source_id,
                    );
                }
                for (key, value) in [
                    ("source_id", source_id.to_string()),
                    ("label_1", fields[0].clone()),
                    ("label_2", fields[1].clone()),
                    ("saved_path", fields[2].clone()),
                    ("category", fields[3].clone()),
                ] {
                    missing.metadata.insert(key.into(), value);
                }
                project.missing_samples.push(missing);
                bars
            }
        };
        sources.insert(source_id, (id, bars, String::new()));
    }
    for event in &events {
        if sources.contains_key(&event.sample) {
            continue;
        }
        let saved_kit = footer
            .regions
            .iter()
            .enumerate()
            .find(|(_, (first, last, _))| (*first..=*last).contains(&event.sample));
        let unresolved_kit = saved_kit
            .filter(|(index, (first, last, name))| {
                let (known_first, known_last) = KIT_REGIONS[*index];
                let known_old_last = *index == 1 && footer.regions.len() == 2 && *last == 4500;
                *first != known_first
                    || (*last != known_last && !known_old_last)
                    || !known_saved_kit_name(*index, name)
            })
            .map(|(_, region)| region);
        let resolved = if unresolved_kit.is_some() {
            Err(
                "The saved D1 expansion kit name or ID range cannot be matched to the active \
                 catalog."
                    .into(),
            )
        } else {
            resolve(event.sample)
        };
        let source = match resolved {
            Ok(source) => source,
            Err(reason) => {
                let (mut missing, bars, code) = native_placeholder(
                    Product::D1,
                    event.sample,
                    inferred_bars(&events, event.sample),
                    reason,
                    if unresolved_kit.is_some() {
                        None
                    } else {
                        identity(u32::from(event.sample))
                    },
                )?;
                if let Some((_, (first, last, name))) = saved_kit {
                    missing.metadata.insert("kit_name".into(), name.clone());
                    missing
                        .metadata
                        .insert("source_id".into(), event.sample.to_string());
                    missing
                        .metadata
                        .insert("kit_first_id".into(), first.to_string());
                    missing
                        .metadata
                        .insert("kit_last_id".into(), last.to_string());
                }
                if let Some((first, last, name)) = unresolved_kit {
                    missing.id = project.next_import_id()?;
                    missing.reference = MissingReference::Generated {
                        product: Product::D1,
                        kind: "unresolved-kit".into(),
                        data: serde_json::json!({"native_id": event.sample, "first_id": first, "last_id": last, "name": name}),
                    };
                }
                let id = missing.id;
                project.missing_samples.push(missing);
                (id, bars, code)
            }
        };
        if source.1 == 0 || source.1 > 32766 {
            return Err("A D1 sample has an invalid native bar length.".into());
        }
        if let Some((kit, _)) = saved_kit {
            let prefix = format!("dmkit{}/", kit + 1);
            if !source.2.is_empty()
                && !source
                    .2
                    .replace('\\', "/")
                    .to_ascii_lowercase()
                    .starts_with(&prefix)
            {
                return Err(
                    "The saved D1 kit region does not match the active sample catalog.".into(),
                );
            }
        }
        sources.insert(event.sample, source);
    }
    for track in 0..TRACKS {
        let track_events: Vec<_> = events.iter().filter(|e| e.track == track).collect();
        for (index, event) in track_events.iter().enumerate() {
            let (sample_id, bars, _) = &sources[&event.sample];
            let extent =
                (*bars as usize + usize::from(event.offset > 0)).min(EDITABLE_ROWS - event.row);
            let next = track_events.get(index + 1);
            let gap = next.map_or(extent, |next| next.row - event.row).min(extent);
            let end_offset = next
                .filter(|next| next.row - event.row == gap)
                .map_or(0, |next| u32::from(next.offset) * 16);
            let length = (gap as u32 * TICKS_PER_BAR + end_offset)
                .checked_sub(u32::from(event.offset) * 16)
                .filter(|length| *length > 0)
                .ok_or("The D1 sample has no positive scheduled duration.")?;
            project.clips.push(Clip {
                id: project.clips.len() as u64 + 1,
                sample_id: *sample_id,
                track,
                start_tick: event.row as u32 * TICKS_PER_BAR + u32::from(event.offset) * 16,
                length_ticks: length,
                offset_frames: 0,
                gain: 1.0,
                fades: Default::default(),
                pitch_cents: 0,
                reverse: false,
            });
        }
    }
    project
        .clips
        .sort_by_key(|clip| (clip.start_tick, clip.track));
    normalize_import(project)
}

fn known_saved_kit_name(index: usize, name: &str) -> bool {
    // These aliases occur in the shipped D1 demos. Their ordinal regions use
    // the same dmkit1/..dmkit3/ catalogs checked below before source adoption.
    // The first two aliases come from the older plain-footer demos; the three
    // descriptive aliases come from dream.mix and space.mix.
    const NAMES: [&[&str]; 3] = [
        &[
            "DanceMachine Sample-Kit Vol. 1",
            "gung -SAMPLE BOX- [Raps && Vocals]",
        ],
        &[
            "DanceMachine Sample-Kit Vol. 2",
            "gung -SAMPLE BOX- [Drams && Synthies]",
        ],
        &["gung -SAMPLE BOX- [Space Sounds]"],
    ];
    name.is_empty() || NAMES.get(index).is_some_and(|names| names.contains(&name))
}

fn inferred_bars(events: &[Event], source_id: u16) -> u32 {
    let events: Vec<_> = events
        .iter()
        .map(|event| (event.row, event.track, event.sample))
        .collect();
    super::import_support::inferred_bars(&events, source_id)
}

pub(super) fn native_placeholder(
    product: Product,
    native_id: u16,
    inferred_bars: u32,
    reason: String,
    metadata: Option<&crate::catalog::WaveMetadata>,
) -> Result<(MissingSample, u32, String), String> {
    let bars = metadata.map_or(inferred_bars, |sample| sample.bars);
    let name = metadata.map_or_else(
        || format!("{} sample {native_id}", product.label()),
        |sample| sample.display_name(product),
    );
    let mut missing = MissingSample::native(
        product,
        u32::from(native_id),
        name,
        if product == Product::T1 { 180.0 } else { 140.0 },
        bars * TICKS_PER_BAR,
        metadata.is_none(),
        reason,
    )?;
    let code = metadata.map_or_else(String::new, |sample| sample.code.clone());
    if let Some(sample) = metadata {
        for (key, value) in [
            ("code", sample.code.clone()),
            ("category", sample.group_name.clone()),
            ("catalog_bars", sample.bars.to_string()),
            ("catalog_frame_count", sample.frame_count.to_string()),
        ] {
            missing.metadata.insert(key.into(), value);
        }
        if let Some(digest) = &sample.pcm_digest {
            missing.metadata.insert("pcm_digest".into(), digest.clone());
        }
    }
    Ok((missing, bars, code))
}

fn parse_footer(bytes: &[u8], referenced_ids: &BTreeSet<u16>) -> Result<Footer, String> {
    let mut footer = Footer {
        title: String::new(),
        routes: [0; TRACKS],
        regions: Vec::new(),
        imports: Vec::new(),
    };
    if bytes.is_empty() {
        if referenced_ids
            .iter()
            .any(|id| (IMPORT_FIRST..KIT_FIRST).contains(id))
        {
            return Err("The D1 MIX has imported sample IDs without saved file references.".into());
        }
        return Ok(footer);
    }
    let mut reader = Reader { bytes, offset: 0 };
    let marker = reader.word()?;
    let wrapped = match marker {
        0x0a06 => false,
        0x0a08 => true,
        _ => return Err("The D1 MIX footer encoding is not supported.".into()),
    };
    let sections = reader.word()?;
    if !matches!(sections, 1 | 2) {
        return Err("The D1 MIX footer section count is invalid.".into());
    }
    footer.title = reader.text(wrapped)?;
    if sections == 2 {
        let count = reader.word()? as usize;
        if count != if wrapped { 3 } else { 2 } {
            return Err("The D1 MIX has an unsupported kit table.".into());
        }
        let mut previous_last = IMPORT_FIRST - 1;
        for _ in 0..count {
            let first = reader.word()?;
            let last = reader.word()?;
            let name = reader.text(wrapped)?;
            if first <= previous_last || last < first || last > i16::MAX as u16 {
                return Err("The D1 expansion-kit ID ranges are invalid or overlap.".into());
            }
            previous_last = last;
            footer.regions.push((first, last, name));
        }
    }
    let first_kit = footer
        .regions
        .first()
        .map_or(KIT_FIRST, |(first, _, _)| *first);
    let expected_imports: BTreeSet<_> = referenced_ids
        .iter()
        .copied()
        .filter(|id| (IMPORT_FIRST..first_kit).contains(id))
        .collect();
    let count = reader.word()? as usize;
    if count != expected_imports.len() {
        return Err("The D1 imported sample count does not match the referenced IDs.".into());
    }
    if !wrapped && count > 0 {
        return Err("Imported samples in the old plain footer are not supported.".into());
    }
    for source_id in expected_imports {
        let payload = reader.word()? as usize;
        let field_start = reader.offset;
        let fields = [
            reader.text(true)?,
            reader.text(true)?,
            reader.text(true)?,
            reader.text(true)?,
        ];
        // Each wrapped field contains a length WORD and two delimiters. The
        // payload excludes the four length WORDs; measure original ANSI bytes.
        if payload != reader.offset - field_start - 8 {
            return Err("The D1 imported record length does not match its text fields.".into());
        }
        footer.imports.push((source_id, fields));
    }
    if wrapped {
        if reader.word()? != TRACKS as u16 {
            return Err("The D1 saved speaker count must be eight.".into());
        }
        for route in &mut footer.routes {
            *route = reader.word()?;
            if *route > 3 {
                return Err("The D1 MIX contains an invalid speaker route.".into());
            }
        }
    }
    if reader.offset != bytes.len() {
        return Err("The D1 MIX footer has unsupported trailing data.".into());
    }
    Ok(footer)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("The D1 footer offset overflows.")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("The D1 MIX footer is truncated.")?;
        self.offset = end;
        Ok(bytes)
    }
    fn word(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn text(&mut self, wrapped: bool) -> Result<String, String> {
        let length = self.word()? as usize;
        if length > 16539 || (wrapped && length < 2) {
            return Err("The D1 text field exceeds its supported length.".into());
        }
        let bytes = self.take(length)?;
        let bytes = if wrapped {
            bytes
                .strip_suffix(&[0, 1])
                .ok_or("The D1 text delimiters are invalid.")?
        } else {
            bytes
        };
        if bytes.iter().any(|byte| *byte <= 1) {
            return Err("The D1 text field contains an invalid delimiter.".into());
        }
        Ok(decode_1252(bytes))
    }
}

fn reference_path(text: &str, base: &Path) -> Result<PathBuf, String> {
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err("The D1 imported sample path is invalid.".into());
    }
    super::import_support::saved_path(
        text,
        || {
            if base.is_absolute() {
                Ok(base.to_path_buf())
            } else {
                std::env::current_dir()
                    .map(|dir| dir.join(base))
                    .map_err(|error| format!("Cannot resolve the D1 project folder: {error}"))
            }
        },
        "The D1 imported sample needs a full drive path or a relative project path.",
    )
}

fn resolve_path(
    path: &Path,
    text: &str,
    library: &Library,
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let (path, candidates) = super::import_support::find_file(path, text, library)?;
    fs::canonicalize(path)
        .map(|path| (path, candidates))
        .map_err(|e| format!("Cannot open the D1 imported sample: {e}"))
}

#[cfg(test)]
mod tests;
