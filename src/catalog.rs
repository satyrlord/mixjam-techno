//! Native catalog profiles. Native IDs come from saved indexes or recovered loaders.
//! Installation paths do not form part of sample identity.
use crate::products::Product;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct NativeSample {
    pub native_id: u32,
    pub code: String,
    pub name: String,
    pub group_name: String,
    pub bars: u32,
    pub source_bpm: f64,
    pub frame_count: u32,
    pub audio: NativeAudio,
}

#[derive(Clone, Debug)]
pub enum NativeAudio {
    Archive {
        path: PathBuf,
        offset: u64,
        length: u64,
    },
    Packed {
        path: PathBuf,
    },
}

/// Factual sample identity recovered from the source catalogs and headers.
/// This contains no original program or audio data.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveMetadata {
    pub native_id: u32,
    pub code: String,
    pub name: String,
    pub group_name: String,
    pub bars: u32,
    pub source_bpm: f64,
    pub frame_count: u32,
    /// Hex [`pcm_identity`] of the original decoded audio. Catalogs generated
    /// before this field exists leave it absent; those identities match by
    /// name and duration only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcm_digest: Option<String>,
}

impl WaveMetadata {
    pub fn pcm_identity(&self) -> Option<[u8; 16]> {
        let text = self.pcm_digest.as_deref()?;
        if text.len() != 32 {
            return None;
        }
        let mut digest = [0; 16];
        for (byte, pair) in digest.iter_mut().zip(text.as_bytes().chunks(2)) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        Some(digest)
    }
}

/// Identify decoded audio independently of file names: the first 16 bytes of
/// SHA-256 over the first differences of the mono PCM16 frames, omitting the
/// final frame. Decoders that start from a different predictor leave a constant
/// offset, and the original pads a missing final frame with silence where other
/// tools hold the last value; neither changes this digest. Any other change to
/// the samples does.
pub fn pcm_identity(samples: &[i16]) -> [u8; 16] {
    let mut identity = PcmIdentity::default();
    for &sample in samples {
        identity.push(sample);
    }
    identity.finish()
}

/// Streaming identity retains one pending difference so the final frame is omitted.
#[derive(Default)]
pub(crate) struct PcmIdentity {
    hasher: sha2::Sha256,
    previous: Option<i16>,
    pending: Option<[u8; 2]>,
}

impl PcmIdentity {
    pub(crate) fn push(&mut self, sample: i16) {
        use sha2::Digest;
        if let Some(difference) = self.pending.take() {
            self.hasher.update(difference);
        }
        if let Some(previous) = self.previous.replace(sample) {
            self.pending = Some(sample.wrapping_sub(previous).to_le_bytes());
        }
    }

    pub(crate) fn finish(self) -> [u8; 16] {
        use sha2::Digest;
        self.hasher.finalize()[..16].try_into().unwrap()
    }
}

