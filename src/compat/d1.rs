//! D1's observed 0x0a06 grids and its two shipped footer encodings.
//! The common project uses ticks directly; T1's 180 BPM frame clock is not used.
use crate::{
    engine::{Clip, ImportedSample, Project, TICKS_PER_BAR, normalize_import},
    library::{Library, decode_1252},
    products::{IMPORTED_ID_START, Product},
};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::Read,
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
    let bytes = read_bounded(path, 16 * 1024 * 1024)?;
    convert(&bytes, path, library, |native_id| {
        let id = Product::D1.global_sample_id(u32::from(native_id))?;
        let sample = library
            .sample_by_id(id)
            .ok_or_else(|| format!("D1 sample {native_id} is not in the active library."))?;
        if sample.source_bpm != 140.0
            || (!library.is_wav_product(Product::D1) && sample.sample_rate != 44_100)
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
    regions: Vec<(u16, u16)>,
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
    let expected_imports: BTreeSet<_> = events
        .iter()
        .map(|e| e.sample)
        .filter(|id| (IMPORT_FIRST..KIT_FIRST).contains(id))
        .collect();
    let footer = parse_footer(&bytes[TRAILER_OFFSET..], &expected_imports)?;
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
        let imported_path = resolve_path(
            &fields[2],
            path.parent().unwrap_or_else(|| Path::new(".")),
            library,
        )?;
        let pcm = crate::audio::read_wav(&imported_path)?;
        if pcm.frames() == 0 {
            return Err("The D1 imported sample contains no audio.".into());
        }
        // New-project policy for external WAVs: retain their measured length in
        // a whole-bar placement envelope. Original D1 import rounding is unmeasured.
        let bars = (pcm.frames() as f64 / pcm.sample_rate as f64 * 140.0 / 240.0).ceil() as u32;
        let id = IMPORTED_ID_START
            .checked_add(project.imported_samples.len() as u32)
            .ok_or("Imported sample ID limit reached.")?;
        let name = format!("{} {}", fields[0], fields[1]).trim().to_owned();
        let name = if name.is_empty() {
            format!("Imported sample {source_id}")
        } else {
            name
        };
        project.imported_samples.push(ImportedSample {
            id,
            name,
            group_name: if fields[3].is_empty() {
                "Wave".into()
            } else {
                fields[3].clone()
            },
            path: imported_path,
            source_bpm: 140.0,
            origin: Some(Product::D1),
        });
        sources.insert(source_id, (id, bars, String::new()));
    }
    for event in &events {
        if sources.contains_key(&event.sample) {
            continue;
        }
        let source = resolve(event.sample)?;
        if source.1 == 0 || source.1 > 32766 {
            return Err("A D1 sample has an invalid native bar length.".into());
        }
        if let Some((kit, _)) = footer
            .regions
            .iter()
            .enumerate()
            .find(|(_, (first, last))| (*first..=*last).contains(&event.sample))
        {
            let prefix = format!("dmkit{}/", kit + 1);
            if !source
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
            });
        }
    }
    project
        .clips
        .sort_by_key(|clip| (clip.start_tick, clip.track));
    normalize_import(project)
}

fn parse_footer(bytes: &[u8], expected_imports: &BTreeSet<u16>) -> Result<Footer, String> {
    let mut footer = Footer {
        title: String::new(),
        routes: [0; TRACKS],
        regions: Vec::new(),
        imports: Vec::new(),
    };
    if bytes.is_empty() {
        if !expected_imports.is_empty() {
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
        for (kit, &(expected_first, expected_last)) in KIT_REGIONS.iter().enumerate().take(count) {
            let first = reader.word()?;
            let last = reader.word()?;
            let _name = reader.text(wrapped)?;
            let observed_old_boundary = !wrapped && kit == 1 && last == 4500;
            if first != expected_first || (last != expected_last && !observed_old_boundary) {
                return Err(
                    "The D1 MIX remaps expansion-kit IDs. That kit mapping is not supported."
                        .into(),
                );
            }
            footer.regions.push((first, last));
        }
    }
    let count = reader.word()? as usize;
    if count != expected_imports.len() {
        return Err("The D1 imported sample count does not match the referenced IDs.".into());
    }
    if !wrapped && count > 0 {
        return Err("Imported samples in the old plain footer are not supported.".into());
    }
    for &source_id in expected_imports {
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

fn resolve_path(text: &str, base: &Path, library: &Library) -> Result<PathBuf, String> {
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
    let path = match crate::preservation::resolve_asset(&path) {
        Ok(found) if found.is_file() => found,
        _ => library.resolve_wav_reference(text)?,
    };
    fs::canonicalize(path).map_err(|e| format!("Cannot open the D1 imported sample: {e}"))
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("The D1 MIX exceeds its file-size limit.".into());
    }
    Ok(bytes)
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
