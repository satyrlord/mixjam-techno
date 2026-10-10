//! Installed catalog metadata with native factory and kit IDs established by code.
use crate::mix::Catalog;
use std::{collections::BTreeMap, fmt, fs, path::Path};

#[derive(Debug)]
pub struct LibraryError(pub String);
impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for LibraryError {}
impl From<std::io::Error> for LibraryError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRecord {
    /// Decoded fields, including the unclassified second field and length text.
    pub fields: [String; 6],
    pub nominal_bars: u32,
}

/// Parse the observed six quoted lines per record. Preserve text inside quotes.
pub fn parse_catalog(bytes: &[u8]) -> Result<Vec<CatalogRecord>, LibraryError> {
    let text: String = bytes.iter().map(|&b| char::from(b)).collect();
    let lines: Vec<_> = text.lines().collect();
    if lines.len() % 6 != 0 {
        return Err(LibraryError("Catalog needs six lines per record".into()));
    }
    let mut out = Vec::with_capacity(lines.len() / 6);
    for (index, record) in lines.as_chunks::<6>().0.iter().enumerate() {
        let mut fields = std::array::from_fn(|_| String::new());
        for (target, line) in fields.iter_mut().zip(record) {
            let line = line.trim();
            let inner = line
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or_else(|| {
                    LibraryError(format!("Catalog record {index} has an unquoted field"))
                })?;
            // VB Input # uses doubled quotation marks inside quoted fields.
            let mut chars = inner.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '"' && chars.next() != Some('"') {
                    return Err(LibraryError(format!(
                        "Catalog record {index} has an invalid quote"
                    )));
                }
                target.push(c);
            }
        }
        validate_relative_path(&fields[0])?;
        let nominal_bars = fields[3].parse::<u32>().map_err(|_| {
            LibraryError(format!(
                "Catalog record {index} has an invalid integer length"
            ))
        })?;
        if !(1..=32766).contains(&nominal_bars) {
            return Err(LibraryError(format!(
                "Catalog record {index} has an unsupported signed length"
            )));
        }
        out.push(CatalogRecord {
            fields,
            nominal_bars,
        });
    }
    Ok(out)
}

fn validate_relative_path(path: &str) -> Result<(), LibraryError> {
    if path.is_empty() {
        return Err(LibraryError("Sample path is empty".into()));
    }
    Catalog::parse(path.as_bytes()).map_err(|e| LibraryError(e.to_string()))?;
    if path.contains(['\r', '\n', '\0']) {
        return Err(LibraryError("Sample path contains a control byte".into()));
    }
    Ok(())
}
fn key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleMetadata {
    /// Original factory or kit ID. Imported records need separate MIX remapping.
    pub id: u16,
    /// Auxiliary MAX position. It must not replace the native sample ID.
    pub max_slot: Option<u16>,
    pub path: String,
    /// Native catalog integer, stored in the original per-sample length table.
    pub nominal_bars: Option<u32>,
    pub category: String,
    pub group: String,
    pub labels: [String; 2],
    pub secondary: String,
    pub source_catalog: String,
    pub raw_fields: Option<[String; 6]>,
}