pub fn pcm_identity_hex(samples: &[i16]) -> String {
    pcm_identity(samples)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn wave_metadata(product: Product) -> Result<Vec<WaveMetadata>, String> {
    let text = match product {
        Product::D1 => include_str!(concat!(env!("OUT_DIR"), "/d1.json")),
        Product::D2 => include_str!(concat!(env!("OUT_DIR"), "/d2.json")),
        Product::D3 => include_str!(concat!(env!("OUT_DIR"), "/d3.json")),
        Product::D4 => include_str!(concat!(env!("OUT_DIR"), "/d4.json")),
        Product::H1 => include_str!(concat!(env!("OUT_DIR"), "/h1.json")),
        Product::T1 => include_str!(concat!(env!("OUT_DIR"), "/t1.json")),
        Product::T2 => include_str!(concat!(env!("OUT_DIR"), "/t2.json")),
        Product::T3 => include_str!(concat!(env!("OUT_DIR"), "/t3.json")),
    };
    serde_json::from_str(text)
        .map_err(|error| format!("Cannot read bundled sample identities: {error}"))
}

pub fn has_native_catalog(product: Product, root: &Path) -> bool {
    let markers: &[&str] = match product {
        Product::T1 => &["\x65Jay/\x65Jay/\x50XD", "\x65Jay/\x65Jay/MAX"],
        Product::D1 => &["\x65Jay/\x65Jay/\x50xd\x64ance"],
        Product::D2 => &["d_\x65Jay2.inf", "../d_\x65Jay2.inf"],
        Product::D3 => &["\x65Jay/\x50XD/\x64ance30.inf"],
        Product::D4 => &["\x65Jay/\x50XD/\x64ance40.inf"],
        Product::H1 => &["\x65Jay/\x50XD/\x68ouse10.inf"],
        Product::T2 => &["\x65Jay/\x50XD/\x72ave20.inf"],
        Product::T3 => &["\x65Jay/\x50XD/\x72ave30.inf"],
    };
    markers.iter().any(|path| root.join(path).is_file())
}

pub fn load(product: Product, root: &Path) -> Result<Vec<NativeSample>, String> {
    let mut samples = match product {
        Product::T1 => load_t1(root)?,
        Product::D1 => load_d1(root)?,
        Product::D2 => load_d2(root)?,
        Product::D3 => load_packed(root, "\x64ance30", 140.0)?,
        Product::D4 => load_packed(root, "\x64ance40", 140.0)?,
        Product::H1 => load_packed(root, "\x68ouse10", 125.0)?,
        Product::T2 => load_packed(root, "\x72ave20", 140.0)?,
        Product::T3 => load_packed(root, "\x72ave30", 140.0)?,
    };
    let mut ids = HashSet::new();
    for sample in &samples {
        product.global_sample_id(sample.native_id)?;
        if !ids.insert(sample.native_id) {
            return Err(format!(
                "{} has duplicate native sample IDs.",
                product.label()
            ));
        }
    }
    if samples.is_empty() {
        return Err(format!(
            "{} has no registered audio samples.",
            product.label()
        ));
    }
    samples.sort_by_key(|sample| sample.native_id);
    Ok(samples)
}

fn load_t1(root: &Path) -> Result<Vec<NativeSample>, String> {
    let library = legacy_format::library::Library::load(root).map_err(|error| error.to_string())?;
    library
        .samples()
        .map(|metadata| {
            let path = relative_file(root, &format!("\x50XD/{}", metadata.path))?;
            let header = packed_header(
                &path,
                0,
                fs::metadata(&path).map_err(|e| e.to_string())?.len(),
            )?;
            Ok(NativeSample {
                native_id: u32::from(metadata.id),
                code: metadata.path.clone(),
                name: metadata.labels.join(" ").trim().to_owned(),
                group_name: metadata.category.clone(),
                bars: metadata.nominal_bars.unwrap_or(1).max(1),
                source_bpm: 180.0,
                frame_count: header.decoded_samples as u32,
                audio: NativeAudio::Packed { path },
            })
        })
        .collect()
}

fn load_d1(root: &Path) -> Result<Vec<NativeSample>, String> {
    // deinst_d1 SHA256 853d17575d218daf74f099c888265c6fe9f57fe4c08cd1d3711a80109ef763ec:
    // The loader initializes the kit destination ID to 0x2db (731).
    // The shipped dream.mix and space.mix footers declare the three kit regions.
    // MAX positions are separate lookup slots and must not replace these IDs.
    let profiles = [
        ("\x50xd\x64ance", "", 731u32),
        ("kit1.txt", "dmkit1/", 3400),
        ("kit2.txt", "dmkit2/", 3900),
        ("kit3.txt", "dmkit3/", 4500),
    ];
    let mut samples = Vec::new();
    for (catalog, prefix, first_id) in profiles {
        let path = relative_file(root, &format!("\x65Jay/\x65Jay/{catalog}"))?;
        let text = read_text(&path)?;
        let lines: Vec<_> = text.lines().collect();
        if lines.is_empty() || !lines.len().is_multiple_of(6) {
            return Err(format!("Catalog {catalog} must have six lines per sample."));
        }
        for (index, fields) in lines.as_chunks::<6>().0.iter().enumerate() {
            let sample_path = field(fields[0]);
            let path = relative_file(root, &format!("\x50XD/{prefix}{sample_path}"))?;
            let length = fs::metadata(&path).map_err(|e| e.to_string())?.len();
            let header = packed_header(&path, 0, length)?;
            let bars = field(fields[3])
                .parse::<u32>()
                .map_err(|_| format!("Catalog {catalog} has an invalid bar count."))?;
            if bars == 0 {
                return Err(format!("Catalog {catalog} has a zero bar count."));
            }
            samples.push(NativeSample {
                native_id: first_id
                    .checked_add(index as u32)
                    .ok_or("Native sample ID limit reached.")?,
                code: format!("{prefix}{sample_path}"),
                name: format!("{} {}", field(fields[4]), field(fields[5]))
                    .trim()
                    .to_owned(),
                group_name: field(fields[2]),
                bars,
                source_bpm: 140.0,
                frame_count: header.decoded_samples as u32,
                audio: NativeAudio::Packed { path },
            });
        }
    }
    Ok(samples)
}

fn load_packed(root: &Path, archive: &str, bpm: f64) -> Result<Vec<NativeSample>, String> {
    let folder = relative_file(root, "\x65Jay/\x50XD")?;
    let index = relative_file(&folder, &format!("{archive}.inf"))?;
    let house = archive.eq_ignore_ascii_case("\x68ouse10");
    let archive = relative_file(&folder, archive)?;
    let mut samples = load_index(&index, bpm, false, |_| Ok(archive.clone()))?;
    if house {
        for sample in &mut samples {
            if sample.group_name.starts_with("Group ")
                && let Some(group) = house_group_name(&sample.code)
            {
                sample.group_name = group.to_owned();
            }
        }
    }
    Ok(samples)
}

/// House index records carry numeric groups without labels. The shipped
/// program's string table names the sample groups (House10.exe,
/// SP_LOOP, SP_DRUM, SP_BASS, SP_EFFECTS, SP_KEYS, SP_GUITARS, SP_XTRA,
/// SP_GROOVE, SP_VOICE). The kind letters that follow the `HS1` prefix and the
/// bank letter in each code select the group; drum pads keep their header labels.
fn house_group_name(code: &str) -> Option<&'static str> {
    let kind: String = code
        .get(4..)?
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect();
    Some(match kind.as_str() {
        "LA" => "Loop",
        "BS" => "Bass",
        "FX" => "Effect",
        "GT" => "Guitar",
        "EX" => "Groove",
        "SX" => "Xtra",
        "ON" | "PN" | "SY" => "Keys",
        kind if kind.starts_with('V') => "Voice",
        kind if kind.starts_with('D') => "Drum",
        _ => return None,
    })
}

