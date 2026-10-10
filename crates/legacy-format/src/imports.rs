//! MIX import records and wrapped byte strings recovered from original save/load code.
//! This module does not open referenced paths or emulate the original import dialog.
use crate::mix::Mix;
use std::{collections::BTreeSet, fmt};

pub const FIRST_FACTORY_ID: u16 = 731;
pub const LAST_FACTORY_ID: u16 = 1879;
pub const FIRST_IMPORT_ID: u16 = 1880;
pub const FIRST_KIT_ID: u16 = 4400;
pub const MAX_STRING_BYTES: usize = 16_537;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError(pub String);
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ImportError {}

/// Raw single-byte text. The original file stores ANSI bytes, not UTF-8.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ByteText(pub Vec<u8>);
impl ByteText {
    /// Convenience for the observed one-byte character range. This is not a code-page conversion.
    pub fn from_text(text: &str) -> Result<Self, ImportError> {
        let bytes = text
            .chars()
            .map(|c| {
                u8::try_from(c as u32).map_err(|_| {
                    ImportError("Text needs an explicit original ANSI code-page conversion".into())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = Self(bytes);
        result.validate()?;
        Ok(result)
    }
    /// Reversible byte-to-Unicode view for inspection, not original locale interpretation.
    pub fn byte_view(&self) -> String {
        self.0.iter().map(|&b| char::from(b)).collect()
    }
    fn validate(&self) -> Result<(), ImportError> {
        if self.0.len() > MAX_STRING_BYTES || self.0.iter().any(|&b| b <= 1) {
            return Err(ImportError(
                "String exceeds the original bound or contains a string delimiter".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitRegion {
    pub first_id: u16,
    pub last_id: u16,
    pub name: ByteText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSample {
    /// Implied by ascending referenced IDs in the saved grid. It is not a record field.
    pub source_id: u16,
    pub labels: [ByteText; 2],
    pub path: ByteText,
    pub category: ByteText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixFooter {
    pub title: ByteText,
    /// Empty for observed section count 1; three regions for the current writer's count 2.
    pub kit_regions: Vec<KitRegion>,
    pub imports: Vec<ImportedSample>,
    pub routes: [u16; 8],
    /// Original loader closes after routes. Retain any unread bytes for exact copies.
    pub trailing: Vec<u8>,
}

impl MixFooter {
    /// Minimal recovered layout accepted by the original reader, with no kit remapping.
    pub fn new(title: ByteText, routes: [u16; 8]) -> Self {
        Self {
            title,
            kit_regions: Vec::new(),
            imports: Vec::new(),
            routes,
            trailing: Vec::new(),
        }
    }
    pub fn parse(mix: &Mix) -> Result<Self, ImportError> {
        let mut input = Cursor {
            bytes: &mix.trailer,
            offset: 0,
        };
        if input.word()? != 0x0a08 {
            return Err(ImportError("Unsupported MIX footer string format".into()));
        }
        let sections = input.word()?;
        if !(1..=2).contains(&sections) {
            return Err(ImportError("Unsupported MIX footer section count".into()));
        }
        let title = input.text()?;
        let mut kit_regions = Vec::new();
        if sections == 2 {
            let count = input.word()?;
            if count != 3 {
                return Err(ImportError("Unsupported kit region table count".into()));
            }
            for _ in 0..count {
                kit_regions.push(KitRegion {
                    first_id: input.word()?,
                    last_id: input.word()?,
                    name: input.text()?,
                });
            }
        }
        validate_regions(&kit_regions)?;
        let expected_ids = referenced_import_ids(mix, first_kit(&kit_regions))?;
        let count = usize::from(input.word()?);
        if count != expected_ids.len() {
            return Err(ImportError(format!(
                "Imported record count {count} differs from {} referenced source IDs",
                expected_ids.len()
            )));
        }
        let mut imports = Vec::with_capacity(count);
        for source_id in expected_ids {
            let payload = usize::from(input.word()?);
            let labels = [input.text()?, input.text()?];
            let path = input.text()?;
            let category = input.text()?;
            let record = ImportedSample {
                source_id,
                labels,
                path,
                category,
            };
            if payload != record_payload(&record)? {
                return Err(ImportError(
                    "Imported record payload length differs from its four strings".into(),
                ));
            }
            if record.path.0.is_empty() {
                return Err(ImportError("Imported sample path is empty".into()));
            }
            imports.push(record);
        }
        if input.word()? != 8 {
            return Err(ImportError("Saved speaker count must be eight".into()));
        }
        let mut routes = [0; 8];
        for route in &mut routes {
            *route = input.word()?;
        }
        validate_routes(&routes)?;
        Ok(Self {
            title,
            kit_regions,
            imports,
            routes,
            trailing: input.bytes[input.offset..].to_vec(),
        })
    }

    /// Serialize verified fields. Preserve trailing bytes unless the caller clears them.
    pub fn to_bytes(&self, mix: &Mix) -> Result<Vec<u8>, ImportError> {
        validate_regions(&self.kit_regions)?;
        validate_routes(&self.routes)?;
        let expected_ids = referenced_import_ids(mix, first_kit(&self.kit_regions))?;
        if self
            .imports
            .iter()
            .map(|sample| sample.source_id)
            .collect::<Vec<_>>()
            != expected_ids
        {
            return Err(ImportError(
                "Import records must match ascending referenced source IDs".into(),
            ));
        }
        let mut output = Vec::new();
        word(&mut output, 0x0a08);
        word(&mut output, if self.kit_regions.is_empty() { 1 } else { 2 });
        text(&mut output, &self.title)?;
        if !self.kit_regions.is_empty() {
            word(&mut output, 3);
            for region in &self.kit_regions {
                word(&mut output, region.first_id);
                word(&mut output, region.last_id);
                text(&mut output, &region.name)?;
            }
        }
        word(
            &mut output,
            u16::try_from(self.imports.len())
                .map_err(|_| ImportError("Too many import records".into()))?,
        );
        for sample in &self.imports {
            if sample.path.0.is_empty() {
                return Err(ImportError("Imported sample path is empty".into()));
            }
            word(
                &mut output,
                u16::try_from(record_payload(sample)?)
                    .map_err(|_| ImportError("Imported record exceeds its length field".into()))?,
            );
            for field in [
                &sample.labels[0],
                &sample.labels[1],
                &sample.path,
                &sample.category,
            ] {
                text(&mut output, field)?;
            }
        }
        word(&mut output, 8);
        for route in self.routes {
            word(&mut output, route);
        }
        output.extend_from_slice(&self.trailing);
        Ok(output)
    }
}

fn first_kit(regions: &[KitRegion]) -> u16 {
    regions.first().map_or(FIRST_KIT_ID, |r| r.first_id)
}
fn validate_regions(regions: &[KitRegion]) -> Result<(), ImportError> {
    if !regions.is_empty() && regions.len() != 3 {
        return Err(ImportError("Kit region table needs three entries".into()));
    }
    let mut previous = LAST_FACTORY_ID;
    for region in regions {
        if region.first_id <= previous
            || region.last_id < region.first_id
            || region.last_id > i16::MAX as u16
        {
            return Err(ImportError("Kit regions are invalid or overlap".into()));
        }
        region.name.validate()?;
        previous = region.last_id;
    }
    Ok(())
}
fn validate_routes(routes: &[u16; 8]) -> Result<(), ImportError> {
    if routes.iter().any(|&route| route > 3) {
        return Err(ImportError("Unsupported saved speaker mode".into()));
    }
    Ok(())
}
fn referenced_import_ids(mix: &Mix, first_kit: u16) -> Result<Vec<u16>, ImportError> {
    if mix
        .events()
        .any(|event| event.sample_id < FIRST_FACTORY_ID || event.sample_id > i16::MAX as u16)
    {
        return Err(ImportError(
            "MIX contains an unsupported reserved or signed sample ID".into(),
        ));
    }
    Ok(mix
        .events()
        .map(|event| event.sample_id)
        .filter(|&id| (FIRST_IMPORT_ID..first_kit).contains(&id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}
fn record_payload(record: &ImportedSample) -> Result<usize, ImportError> {
    let mut size = 8;
    for field in [
        &record.labels[0],
        &record.labels[1],
        &record.path,
        &record.category,
    ] {
        field.validate()?;
        size += field.0.len();
    }
    if size > i16::MAX as usize {
        return Err(ImportError(
            "Imported record exceeds the original signed length".into(),
        ));
    }
    Ok(size)
}
fn word(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn text(output: &mut Vec<u8>, value: &ByteText) -> Result<(), ImportError> {
    value.validate()?;
    word(output, (value.0.len() + 2) as u16);
    output.extend_from_slice(&value.0);
    output.extend_from_slice(&[0, 1]);
    Ok(())
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Cursor<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], ImportError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| ImportError("MIX footer offset overflow".into()))?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ImportError(format!("Incomplete MIX footer at byte {}", self.offset)))?;
        self.offset = end;
        Ok(result)
    }
    fn word(&mut self) -> Result<u16, ImportError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn text(&mut self) -> Result<ByteText, ImportError> {
        let length = usize::from(self.word()?);
        if !(2..=MAX_STRING_BYTES + 2).contains(&length) {
            return Err(ImportError("Unsupported wrapped string length".into()));
        }
        let bytes = self.take(length)?;
        if !bytes.ends_with(&[0, 1]) {
            return Err(ImportError("Unsupported wrapped string delimiters".into()));
        }
        let value = ByteText(bytes[..length - 2].to_vec());
        value.validate()?;
        Ok(value)
    }
}

/// Original registration arithmetic for a positive AWaveDauer result in stereo bytes.
/// The 4,000-byte adjustment tolerates short recording tails near a bar boundary.
pub fn imported_nominal_bars(stereo_bytes: i32) -> Result<u32, ImportError> {
    if stereo_bytes <= 0 {
        return Err(ImportError(
            "Imported sample duration must be positive".into(),
        ));
    }
    Ok((((i64::from(stereo_bytes) - 4000) / 235_200) + 1).clamp(1, 350) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bt(text: &str) -> ByteText {
        ByteText::from_text(text).unwrap()
    }
    fn imported(id: u16, name: &str) -> ImportedSample {
        ImportedSample {
            source_id: id,
            labels: [bt(name), bt("take")],
            path: bt("C:\\audio\\take.wav"),
            category: bt("voice"),
        }
    }
    #[test]
    fn observed_factory_footer_and_unread_suffix_round_trip_exactly() {
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 731, 0).unwrap();
        mix.trailer = b"\x08\x0a\x01\0\x10\0\x52ave \x65Jay 1.01\0\x01\0\0\x08\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\x02\0orphan".to_vec();
        let footer = MixFooter::parse(&mix).unwrap();
        assert!(footer.imports.is_empty());
        assert_eq!(footer.title.byte_view(), "\x52ave \x65Jay 1.01");
        assert_eq!(footer.routes, [0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(footer.trailing, b"orphan");
        assert_eq!(footer.to_bytes(&mix).unwrap(), mix.trailer);
    }
    #[test]
    fn imports_bind_to_unique_referenced_source_ids_in_native_order() {
        let mut mix = Mix::default();
        for (row, id) in [(0, 1901), (1, 1880), (2, 1901)] {
            mix.set_cell(row, 0, id, 0).unwrap();
        }
        let mut footer = MixFooter::new(bt("\x52ave \x65Jay 1.01"), [0; 8]);
        footer.imports = vec![imported(1880, "first"), imported(1901, "second")];
        mix.trailer = footer.to_bytes(&mix).unwrap();
        assert_eq!(MixFooter::parse(&mix).unwrap(), footer);
        footer.imports.swap(0, 1);
        assert!(footer.to_bytes(&mix).is_err());
        footer.imports.clear();
        assert!(footer.to_bytes(&mix).is_err());
    }
    #[test]
    fn current_writer_kit_table_and_record_lengths_are_bounded() {
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 1880, 0).unwrap();
        let mut footer = MixFooter::new(bt("\x52ave \x65Jay 1.01"), [0; 8]);
        footer.kit_regions = [(4400, 4999), (5000, 5624), (5625, 6299)]
            .map(|(first_id, last_id)| KitRegion {
                first_id,
                last_id,
                name: bt(""),
            })
            .to_vec();
        footer.imports.push(imported(1880, "label"));
        mix.trailer = footer.to_bytes(&mix).unwrap();
        assert_eq!(u16::from_le_bytes([mix.trailer[2], mix.trailer[3]]), 2);
        assert_eq!(MixFooter::parse(&mix).unwrap(), footer);
        for length in 0..mix.trailer.len() {
            let mut partial = mix.clone();
            partial.trailer.truncate(length);
            assert!(MixFooter::parse(&partial).is_err());
        }
        footer.kit_regions[1].first_id = 4999;
        assert!(footer.to_bytes(&mix).is_err());
    }
    #[test]
    fn malformed_strings_and_reserved_ids_do_not_enter_the_writer() {
        assert!(ByteText::from_text("x\0y").is_err());
        assert!(ByteText::from_text("λ").is_err());
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 1, 0).unwrap();
        let footer = MixFooter::new(bt("\x52ave \x65Jay 1.01"), [0; 8]);
        assert!(footer.to_bytes(&mix).is_err());
        let mut mix = Mix::default();
        mix.trailer = footer.to_bytes(&mix).unwrap();
        mix.trailer[4] = 255;
        mix.trailer[5] = 255;
        assert!(MixFooter::parse(&mix).is_err());
    }
    #[test]
    fn imported_length_uses_original_tolerance_and_song_bound() {
        assert_eq!(imported_nominal_bars(1).unwrap(), 1);
        assert_eq!(imported_nominal_bars(235200).unwrap(), 1);
        assert_eq!(imported_nominal_bars(239199).unwrap(), 1);
        assert_eq!(imported_nominal_bars(239200).unwrap(), 2);
        assert_eq!(imported_nominal_bars(i32::MAX).unwrap(), 350);
        assert!(imported_nominal_bars(0).is_err());
        assert!(imported_nominal_bars(-1).is_err());
    }
}