#[derive(Debug, Clone)]
pub struct Library {
    samples: BTreeMap<u16, SampleMetadata>,
    max: Catalog,
    pub step_synth: BTreeMap<String, (String, String)>,
}
impl Library {
    pub fn load(root: &Path) -> Result<Self, LibraryError> {
        let resources = root.join("\x65Jay/\x65Jay");
        let max = Catalog::parse(&fs::read(resources.join("MAX"))?)
            .map_err(|e| LibraryError(e.to_string()))?;
        if max.len() > usize::from(u16::MAX) + 1 {
            return Err(LibraryError("MAX has too many slots".into()));
        }
        let max_by_path: BTreeMap<_, _> = (0..max.len())
            .filter_map(|id| max.get(id as u16).map(|path| (key(path), id as u16)))
            .collect();
        let catalogs = [
            ("", "\x50XD", 731u16),
            ("rekit1/", "KIT1.TXT", 4400),
            ("rekit2/", "KIT2.TXT", 5000),
            ("rekit3/", "KIT3.TXT", 5625),
        ];
        let mut samples = BTreeMap::new();
        for (prefix, name, first_id) in catalogs {
            for (index, record) in parse_catalog(&fs::read(resources.join(name))?)?
                .into_iter()
                .enumerate()
            {
                let id = usize::from(first_id)
                    .checked_add(index)
                    .filter(|&id| id <= 32767)
                    .ok_or_else(|| {
                        LibraryError("Catalog exceeds native signed sample ID range".into())
                    })? as u16;
                let path = format!("{prefix}{}", record.fields[0].replace('\\', "/"));
                let entry = SampleMetadata {
                    id,
                    max_slot: max_by_path.get(&key(&path)).copied(),
                    path,
                    nominal_bars: Some(record.nominal_bars),
                    category: record.fields[2].clone(),
                    group: String::new(),
                    labels: [record.fields[4].clone(), record.fields[5].clone()],
                    secondary: record.fields[1].clone(),
                    source_catalog: name.into(),
                    raw_fields: Some(record.fields),
                };
                if samples.insert(id, entry).is_some() {
                    return Err(LibraryError("Catalog exceeds its native ID region".into()));
                }
            }
        }
        let step_synth = parse_step_synth(&fs::read(resources.join("\x48YPSOUND"))?)?;
        Ok(Self {
            samples,
            max,
            step_synth,
        })
    }
    pub fn sample(&self, id: u16) -> Option<&SampleMetadata> {
        self.samples.get(&id)
    }
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    pub fn samples(&self) -> impl Iterator<Item = &SampleMetadata> {
        self.samples.values()
    }
    pub fn max_slot_path(&self, slot: u16) -> Option<&str> {
        self.max.get(slot)
    }
    pub fn max_slots(&self) -> usize {
        self.max.len()
    }
    /// Original keys concatenate label 1 and label 2, keeping the last 250 characters.
    /// Equal keys retain catalog order here; native qsort tie order is not established.
    pub fn sorted_samples(&self, category: &str, sort: SampleSort) -> Vec<&SampleMetadata> {
        let mut entries: Vec<_> = self.samples().filter(|s| s.category == category).collect();
        if sort != SampleSort::Catalog {
            entries.sort_by_key(|s| {
                let label = format!("{}{}", s.labels[0], s.labels[1]);
                let chars: Vec<_> = label.chars().collect();
                let suffix: String = chars[chars.len().saturating_sub(250)..].iter().collect();
                let prefix = if sort == SampleSort::Length {
                    format!("{:02}", s.nominal_bars.unwrap_or(0))
                } else {
                    String::new()
                };
                format!("{prefix}{suffix}").to_ascii_lowercase()
            });
        }
        entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleSort {
    Catalog,
    Name,
    Length,
}

fn parse_step_synth(bytes: &[u8]) -> Result<BTreeMap<String, (String, String)>, LibraryError> {
    let text: String = bytes.iter().map(|&b| char::from(b)).collect();
    let lines: Vec<_> = text.lines().collect();
    let mut out = BTreeMap::new();
    let mut pos = 0;
    while pos < lines.len() {
        let group = lines[pos];
        let count = lines
            .get(pos + 1)
            .ok_or_else(|| LibraryError("Incomplete Step synth group".into()))?
            .parse::<usize>()
            .map_err(|_| LibraryError("Invalid Step synth group count".into()))?;
        pos += 2;
        if count > (lines.len() - pos) / 2 {
            return Err(LibraryError("Incomplete Step synth records".into()));
        }
        for _ in 0..count {
            validate_relative_path(lines[pos])?;
            let path = format!("\x68yper/{}", key(lines[pos]));
            if out
                .insert(path, (group.into(), lines[pos + 1].into()))
                .is_some()
            {
                return Err(LibraryError("Duplicate Step synth sound path".into()));
            }
            pos += 2;
        }
    }
    Ok(out)
}

/// Narrow decoder for the common saved footer observed in this installation.
/// Unknown prefix bytes and extra records are retained, never reserialized here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixTrailer {
    pub raw: Vec<u8>,
    pub routes: [u16; 8],
    /// Extra data can contain imported sample overrides. Do not ignore it in playback.
    pub unresolved_import: bool,
    /// Original factory-only loader closes after routes, leaving this suffix unread.
    pub extra_ignored: bool,
    pub extra: Vec<u8>,
}
impl MixTrailer {
    pub fn parse(bytes: &[u8]) -> Result<Self, LibraryError> {
        const PREFIX: &[u8] = b"\x08\x0a\x01\0\x10\0\x52ave \x65Jay 1.01\0\x01\0\0";
        if !bytes.starts_with(PREFIX) || bytes.get(24..26) != Some(&[8, 0]) || bytes.len() < 42 {
            return Err(LibraryError(
                "Unsupported or incomplete MIX footer layout".into(),
            ));
        }
        let routes =
            std::array::from_fn(|i| u16::from_le_bytes([bytes[26 + i * 2], bytes[27 + i * 2]]));
        if routes.iter().any(|&r| r > 3) {
            return Err(LibraryError("Unsupported track speaker mode".into()));
        }
        let extra = bytes[42..].to_vec();
        Ok(Self {
            raw: bytes.to_vec(),
            routes,
            unresolved_import: !extra.is_empty(),
            extra_ignored: false,
            extra,
        })
    }
    /// Apply the proven original footer read boundary for factory-only arrangements.
    /// Imported and kit-remapped MIX layouts need separate decoding.
    pub fn for_mix(mix: &crate::mix::Mix, library: &Library) -> Result<Self, LibraryError> {
        let mut trailer = Self::parse(&mix.trailer)?;
        let factory_only = mix.events().all(|event| {
            library.sample(event.sample_id).is_some_and(|sample| {
                sample.source_catalog == "\x50XD" && sample.secondary.is_empty()
            })
        });
        trailer.unresolved_import = !factory_only;
        trailer.extra_ignored = factory_only && !trailer.extra.is_empty();
        Ok(trailer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_preserves_labels_secondary_and_signed_length_bound() {
        let input = b"\"ba\\one.\x70xd\"\r\n\"opaque\"\r\n\"bass\"\r\n\"2\"\r\n\" a \"\r\n\"say \"\"yes\"\"\"\r\n";
        let records = parse_catalog(input).unwrap();
        assert_eq!(records[0].nominal_bars, 2);
        assert_eq!(records[0].fields[1], "opaque");
        assert_eq!(records[0].fields[4], " a ");
        assert_eq!(records[0].fields[5], "say \"yes\"");
        for invalid in [
            b"\"x\"".as_slice(),
            b"\"../x\"\n\"\"\n\"bass\"\n\"2\"\n\"a\"\n\"b\"",
        ] {
            assert!(parse_catalog(invalid).is_err());
        }
        let invalid = String::from_utf8_lossy(input).replace("\"2\"", "\"32767\"");
        assert!(parse_catalog(invalid.as_bytes()).is_err());
    }
    #[test]
    fn step_synth_groups_have_bounded_counts_and_unique_paths() {
        assert_eq!(
            parse_step_synth(b"Vibes\n1\nr1.\x70xd\nPeel\n").unwrap()["\x68yper/r1.\x70xd"],
            ("Vibes".into(), "Peel".into())
        );
        assert!(parse_step_synth(b"Vibes\n999999999999999999999999999999\n").is_err());
        assert!(parse_step_synth(b"Vibes\n1\n").is_err());
    }
    #[test]
    fn display_sort_uses_both_labels_and_native_integer_length() {
        let samples = [
            (731, 2, ["Alpha", "Z"]),
            (732, 1, ["Alpha", "a"]),
            (733, 2, ["Alpha", "B"]),
        ]
        .into_iter()
        .map(|(id, length, labels)| {
            (
                id,
                SampleMetadata {
                    id,
                    max_slot: None,
                    path: format!("{id}.\x70xd"),
                    nominal_bars: Some(length),
                    category: "bass".into(),
                    group: String::new(),
                    labels: labels.map(String::from),
                    secondary: String::new(),
                    source_catalog: "\x50XD".into(),
                    raw_fields: None,
                },
            )
        })
        .collect();
        let library = Library {
            samples,
            max: Catalog::parse(b"").unwrap(),
            step_synth: BTreeMap::new(),
        };
        let ids = |sort| {
            library
                .sorted_samples("bass", sort)
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(SampleSort::Catalog), [731, 732, 733]);
        assert_eq!(ids(SampleSort::Name), [732, 733, 731]);
        assert_eq!(ids(SampleSort::Length), [732, 733, 731]);
        assert!(library.sorted_samples("voice", SampleSort::Name).is_empty());
    }
    #[test]
    fn footer_keeps_overrides_and_rejects_partial_or_unsupported_modes() {
        let mut bytes = b"\x08\x0a\x01\0\x10\0\x52ave \x65Jay 1.01\0\x01\0\0\x08\0".to_vec();
        for route in [0u16, 0, 0, 0, 1, 2, 1, 2] {
            bytes.extend(route.to_le_bytes());
        }
        let parsed = MixTrailer::parse(&bytes).unwrap();
        assert_eq!(parsed.routes, [0, 0, 0, 0, 1, 2, 1, 2]);
        assert!(!parsed.unresolved_import);
        bytes.extend_from_slice(b"opaque override");
        let parsed = MixTrailer::parse(&bytes).unwrap();
        assert!(parsed.unresolved_import);
        assert_eq!(parsed.raw, bytes);
        for length in 0..42 {
            assert!(MixTrailer::parse(&bytes[..length]).is_err());
        }
        bytes[26] = 4;
        assert!(MixTrailer::parse(&bytes).is_err());
    }
}