fn load_d2(root: &Path) -> Result<Vec<NativeSample>, String> {
    // The installer puts the master index beside D2, rather than beside the kit index.
    // Its quoted SAMPLES marker and record order differ from archive .inf files.
    let (index, folder) = if let Ok(index) = relative_file(root, "d_\x65Jay2.inf") {
        let folder = relative_file(root, "D2/\x50XD").or_else(|_| relative_file(root, "\x50XD"))?;
        (index, folder)
    } else {
        let parent = root.parent().ok_or("Cannot locate the D2 master index.")?;
        (
            relative_file(parent, "d_\x65Jay2.inf")?,
            relative_file(root, "\x50XD")?,
        )
    };
    load_index(&index, 140.0, true, |kind| {
        let name = match kind {
            2 => "\x64ance20",
            3 => "\x44ancesk4",
            4 => "\x44ancesk5",
            5 => "\x44ancesk6",
            _ => {
                return Err(format!("D2 sample archive type {kind} is not supported."));
            }
        };
        relative_file(&folder, name)
    })
}

#[derive(Debug)]
struct IndexRecord {
    id: u32,
    kind: u32,
    code: String,
    offset: u64,
    length: u64,
    name: String,
    bars: u32,
    group: u32,
}

fn index_records(text: &str) -> Result<Vec<IndexRecord>, String> {
    let lines: Vec<_> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| field(line) == "[SAMPLES]")
        .ok_or("The sample index has no SAMPLES section.")?
        + 1;
    let rows = &lines[start..];
    if rows.is_empty() || !rows.len().is_multiple_of(12) || rows.len() / 12 > 100_000 {
        return Err("The sample index has an incomplete or excessive record list.".into());
    }
    rows.as_chunks::<12>()
        .0
        .iter()
        .map(|row| {
            let number = |index: usize| {
                field(row[index])
                    .parse::<u64>()
                    .map_err(|_| "The sample index has an invalid number.".to_owned())
            };
            let small = |index| {
                u32::try_from(number(index)?)
                    .map_err(|_| "A sample field exceeds its limit.".to_owned())
            };
            Ok(IndexRecord {
                id: small(0)?,
                kind: small(1)?,
                code: field(row[2]),
                offset: number(3)?,
                length: number(4)?,
                name: format!("{} {}", field(row[5]), field(row[6]))
                    .trim()
                    .to_owned(),
                bars: small(7)?,
                group: small(8)?,
            })
        })
        .collect()
}

