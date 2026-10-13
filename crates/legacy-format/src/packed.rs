//! packed container and dictionary expansion recovered from the installed corpus.
//!
//! The original DLL table and decoder were called by an isolated x86 oracle.
//! `original_pcm` matches that decoder with an empty destination mix buffer.

use std::fmt;

pub const MAX_DECODED_SAMPLES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderKind {
    Metadata,
    AudioOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedHeader {
    pub kind: HeaderKind,
    /// Latin-1 mapping preserves each metadata byte as the same Unicode value.
    pub metadata_text: String,
    pub decoded_samples: usize,
    /// Raw initial value; the original loader doubles it with 16-bit wrapping.
    pub unknown_field: u16,
    pub audio_offset: usize,
}

impl PackedHeader {
    pub fn metadata_fields(&self) -> Vec<&str> {
        self.metadata_text.lines().map(str::trim).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedCodes {
    pub codes: Vec<u8>,
    pub consumed_bytes: usize,
    pub padded_samples: usize,
    pub clipped_samples: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedPacked {
    pub header: PackedHeader,
    pub codes: Vec<u8>,
    pub consumed_bytes: usize,
    pub trailing_bytes: usize,
    pub padded_samples: usize,
    pub clipped_samples: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackedError {
    InvalidHeader,
    MissingAudioMarker,
    AllocationLimit { requested: usize },
    ImpossibleExpansion { requested: usize, compressed: usize },
    TruncatedOpcode { offset: usize },
    ReservedOpcode { offset: usize, opcode: u8 },
    InvalidDictionaryKey { offset: usize, key: u8 },
    UndefinedDictionaryKey { offset: usize, key: u8 },
    IncompleteAudio { missing_samples: usize },
}

impl fmt::Display for PackedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "Invalid packed header"),
            Self::MissingAudioMarker => write!(f, "Missing packed audio marker"),
            Self::AllocationLimit { requested } => write!(f, "packed sample count exceeds allocation limit: {requested}"),
            Self::ImpossibleExpansion { requested, compressed } => write!(f, "packed sample count {requested} exceeds expansion bound for {compressed} compressed bytes"),
            Self::TruncatedOpcode { offset } => write!(f, "Incomplete packed opcode at byte {offset}"),
            Self::ReservedOpcode { offset, opcode } => write!(f, "Reserved packed opcode {opcode:#04x} at byte {offset}"),
            Self::InvalidDictionaryKey { offset, key } => write!(f, "Invalid packed dictionary key {key:#04x} at byte {offset}"),
            Self::UndefinedDictionaryKey { offset, key } => write!(f, "Undefined packed dictionary key {key:#04x} at byte {offset}"),
            Self::IncompleteAudio { missing_samples } => write!(f, "Incomplete packed audio: {missing_samples} samples missing"),
        }
    }
}

impl std::error::Error for PackedError {}

pub fn parse_header(data: &[u8]) -> Result<PackedHeader, PackedError> {
    let (kind, marker, metadata) = if data.starts_with(b"t\x50xD") {
        let length = *data.get(4).ok_or(PackedError::InvalidHeader)? as usize;
        let marker = 5 + length;
        let raw = data.get(5..marker).ok_or(PackedError::InvalidHeader)?;
        let trimmed = raw.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        (HeaderKind::Metadata, marker, &raw[..trimmed])
    } else if data.first() == Some(&b'T') {
        // One installed file has this seven-byte audio-only header.
        (HeaderKind::AudioOnly, 0, &[][..])
    } else {
        return Err(PackedError::InvalidHeader);
    };
    let audio_header = data
        .get(marker..marker + 7)
        .ok_or(PackedError::InvalidHeader)?;
    if audio_header[0] != b'T' {
        return Err(PackedError::MissingAudioMarker);
    }
    let decoded_samples = u32::from_le_bytes(audio_header[1..5].try_into().unwrap()) as usize;
    if decoded_samples > MAX_DECODED_SAMPLES {
        return Err(PackedError::AllocationLimit {
            requested: decoded_samples,
        });
    }
    Ok(PackedHeader {
        kind,
        metadata_text: metadata.iter().map(|&b| char::from(b)).collect(),
        decoded_samples,
        unknown_field: u16::from_le_bytes(audio_header[5..7].try_into().unwrap()),
        audio_offset: marker + 7,
    })
}

#[derive(Clone, Copy, Default)]
struct Snippet {
    bytes: [u8; 5],
    len: usize,
}

/// Expand codes, allowing a diagnosed tail pad of at most four neutral codes.
/// The expansion is bounded before allocating. Malformed opcodes are errors.
pub fn expand(compressed: &[u8], decoded_samples: usize) -> Result<ExpandedCodes, PackedError> {
    if decoded_samples > MAX_DECODED_SAMPLES {
        return Err(PackedError::AllocationLimit {
            requested: decoded_samples,
        });
    }
    if decoded_samples > compressed.len().saturating_mul(5).saturating_add(4) {
        return Err(PackedError::ImpossibleExpansion {
            requested: decoded_samples,
            compressed: compressed.len(),
        });
    }
    let mut dictionary = [Snippet::default(); 256];
    let mut codes = Vec::with_capacity(decoded_samples);
    let mut pos = 0;
    let mut clipped_samples = 0;
    while pos < compressed.len() && codes.len() < decoded_samples {
        let start = pos;
        let opcode = compressed[pos];
        pos += 1;
        let snippet = match opcode {
            0xf4..=0xf8 => {
                let len = (opcode - 0xf3) as usize;
                let body = compressed
                    .get(pos..pos + len + 1)
                    .ok_or(PackedError::TruncatedOpcode { offset: start })?;
                let key = body[0];
                if !(1..=0xf3).contains(&key) {
                    return Err(PackedError::InvalidDictionaryKey { offset: pos, key });
                }
                let mut snippet = Snippet {
                    len,
                    ..Snippet::default()
                };
                snippet.bytes[..len].copy_from_slice(&body[1..]);
                dictionary[key as usize] = snippet;
                pos += len + 1;
                snippet
            }
            0xff => {
                let literal = *compressed
                    .get(pos)
                    .ok_or(PackedError::TruncatedOpcode { offset: start })?;
                pos += 1;
                Snippet {
                    bytes: [literal, 0, 0, 0, 0],
                    len: 1,
                }
            }
            0 => Snippet {
                bytes: [0x80; 5],
                len: 5,
            },
            0xf9..=0xfe => {
                return Err(PackedError::ReservedOpcode {
                    offset: start,
                    opcode,
                })
            }
            key => {
                let entry = dictionary[key as usize];
                if entry.len > 0 {
                    entry
                } else {
                    return Err(PackedError::UndefinedDictionaryKey { offset: start, key });
                }
            }
        };
        let count = snippet.len.min(decoded_samples - codes.len());
        codes.extend_from_slice(&snippet.bytes[..count]);
        clipped_samples += snippet.len - count;
    }
    let padded_samples = decoded_samples - codes.len();
    if padded_samples > 4 {
        return Err(PackedError::IncompleteAudio {
            missing_samples: padded_samples,
        });
    }
    codes.resize(decoded_samples, 0x80);
    Ok(ExpandedCodes {
        codes,
        consumed_bytes: pos,
        padded_samples,
        clipped_samples,
    })
}

pub fn decode_file(data: &[u8]) -> Result<DecodedPacked, PackedError> {
    let header = parse_header(data)?;
    let compressed = &data[header.audio_offset..];
    let expanded = expand(compressed, header.decoded_samples)?;
    Ok(DecodedPacked {
        header,
        codes: expanded.codes,
        consumed_bytes: expanded.consumed_bytes,
        trailing_bytes: compressed.len() - expanded.consumed_bytes,
        padded_samples: expanded.padded_samples,
        clipped_samples: expanded.clipped_samples,
    })
}

/// Original signed delta table, captured from the DLL initializer on 2026-10-10.
pub const ORIGINAL_DPCM_STEPS: [i16; 256] = [
    0, -25266, -24412, -23582, -22776, -21992, -21232, -20494, -19778, -19082, -18406, -17752,
    -17116, -16500, -15904, -15326, -14766, -14222, -13696, -13186, -12692, -12214, -11752, -11304,
    -10872, -10454, -10050, -9660, -9282, -8916, -8564, -8224, -7896, -7578, -7272, -6976, -6690,
    -6414, -6148, -5892, -5646, -5408, -5178, -4958, -4746, -4542, -4346, -4156, -3974, -3798,
    -3630, -3468, -3312, -3162, -3018, -2880, -2748, -2620, -2498, -2380, -2268, -2160, -2056,
    -1956, -1860, -1768, -1680, -1596, -1516, -1440, -1366, -1296, -1228, -1164, -1102, -1044,
    -988, -934, -882, -834, -788, -744, -702, -662, -624, -588, -554, -520, -488, -458, -430, -402,
    -376, -352, -328, -306, -286, -266, -248, -230, -214, -198, -182, -168, -154, -142, -130, -118,
    -108, -98, -88, -80, -72, -64, -56, -50, -44, -38, -32, -26, -22, -18, -14, -10, -8, -6, -4,
    -2, 0, 2, 4, 6, 8, 10, 14, 18, 22, 26, 32, 38, 44, 50, 56, 64, 72, 80, 88, 98, 108, 118, 130,
    142, 154, 168, 182, 198, 214, 230, 248, 266, 286, 306, 328, 352, 376, 402, 430, 458, 488, 520,
    554, 588, 624, 662, 702, 744, 788, 834, 882, 934, 988, 1044, 1102, 1164, 1228, 1296, 1366,
    1440, 1516, 1596, 1680, 1768, 1860, 1956, 2056, 2160, 2268, 2380, 2498, 2620, 2748, 2880, 3018,
    3162, 3312, 3468, 3630, 3798, 3974, 4156, 4346, 4542, 4746, 4958, 5178, 5408, 5646, 5892, 6148,
    6414, 6690, 6976, 7272, 7578, 7896, 8224, 8564, 8916, 9282, 9660, 10050, 10454, 10872, 11304,
    11752, 12214, 12692, 13186, 13696, 14222, 14766, 15326, 15904, 16500, 17116, 17752, 18406,
    19082, 19778, 20494, 21232, 21992, 22776, 23582, 24412, 25266,
];

/// Recover the original mono channel contribution into an empty mix buffer.
/// The original accumulator wraps at 16 bits. An unwritten short tail stays
/// zero in the destination, rather than repeating the final accumulator.
pub fn original_pcm(decoded: &DecodedPacked) -> Vec<i16> {
    let mut accumulator = (decoded.header.unknown_field as i16).wrapping_mul(2);
    let written_samples = decoded.codes.len().saturating_sub(decoded.padded_samples);
    decoded
        .codes
        .iter()
        .enumerate()
        .map(|(index, &code)| {
            if index < written_samples {
                accumulator = accumulator.wrapping_add(ORIGINAL_DPCM_STEPS[code as usize]);
                accumulator
            } else {
                0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_latin1_metadata_and_audio_only_header() {
        let bytes = [
            b't', b'P', b'x', b'D', 4, 0xe9, 13, 10, 0, b'T', 3, 0, 0, 0, 0xfe, 0xff, 0x80,
        ];
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.metadata_text, "é\r\n");
        assert_eq!(header.unknown_field, 65534);
        assert_eq!(header.audio_offset, 16);
        assert_eq!(
            parse_header(&[84, 0, 0, 0, 0, 0, 0]).unwrap().kind,
            HeaderKind::AudioOnly
        );
    }

    #[test]
    fn validates_header_before_read_or_allocation() {
        for length in 0..12 {
            assert!(parse_header(&b"t\x50xD\0T\x01\0\0\0\0\0"[..length]).is_err());
        }
        assert_eq!(
            parse_header(b"RIFFxxxxxxxx"),
            Err(PackedError::InvalidHeader)
        );
        assert!(matches!(
            parse_header(&[84, 255, 255, 255, 255, 0, 0]),
            Err(PackedError::AllocationLimit { .. })
        ));
    }

    #[test]
    fn expands_dictionary_and_redefinition_without_recursion() {
        let input = [
            0xf5, 0x10, 0x81, 0x7f, 0x10, 0xf4, 0x10, 0x82, 0x10, 0xff, 0x10,
        ];
        assert_eq!(
            expand(&input, 7).unwrap().codes,
            [0x81, 0x7f, 0x81, 0x7f, 0x82, 0x82, 0x10]
        );
    }

    #[test]
    fn every_dictionary_length_and_tail_diagnostic() {
        for len in 1..=5 {
            let mut input = vec![0xf3 + len as u8, 42];
            input.extend(1..=len as u8);
            input.push(42);
            let mut expected: Vec<u8> = (1..=len as u8).collect();
            expected.extend(1..=len as u8);
            assert_eq!(expand(&input, len * 2).unwrap().codes, expected);
        }
        let padded = expand(&[0xff, 0x81], 4).unwrap();
        assert_eq!(padded.codes, [0x81, 0x80, 0x80, 0x80]);
        assert_eq!(padded.padded_samples, 3);
        let clipped = expand(&[0], 3).unwrap();
        assert_eq!(clipped.codes, [0x80; 3]);
        assert_eq!(clipped.clipped_samples, 2);
    }

    #[test]
    fn malformed_streams_are_errors() {
        assert!(matches!(
            expand(&[0xff], 1),
            Err(PackedError::TruncatedOpcode { .. })
        ));
        assert!(matches!(
            expand(&[0xf8, 1, 2], 5),
            Err(PackedError::TruncatedOpcode { .. })
        ));
        assert!(matches!(
            expand(&[0xf4, 0, 1], 1),
            Err(PackedError::InvalidDictionaryKey { .. })
        ));
        assert!(matches!(
            expand(&[0xf9], 1),
            Err(PackedError::ReservedOpcode { .. })
        ));
        assert!(matches!(
            expand(&[1], 1),
            Err(PackedError::UndefinedDictionaryKey { .. })
        ));
        assert!(matches!(
            expand(&[0xff, 1], 6),
            Err(PackedError::IncompleteAudio { .. })
        ));
        assert!(matches!(
            expand(&[], 5),
            Err(PackedError::ImpossibleExpansion { .. })
        ));
        assert!(matches!(
            expand(&[], MAX_DECODED_SAMPLES + 1),
            Err(PackedError::AllocationLimit { .. })
        ));
    }
}
