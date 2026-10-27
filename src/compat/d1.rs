//! D1's observed 0x0a06 grids and its two shipped footer encodings.
//! The common project uses ticks directly; T1's 180 BPM frame clock is not used.
use crate::{
    engine::{
        Clip, ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR,
        normalize_import,
    },
    library::{Library, decode_1252},
    products::{IMPORTED_ID_START, Product},
};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Component, Path, PathBuf},
};

const TRACKS: usize = 8;
const ROWS: usize = 351;
const EDITABLE_ROWS: usize = 350;
const CELLS: usize = TRACKS * ROWS;
const TRAILER_OFFSET: usize = 2 + CELLS * 4;
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
        if sample.source_bpm != 140.0 || (!wav_product && sample.sample_rate != 44_100) {
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
    mut resolve: impl FnMut(u16) -> Result<(u32, u32, String), String>,
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
    let catalog = crate::catalog::wave_metadata(Product::D1)?;
    let mut sources = HashMap::new();
    for (source_id, fields) in footer.imports {
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let saved_path = reference_path(&fields[2], base)?;
        let resolved = resolve_path(&fields[2], base, library).and_then(|(path, candidates)| {
            let pcm = crate::audio::read_wav(&path)?;
            if pcm.frames() == 0 {
                return Err("The D1 imported sample contains no audio.".into());
            }
            let bars = (pcm.frames() as f64 / pcm.sample_rate as f64 * 140.0 / 240.0).ceil() as u32;
            Ok((path, bars, pcm.sample_rate, candidates))
        });
        let id = IMPORTED_ID_START
            .checked_add((project.imported_samples.len() + project.missing_samples.len()) as u32)
            .ok_or("Imported sample ID limit reached.")?;
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
            Err("The saved D1 expansion kit name or ID range cannot be matched to the active catalog.".into())
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
                        catalog
                            .iter()
                            .find(|sample| sample.native_id == u32::from(event.sample))
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
                    missing.id = IMPORTED_ID_START
                        .checked_add(
                            (project.imported_samples.len() + project.missing_samples.len()) as u32,
                        )
                        .ok_or("Imported sample ID limit reached.")?;
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
    events
        .iter()
        .filter(|event| event.sample == source_id)
        .map(|event| {
            events
                .iter()
                .find(|next| next.track == event.track && next.row > event.row)
                .map_or(1, |next| (next.row - event.row) as u32)
        })
        .max()
        .unwrap_or(1)
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
        |sample| sample.name.clone(),
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
    let path = PathBuf::from(text.replace('\\', "/"));
    let path = if path.is_absolute() {
        path
    } else {
        if path.has_root()
            || path
                .components()
                .any(|part| matches!(part, Component::Prefix(_)))
        {
            return Err(
                "The D1 imported sample needs a full drive path or a relative project path.".into(),
            );
        }
        base.join(path)
    };
    Ok(path)
}

fn resolve_path(
    text: &str,
    base: &Path,
    library: &Library,
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let path = reference_path(text, base)?;
    let (path, candidates) = if path.is_file() {
        (path, Vec::new())
    } else {
        let candidates = library.find_wav_candidates(text)?;
        let path = candidates
            .first()
            .cloned()
            .ok_or_else(|| format!("WAV sample {text} is missing from the selected folder."))?;
        (path, candidates)
    };
    fs::canonicalize(path)
        .map(|path| (path, candidates))
        .map_err(|e| format!("Cannot open the D1 imported sample: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mix() -> Vec<u8> {
        let mut b = vec![0; TRAILER_OFFSET];
        b[..2].copy_from_slice(&[6, 10]);
        b
    }
    fn place(b: &mut [u8], row: usize, track: usize, sample: u16, offset: u16) {
        let cell = row * TRACKS + track;
        b[2 + cell * 2..4 + cell * 2].copy_from_slice(&sample.to_le_bytes());
        b[2 + CELLS * 2 + cell * 2..4 + CELLS * 2 + cell * 2]
            .copy_from_slice(&offset.to_le_bytes());
    }
    fn resolve(id: u16) -> Result<(u32, u32, String), String> {
        if (731..2083).contains(&id) {
            Ok((
                Product::D1.global_sample_id(u32::from(id))?,
                2,
                "base.\x70xd".into(),
            ))
        } else {
            Err("Unknown fixture sample.".into())
        }
    }
    fn wrapped(b: &mut Vec<u8>, text: &[u8]) {
        b.extend(((text.len() + 2) as u16).to_le_bytes());
        b.extend(text);
        b.extend([0, 1]);
    }
    #[test]
    fn wav_only_provider_keeps_d1_native_ids_and_nominal_bars() {
        let root = std::env::temp_dir().join(format!(
            "d1-wav-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let wave_path = root.join("base.wav");
        fs::write(
            &wave_path,
            legacy_format::wav::PcmWave {
                sample_rate: 22_050,
                channels: 1,
                samples: vec![1000; 37_800],
            }
            .encode()
            .unwrap(),
        )
        .unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(&root, 140.0).unwrap();
        library
            .register_wav_product(
                Product::D1,
                &root,
                &[crate::catalog::WaveMetadata {
                    native_id: 731,
                    code: "base.\x70xd".into(),
                    name: "Sample".into(),
                    group_name: "Test".into(),
                    bars: 2,
                    source_bpm: 140.0,
                    frame_count: 75_600,
                    pcm_digest: None,
                }],
            )
            .unwrap();
        let mut bytes = mix();
        place(&mut bytes, 0, 0, 731, 0);
        let path = root.join("song.mix");
        fs::write(&path, bytes).unwrap();
        let project = load(&path, &library).unwrap();
        assert_eq!(
            project.clips[0].sample_id,
            Product::D1.global_sample_id(731).unwrap()
        );
        assert_eq!(project.clips[0].start_tick, 0);
        assert_eq!(project.clips[0].length_ticks, 512);
        fs::remove_file(path).unwrap();
        fs::remove_file(wave_path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn matrix_positions_use_sixteenth_ticks_and_native_duration_caps_at_140_bpm() {
        let mut b = mix();
        place(&mut b, 0, 0, 731, 4);
        place(&mut b, 1, 0, 732, 12);
        let p = convert(&b, Path::new("fixture.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(p.bpm, 140.0);
        assert_eq!(p.tracks.len(), 64);
        assert_eq!((p.clips[0].start_tick, p.clips[0].length_ticks), (64, 384));
        assert_eq!((p.clips[1].start_tick, p.clips[1].length_ticks), (448, 576));
        assert!(p.import_notices.is_empty());
    }
    #[test]
    fn factory_ids_above_t1_import_boundary_remain_native_and_routes_survive() {
        let mut b = mix();
        place(&mut b, 0, 0, 1989, 0);
        b.extend([8, 10, 1, 0]);
        wrapped(&mut b, b"\x44ance \x65Jay 1.01");
        b.extend([0, 0, 8, 0]);
        for route in [1u16, 2, 3, 0, 0, 0, 0, 0] {
            b.extend(route.to_le_bytes());
        }
        let p = convert(&b, Path::new("fixture.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(
            p.clips[0].sample_id,
            Product::D1.global_sample_id(1989).unwrap()
        );
        assert!(p.imported_samples.is_empty());
        assert!(!p.tracks[0].right_enabled);
        assert!(!p.tracks[1].left_enabled);
        assert!(!p.tracks[2].left_enabled && !p.tracks[2].right_enabled);
    }
    #[test]
    fn invalid_guards_offsets_footer_counts_and_truncation_fail_without_partial_project() {
        let mut b = mix();
        place(&mut b, 350, 0, 731, 0);
        assert!(convert(&b, Path::new("bad.mix"), &Library::empty(), resolve).is_err());
        let mut b = mix();
        place(&mut b, 0, 0, 731, 16);
        assert!(convert(&b, Path::new("bad.mix"), &Library::empty(), resolve).is_err());
        assert!(convert(&b[..100], Path::new("bad.mix"), &Library::empty(), resolve).is_err());
        let mut b = mix();
        place(&mut b, 0, 0, 2083, 0);
        assert!(convert(&b, Path::new("bad.mix"), &Library::empty(), resolve).is_err());
        assert!(parse_footer(&[8, 10, 3, 0], &BTreeSet::new()).is_err());
    }
    #[test]
    fn imported_footer_lengths_use_original_ansi_bytes_and_d1_import_ids() {
        let mut footer = vec![8, 10, 1, 0];
        wrapped(&mut footer, b"\x44ance \x65Jay 1.01");
        footer.extend(1u16.to_le_bytes());
        let fields: [&[u8]; 4] = [b"caf\xe9", b"take", b"tone.wav", b"Wave"];
        let payload = 8 + fields.iter().map(|field| field.len()).sum::<usize>();
        footer.extend((payload as u16).to_le_bytes());
        for field in fields {
            wrapped(&mut footer, field);
        }
        footer.extend(8u16.to_le_bytes());
        footer.extend([0; 16]);
        let parsed = parse_footer(&footer, &BTreeSet::from([2083])).unwrap();
        assert_eq!(parsed.imports[0].0, 2083);
        assert_eq!(parsed.imports[0].1[0], "café");
        assert!(parse_footer(&footer[..footer.len() - 1], &BTreeSet::from([2083])).is_err());
        assert!(parse_footer(&footer, &BTreeSet::new()).is_err());
    }
    #[test]
    fn shipped_kit_names_resolve_and_unknown_names_preserve_silent_recovery_details() {
        for (kit, names) in [
            vec![
                "DanceMachine Sample-Kit Vol. 1",
                "gung -SAMPLE BOX- [Raps && Vocals]",
            ],
            vec![
                "DanceMachine Sample-Kit Vol. 2",
                "gung -SAMPLE BOX- [Drams && Synthies]",
            ],
            vec!["gung -SAMPLE BOX- [Space Sounds]"],
        ]
        .into_iter()
        .enumerate()
        {
            for name in names
                .into_iter()
                .chain(std::iter::once("Unknown replacement kit"))
            {
                let source_id = KIT_REGIONS[kit].0;
                let mut bytes = mix();
                place(&mut bytes, 0, 0, source_id, 0);
                bytes.extend([8, 10, 2, 0]);
                wrapped(&mut bytes, b"Kit demo");
                bytes.extend(3u16.to_le_bytes());
                for (index, (first, last)) in KIT_REGIONS.into_iter().enumerate() {
                    bytes.extend(first.to_le_bytes());
                    bytes.extend(last.to_le_bytes());
                    wrapped(&mut bytes, if index == kit { name.as_bytes() } else { b"" });
                }
                bytes.extend([0, 0, 8, 0]);
                bytes.extend([0; 16]);
                let project = convert(&bytes, Path::new("kit.mix"), &Library::empty(), |id| {
                    Ok((
                        Product::D1.global_sample_id(u32::from(id))?,
                        2,
                        format!("dmkit{}/sample.pxd", kit + 1),
                    ))
                })
                .unwrap();
                assert_eq!(project.clips.len(), 1);
                if name == "Unknown replacement kit" {
                    assert_eq!(project.missing_samples.len(), 1);
                    assert_eq!(project.missing_samples[0].metadata["kit_name"], name);
                    assert!(matches!(&project.missing_samples[0].reference,
                        MissingReference::Generated { kind, .. } if kind == "unresolved-kit"));
                    assert!(project.missing_samples[0].id >= IMPORTED_ID_START);
                } else {
                    assert!(project.missing_samples.is_empty(), "{name}");
                    assert_eq!(
                        project.clips[0].sample_id,
                        Product::D1.global_sample_id(u32::from(source_id)).unwrap()
                    );
                    assert_eq!(project.clips[0].length_ticks, 2 * TICKS_PER_BAR);
                }
            }
        }
    }

    #[test]
    fn remapped_kit_ranges_preserve_sources_but_invalid_ranges_reject() {
        let build = |first: u16, last: u16| {
            let mut bytes = mix();
            place(&mut bytes, 0, 0, 3300, 4);
            place(&mut bytes, 7, 0, 731, 0);
            bytes.extend([8, 10, 2, 0]);
            wrapped(&mut bytes, b"Remapped kit");
            bytes.extend(3u16.to_le_bytes());
            for (first, last) in [(first, last), (3900, 4499), (4500, 5199)] {
                bytes.extend(first.to_le_bytes());
                bytes.extend(last.to_le_bytes());
                wrapped(&mut bytes, b"");
            }
            bytes.extend([0, 0, 8, 0]);
            bytes.extend([0; 16]);
            bytes
        };
        let project = convert(
            &build(3300, 3899),
            Path::new("remapped.mix"),
            &Library::empty(),
            resolve,
        )
        .unwrap();
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.clips[0].start_tick, 64);
        assert_eq!(project.clips[0].length_ticks, 7 * TICKS_PER_BAR - 64);
        let missing = &project.missing_samples[0];
        assert_eq!(missing.metadata["source_id"], "3300");
        assert_eq!(missing.metadata["kit_first_id"], "3300");
        assert_eq!(missing.metadata["kit_last_id"], "3899");
        assert!(missing.length_inferred);
        assert!(matches!(&missing.reference,
            MissingReference::Generated { kind, .. } if kind == "unresolved-kit"));
        for invalid in [build(3300, 3900), build(3300, 3299), build(700, 3899)] {
            assert!(
                convert(
                    &invalid,
                    Path::new("invalid.mix"),
                    &Library::empty(),
                    resolve
                )
                .unwrap_err()
                .contains("invalid or overlap")
            );
        }
    }

    #[test]
    fn missing_catalog_audio_keeps_duration_metadata_and_every_placement() {
        let sample = crate::catalog::wave_metadata(Product::D1)
            .unwrap()
            .into_iter()
            .find(|sample| sample.native_id < u32::from(IMPORT_FIRST) && sample.bars > 1)
            .unwrap();
        let mut bytes = mix();
        place(&mut bytes, 0, 0, sample.native_id as u16, 0);
        place(&mut bytes, 12, 2, sample.native_id as u16, 4);
        let project = convert(&bytes, Path::new("missing.mix"), &Library::empty(), |_| {
            Err("Audio unavailable.".into())
        })
        .unwrap();
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.missing_samples.len(), 1);
        let missing = &project.missing_samples[0];
        assert!(!missing.length_inferred);
        assert_eq!(missing.nominal_ticks, sample.bars * TICKS_PER_BAR);
        assert_eq!(missing.name, sample.name);
        assert_eq!(missing.metadata["code"], sample.code);
        assert_eq!(missing.metadata["category"], sample.group_name);
        assert_eq!(project.clips[0].length_ticks, sample.bars * TICKS_PER_BAR);
        assert_eq!(project.clips[1].start_tick, 12 * TICKS_PER_BAR + 64);
    }

    #[test]
    fn missing_external_audio_retains_labels_path_category_and_inferred_extent() {
        let mut bytes = mix();
        place(&mut bytes, 0, 0, IMPORT_FIRST, 0);
        place(&mut bytes, 5, 0, 731, 0);
        place(&mut bytes, 10, 2, IMPORT_FIRST, 8);
        bytes.extend([8, 10, 1, 0]);
        wrapped(&mut bytes, b"Missing take");
        bytes.extend(1u16.to_le_bytes());
        let fields: [&[u8]; 4] = [b"caf\xe9", b"take", b"absent/take.wav", b"Voice"];
        bytes.extend(
            ((8 + fields.iter().map(|field| field.len()).sum::<usize>()) as u16).to_le_bytes(),
        );
        for field in fields {
            wrapped(&mut bytes, field);
        }
        bytes.extend(8u16.to_le_bytes());
        bytes.extend([0; 16]);
        let project =
            convert(&bytes, Path::new("missing.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(project.clips.len(), 3);
        assert_eq!(project.missing_samples.len(), 1);
        assert!(project.imported_samples.is_empty());
        let missing = &project.missing_samples[0];
        assert_eq!(missing.name, "café take");
        assert_eq!(missing.metadata["label_1"], "café");
        assert_eq!(missing.metadata["label_2"], "take");
        assert_eq!(missing.metadata["saved_path"], "absent/take.wav");
        assert_eq!(missing.metadata["category"], "Voice");
        assert_eq!(missing.metadata["source_id"], IMPORT_FIRST.to_string());
        assert_eq!(missing.nominal_ticks, 5 * TICKS_PER_BAR);
        assert!(missing.length_inferred);
        assert_eq!(project.clips[0].length_ticks, 5 * TICKS_PER_BAR);
        assert_eq!(project.clips[2].start_tick, 10 * TICKS_PER_BAR + 128);
        let encoded = serde_json::to_string(&project).unwrap();
        let reopened: Project = serde_json::from_str(&encoded).unwrap();
        reopened.validate().unwrap();
        assert_eq!(
            serde_json::to_value(reopened).unwrap(),
            serde_json::to_value(project).unwrap()
        );
    }

    #[test]
    fn external_wave_conflict_selects_first_for_all_repeated_placements() {
        let root = std::env::temp_dir().join(format!(
            "d1-conflict-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for (folder, value) in [("A", 0.1), ("B", 0.2)] {
            fs::create_dir_all(root.join(folder)).unwrap();
            crate::audio::Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: vec![value; 4410],
            }
            .write_wav(&root.join(folder).join("take.wav"))
            .unwrap();
        }
        let mut library = Library::empty();
        library.import_wav_folder(&root, 140.0).unwrap();
        let mut bytes = mix();
        place(&mut bytes, 0, 0, IMPORT_FIRST, 0);
        place(&mut bytes, 4, 1, IMPORT_FIRST, 0);
        bytes.extend([8, 10, 1, 0]);
        wrapped(&mut bytes, b"Conflicting take");
        bytes.extend(1u16.to_le_bytes());
        let fields: [&[u8]; 4] = [b"Recorded", b"take", b"old/take.wav", b"Voice"];
        bytes.extend(
            ((8 + fields.iter().map(|field| field.len()).sum::<usize>()) as u16).to_le_bytes(),
        );
        for field in fields {
            wrapped(&mut bytes, field);
        }
        bytes.extend(8u16.to_le_bytes());
        bytes.extend([0; 16]);
        let project = convert(&bytes, &root.join("song.mix"), &library, resolve).unwrap();
        assert!(project.missing_samples.is_empty());
        assert_eq!(project.sample_conflicts.len(), 1);
        let conflict = &project.sample_conflicts[0];
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.selected, 0);
        assert_eq!(conflict.sample_ids, vec![project.imported_samples[0].id]);
        assert_eq!(
            project.imported_samples[0].path,
            fs::canonicalize(root.join("A/take.wav")).unwrap()
        );
        assert_eq!(project.clips.len(), 2);
        assert!(
            project
                .clips
                .iter()
                .all(|clip| clip.sample_id == conflict.sample_ids[0])
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "Set MIXJAM_TECHNO_COMPAT_INSTALLATIONS to the extracted five-product media directory."]
    fn all_sixteen_shipped_d1_mixes_import_with_native_ids_and_complete_footers() {
        let base = PathBuf::from(
            std::env::var_os("MIXJAM_TECHNO_COMPAT_INSTALLATIONS")
                .expect("Set MIXJAM_TECHNO_COMPAT_INSTALLATIONS"),
        );
        let root = base.join("\x64ance1");
        let mut library = Library::empty();
        library
            .register(Product::D1, &root, Path::new("unused"))
            .unwrap();
        let mut count = 0;
        for entry in fs::read_dir(root.join("MIX")).unwrap() {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            let expected = bytes[2..5618]
                .as_chunks::<2>()
                .0
                .iter()
                .filter(|cell| **cell != [0, 0])
                .count();
            let project =
                load(&path, &library).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(project.clips.len(), expected);
            assert_eq!(project.bpm, 140.0);
            assert!(
                project
                    .clips
                    .iter()
                    .all(|clip| library.sample_by_id(clip.sample_id).is_some())
            );
            count += 1;
        }
        assert_eq!(count, 16);
    }
}