fn load_index(
    index: &Path,
    bpm: f64,
    d2: bool,
    mut archive_for: impl FnMut(u32) -> Result<PathBuf, String>,
) -> Result<Vec<NativeSample>, String> {
    let records = index_records(&read_text(index)?)?;
    let mut samples = Vec::new();
    let mut ranges: BTreeMap<PathBuf, Vec<(u64, u64)>> = BTreeMap::new();
    for record in records {
        // Type 10 records are Drum Machine state, not compressed PCM. They remain
        // available in the original master index for the song adapter.
        if d2 && record.kind == 10 {
            if record.offset != 0 || record.length != 0 {
                return Err(
                    "A Drum Machine catalog record contains an invalid audio range.".into(),
                );
            }
            continue;
        }
        if !d2 && !matches!(record.kind, 3 | 6) {
            return Err(format!(
                "Native sample {} has an unsupported archive type.",
                record.id
            ));
        }
        if record.id == 0 || record.length < 12 || record.length > 64 * 1024 * 1024 {
            return Err(format!(
                "Native sample {} has an invalid ID or length.",
                record.id
            ));
        }
        let path = archive_for(record.kind)?;
        let archive_size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
        let end = record
            .offset
            .checked_add(record.length)
            .filter(|end| *end <= archive_size)
            .ok_or_else(|| format!("Native sample {} exceeds its archive.", record.id))?;
        let header = packed_header(&path, record.offset, record.length)?;
        let fields = header.metadata_fields();
        let group_name = if fields.len() >= 5 && !fields[4].is_empty() {
            fields[4].to_owned()
        } else {
            format!("Group {}", record.group)
        };
        // One shipped D2 record (7051) has zero bars/group in its index.
        // Use measured audio length for its display length, without changing ID.
        let bars = if record.bars == 0 {
            ((header.decoded_samples as f64 * bpm / 44_100.0 / 240.0).ceil() as u32).max(1)
        } else {
            record.bars
        };
        ranges
            .entry(path.clone())
            .or_default()
            .push((record.offset, end));
        samples.push(NativeSample {
            native_id: record.id,
            code: record.code,
            name: record.name,
            group_name,
            bars,
            source_bpm: bpm,
            frame_count: header.decoded_samples as u32,
            audio: NativeAudio::Archive {
                path,
                offset: record.offset,
                length: record.length,
            },
        });
    }
    for (path, mut ranges) in ranges {
        ranges.sort_unstable();
        let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
        let mut end = 0;
        for (start, next) in ranges {
            let unregistered_kit6_record = d2
                && path
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("\x44ancesk6"))
                && end == 43_318_902
                && start == 43_525_770;
            if start != end && !unregistered_kit6_record {
                return Err(format!(
                    "Archive {} has overlapping or incomplete sample ranges.",
                    path.display()
                ));
            }
            end = next;
        }
        if end != size {
            return Err(format!(
                "Archive {} has unindexed trailing data.",
                path.display()
            ));
        }
        // D2 kit 6 contains an unregistered record (ID/type 0 in its side
        // index). The master omits it. Do not invent an ID to fill that gap.
    }
    Ok(samples)
}

fn packed_header(
    path: &Path,
    offset: u64,
    length: u64,
) -> Result<legacy_format::packed::PackedHeader, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut prefix = vec![0; length.min(512) as usize];
    file.read_exact(&mut prefix).map_err(|e| e.to_string())?;
    let header = legacy_format::packed::parse_header(&prefix).map_err(|e| {
        format!(
            "Cannot read sample header in {} at {offset}: {e}",
            path.display()
        )
    })?;
    if header.decoded_samples == 0 {
        return Err("The native sample contains no audio frames.".into());
    }
    Ok(header)
}

fn read_text(path: &Path) -> Result<String, String> {
    let length = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if length > 32 * 1024 * 1024 {
        return Err("The sample catalog exceeds its size limit.".into());
    }
    fs::read(path)
        .map(|bytes| crate::library::decode_1252(&bytes))
        .map_err(|e| e.to_string())
}

fn field(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(text)
        .replace("\"\"", "\"")
}

fn relative_file(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let normalized = relative.replace('\\', "/");
    let mut path = root.to_path_buf();
    for component in Path::new(&normalized).components() {
        let Component::Normal(name) = component else {
            return Err("A native catalog path is not relative.".into());
        };
        let direct = path.join(name);
        if direct.exists() {
            path = direct;
            continue;
        }
        let expected = name.to_string_lossy();
        let matches = fs::read_dir(&path)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&expected)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "Native library item {} is missing or ambiguous.",
                direct.display()
            ));
        }
        path = matches.into_iter().next().unwrap();
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn pcm_identity_ignores_constant_offset_and_final_frame_but_not_other_changes() {
        let original: Vec<i16> = (0..64).map(|i| (i * 31 % 97 - 48) as i16).collect();
        let shifted: Vec<i16> = original.iter().map(|s| s - 178).collect();
        let mut held = original.clone();
        held[63] = held[62];
        let mut changed = original.clone();
        changed[20] += 1;
        let expected = pcm_identity(&original);
        assert_eq!(pcm_identity(&shifted), expected);
        assert_eq!(pcm_identity(&held), expected);
        assert_ne!(pcm_identity(&changed), expected);
        assert_ne!(pcm_identity(&original[..63]), expected);
        let metadata = WaveMetadata {
            native_id: 1,
            code: String::new(),
            name: String::new(),
            group_name: String::new(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 64,
            pcm_digest: Some(pcm_identity_hex(&original)),
        };
        assert_eq!(metadata.pcm_identity(), Some(expected));
        for (code, group) in [
            ("HS1AEX001(L)", Some("Groove")),
            ("HS1DSX010", Some("Xtra")),
            ("HS1BVA047(L)", Some("Voice")),
            ("HS1EDF051", Some("Drum")),
            ("HS1CON033(R)", Some("Keys")),
            ("HS1FGT001", Some("Guitar")),
            ("HS1CLA001", Some("Loop")),
            ("HS1EBS001", Some("Bass")),
            ("HS1EFX072", Some("Effect")),
            ("HS1", None),
            ("HS1Q9", None),
        ] {
            assert_eq!(house_group_name(code), group, "{code}");
        }
        let text = serde_json::to_string(&metadata).unwrap();
        assert!(text.contains("pcm_digest"));
        let legacy: WaveMetadata = serde_json::from_str(&text.replace(
            &format!(
                ",\"pcm_digest\":\"{}\"",
                metadata.pcm_digest.as_deref().unwrap()
            ),
            "",
        ))
        .unwrap();
        assert_eq!(legacy.pcm_identity(), None);
        assert_eq!(pcm_identity(&[]), pcm_identity(&[7]));
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    #[test]
    #[cfg_attr(
        not(bundled_catalog),
        ignore = "Needs the locally generated sample catalogs."
    )]
    fn bundled_identities_are_complete_valid_and_do_not_need_research_files() {
        let expected = [2845, 3795, 5049, 5330, 3563, 3017, 4015, 4006];
        let mut total = 0;
        for (product, count) in Product::ALL.into_iter().zip(expected) {
            let metadata = wave_metadata(product).unwrap();
            assert_eq!(metadata.len(), count, "{}", product.label());
            let mut ids = HashSet::new();
            for entry in &metadata {
                assert!(ids.insert(product.global_sample_id(entry.native_id).unwrap()));
                assert!(entry.bars > 0 && entry.frame_count > 0);
                assert!(entry.source_bpm.is_finite() && entry.source_bpm > 0.0);
                assert!(!entry.name.is_empty() && !entry.code.is_empty());
            }
            total += metadata.len();
        }
        assert_eq!(total, 31_620);
    }
    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "mixjam-techno-catalog-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn packed(metadata: &str, frames: u32) -> Vec<u8> {
        let mut bytes = b"t\x50xD".to_vec();
        bytes.push((metadata.len() + 2) as u8);
        bytes.extend(metadata.as_bytes());
        bytes.extend([0, 0, b'T']);
        bytes.extend(frames.to_le_bytes());
        bytes.extend([0, 0, 0]);
        bytes
    }
    fn row(id: u32, kind: u32, offset: u64, length: usize, bars: u32) -> String {
        format!(
            "{id}\n{kind}\n\"CODE{id}\"\n{offset}\n{length}\n\"name\"\n\"{id}\"\n{bars}\n18\n\"\"\n0\n\"\"\n"
        )
    }
    #[test]
    fn quoted_marker_unsorted_multibank_index_and_short_metadata_keep_native_ids() {
        let folder = Folder::new();
        let metadata = "sound\r\n1\r\n\x44ance20\r\n\r\nKick\r\n75600";
        let first = packed(metadata, 9450);
        let second = packed(metadata, 18900);
        fs::write(
            folder.0.join("bank"),
            [first.clone(), second.clone()].concat(),
        )
        .unwrap();
        fs::write(folder.0.join("kit"), first.clone()).unwrap();
        let index = folder.0.join("index.inf");
        fs::write(
            &index,
            format!(
                "\"[SAMPLES]\"\n{}{}{}{}",
                row(2, 2, first.len() as u64, second.len(), 1),
                row(1, 2, 0, first.len(), 0),
                row(5100, 3, 0, first.len(), 1),
                row(7200, 10, 0, 0, 1)
            ),
        )
        .unwrap();
        let samples = load_index(&index, 140.0, true, |kind| {
            Ok(folder.0.join(if kind == 2 { "bank" } else { "kit" }))
        })
        .unwrap();
        assert_eq!(
            samples.iter().map(|s| s.native_id).collect::<Vec<_>>(),
            [2, 1, 5100]
        );
        assert_eq!(samples[1].bars, 1);
        assert_eq!(samples[1].group_name, "Kick");
        assert_eq!(samples[0].frame_count, 18900);
        fs::write(
            &index,
            format!(
                "[SAMPLES]\n{}{}",
                row(1, 2, 0, first.len(), 1),
                row(2, 2, 1, first.len(), 1)
            ),
        )
        .unwrap();
        assert!(load_index(&index, 140.0, true, |_| Ok(folder.0.join("bank"))).is_err());
    }
    #[test]
    fn path_and_catalog_bounds_reject_traversal_truncation_and_excessive_fields() {
        let folder = Folder::new();
        assert!(relative_file(&folder.0, "../escape.\x70xd").is_err());
        assert!(relative_file(&folder.0, "C:/escape.\x70xd").is_err());
        assert!(index_records("[SAMPLES]\n1\n2\n").is_err());
        assert!(index_records(&format!("[SAMPLES]\n{}", row(1, 6, u64::MAX, 20, 1))).is_ok());
        let index = folder.0.join("index.inf");
        let bank = folder.0.join("bank");
        fs::write(&bank, packed("sample", 5)).unwrap();
        fs::write(&index, format!("[SAMPLES]\n{}", row(1, 6, u64::MAX, 20, 1))).unwrap();
        assert!(load_index(&index, 140.0, false, |_| Ok(bank.clone())).is_err());
    }
    #[test]
    fn d1_loader_ids_do_not_use_max_slots_and_numeric_secondary_fields_are_valid() {
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join("\x65Jay/\x65Jay")).unwrap();
        for (name, prefix) in [
            ("\x50xd\x64ance", ""),
            ("kit1.txt", "dmkit1/"),
            ("kit2.txt", "dmkit2/"),
            ("kit3.txt", "dmkit3/"),
        ] {
            let path = folder.0.join(format!("\x50XD/{prefix}bank/sound.\x70xd"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, packed("name\r\n1", 5)).unwrap();
            fs::write(
                folder.0.join("\x65Jay/\x65Jay").join(name),
                "\"bank\\sound.\x70xd\"\n4428\n\"bass\"\n\"1\"\n\"sound\"\n\"1\"\n",
            )
            .unwrap();
        }
        fs::write(
            folder.0.join("\x65Jay/\x65Jay/MAX"),
            "unrelated-slot.\x70xd\n",
        )
        .unwrap();
        let samples = load(Product::D1, &folder.0).unwrap();
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.native_id)
                .collect::<Vec<_>>(),
            [731, 3400, 3900, 4500]
        );
        assert_eq!(samples[0].code, "bank\\sound.\x70xd");
    }
    #[test]
    #[ignore = "Set MIXJAM_TECHNO_COMPAT_INSTALLATIONS to the extracted five-product media directory."]
    fn all_eight_native_catalogs_keep_registered_ids_and_resolve_original_headers() {
        let base = PathBuf::from(
            std::env::var_os("MIXJAM_TECHNO_COMPAT_INSTALLATIONS")
                .expect("Set MIXJAM_TECHNO_COMPAT_INSTALLATIONS"),
        );
        let profiles = [
            (Product::D1, base.join("\x64ance1"), 2845),
            (Product::D2, base.join("\x64ance2"), 3795),
            (Product::D3, base.join("\x64ance3"), 5049),
            (Product::D4, base.join("\x64ance4"), 5330),
            (Product::H1, base.join("\x68ouse"), 3563),
            (
                Product::T2,
                crate::project_io::default_root(Product::T2).expect("T2 fixtures"),
                4015,
            ),
            (
                Product::T1,
                crate::project_io::default_root(Product::T1).expect("T1 fixtures"),
                3017,
            ),
            (
                Product::T3,
                crate::project_io::default_root(Product::T3).expect("T3 fixtures"),
                4006,
            ),
        ];
        for (product, root, expected) in profiles {
            let samples =
                load(product, &root).unwrap_or_else(|e| panic!("{}: {e}", product.label()));
            assert_eq!(samples.len(), expected, "{}", product.label());
            assert!(samples.iter().all(|s| s.frame_count > 0 && s.bars > 0));
            if product == Product::D1 {
                assert_eq!(
                    samples.iter().find(|s| s.native_id == 731).unwrap().code,
                    "bm\\asjo.\x70xd"
                );
                let ids: HashSet<_> = samples.iter().map(|s| s.native_id).collect();
                for entry in fs::read_dir(root.join("MIX")).unwrap() {
                    let bytes = fs::read(entry.unwrap().path()).unwrap();
                    for cell in bytes[2..5618].as_chunks::<2>().0 {
                        let id = u32::from(u16::from_le_bytes([cell[0], cell[1]]));
                        assert!(id == 0 || ids.contains(&id), "Unknown D1 fixture ID {id}");
                    }
                }
            }
        }
    }
}
