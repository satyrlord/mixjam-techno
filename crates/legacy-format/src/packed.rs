//! Packed container and dictionary expansion recovered from the installed corpus.
//!
//! The original DLL table and decoder were called by an isolated x86 oracle.
//! `decode` is the one decoder. Archive records and stand-alone files differ
//! in two documented ways, which `DecodeOptions` names: where the predictor
//! starts, and what happens when the stream ends before the header's count.
//! Both modes validate every opcode they read.

use std::fmt;

/// The largest packed sample, as an archive record or a stand-alone file, in
/// bytes. Every installed sample is far smaller. The bound stops a damaged
/// catalog or file from making a reader load a huge file.
pub const MAX_PACKED_BYTES: u64 = 64 * 1024 * 1024;

/// The longest header that a packed sample can have: the signature, a length
/// byte, up to 255 bytes of metadata, and the seven-byte audio header. A reader
/// that needs only the header can read this many bytes of a record.
pub const MAX_HEADER_BYTES: usize = 5 + 255 + 7;

/// The largest sample count that a packed header may declare: about 25 minutes
/// of 44.1 kHz audio. The header field holds up to four billion, and every
/// installed sample is far shorter. The bound stops a damaged header from
/// asking for a silent tail of many gigabytes. Decoding allocates only the
/// audio that the stream produces, and the declared tail when it is padded.
pub const MAX_DECODED_SAMPLES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderKind {
    Metadata,
    AudioOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedHeader {
    pub kind: HeaderKind,
    /// The metadata bytes, without the zero bytes that end them.
    pub metadata: Vec<u8>,
    pub decoded_samples: usize,
    /// Raw initial value; the original loader doubles it with 16-bit wrapping.
    pub unknown_field: u16,
    pub audio_offset: usize,
}

impl PackedHeader {
    /// The metadata as text. The Latin-1 mapping keeps each byte as the same
    /// Unicode value.
    pub fn metadata_text(&self) -> String {
        self.metadata.iter().map(|&b| char::from(b)).collect()
    }
    pub fn metadata_fields(&self) -> Vec<String> {
        self.metadata_text()
            .lines()
            .map(|line| line.trim().to_owned())
            .collect()
    }
    /// The predictor value that the original file loader starts from.
    fn header_word(&self) -> i16 {
        (self.unknown_field as i16).wrapping_mul(2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackedError {
    InvalidHeader,
    MissingAudioMarker,
    AllocationLimit { requested: usize },
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
            Self::AllocationLimit { requested } => write!(
                f,
                "Packed sample count exceeds allocation limit: {requested}"
            ),
            Self::TruncatedOpcode { offset } => {
                write!(f, "Incomplete packed opcode at byte {offset}")
            }
            Self::ReservedOpcode { offset, opcode } => {
                write!(f, "Reserved packed opcode {opcode:#04x} at byte {offset}")
            }
            Self::InvalidDictionaryKey { offset, key } => write!(
                f,
                "Invalid packed dictionary key {key:#04x} at byte {offset}"
            ),
            Self::UndefinedDictionaryKey { offset, key } => write!(
                f,
                "Undefined packed dictionary key {key:#04x} at byte {offset}"
            ),
            Self::IncompleteAudio { missing_samples } => write!(
                f,
                "Incomplete packed audio: {missing_samples} samples missing"
            ),
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
        metadata: metadata.to_vec(),
        decoded_samples,
        unknown_field: u16::from_le_bytes(audio_header[5..7].try_into().unwrap()),
        audio_offset: marker + 7,
    })
}

/// Where the predictor starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartState {
    /// An archive record starts at zero. The channel state of the original
    /// decoder supplies the start, and the header word is unused.
    Zero,
    /// The original file loader starts from the doubled header word. A header
    /// with metadata also hands the loader's pending initial sample out as
    /// the first sample, so the audio is one sample longer than the stream.
    /// An audio-only header has no such sample.
    HeaderWord,
}

/// What happens when the stream ends or passes the header's sample count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    /// The audio is exactly the header's count. The last run is cut at the
    /// count, and a stream that ends early is padded with silence (zero
    /// samples, not a held level).
    PadSilence,
    /// The audio ends at the last complete opcode. The last run stays whole,
    /// even past the header's count, and nothing is invented for a stream
    /// that ends early.
    PhysicalEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeOptions {
    pub start: StartState,
    pub tail: Tail,
}

impl DecodeOptions {
    /// An archive record: zero start, padded to the header's count.
    pub const ARCHIVE: Self = Self {
        start: StartState::Zero,
        tail: Tail::PadSilence,
    };
    /// A stand-alone file: header-word start, ends at the last opcode.
    pub const FILE: Self = Self {
        start: StartState::HeaderWord,
        tail: Tail::PhysicalEnd,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub header: PackedHeader,
    /// 16-bit mono audio at the container's own rate.
    pub samples: Vec<i16>,
    /// How many samples the stream itself produced. The loader's initial
    /// sample and the silence pad are not counted.
    pub produced: usize,
}

#[derive(Clone, Copy, Default)]
struct Snippet {
    bytes: [u8; 5],
    len: usize,
}

/// Walk the opcodes of `compressed` until `nominal` samples exist or the input
/// ends, and hand each run of delta codes to `sink`. Every opcode that is read
/// is validated: a reserved opcode, a dictionary key outside `1..=0xf3`, an
/// undefined key and a truncated opcode are all errors. Slot 0 always holds
/// five unchanged codes. The result is the pair of bytes consumed and samples
/// produced.
fn walk(
    compressed: &[u8],
    nominal: usize,
    tail: Tail,
    mut sink: impl FnMut(&[u8]),
) -> Result<(usize, usize), PackedError> {
    let mut dictionary = [Snippet::default(); 256];
    let mut pos = 0;
    let mut produced = 0;
    while pos < compressed.len() && produced < nominal {
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
                });
            }
            key => {
                let entry = dictionary[key as usize];
                if entry.len == 0 {
                    return Err(PackedError::UndefinedDictionaryKey { offset: start, key });
                }
                entry
            }
        };
        let count = match tail {
            Tail::PadSilence => snippet.len.min(nominal - produced),
            Tail::PhysicalEnd => snippet.len,
        };
        sink(&snippet.bytes[..count]);
        produced += count;
    }
    Ok((pos, produced))
}

/// Decode one packed archive record or stand-alone file to 16-bit mono audio.
///
/// The decoder reserves room only for what the input bytes can produce: no
/// opcode yields more than five samples. A declared count that the stream
/// never reaches costs memory only under `Tail::PadSilence`, which pads to it.
pub fn decode(data: &[u8], options: DecodeOptions) -> Result<Decoded, PackedError> {
    let header = parse_header(data)?;
    let compressed = &data[header.audio_offset..];
    let nominal = header.decoded_samples;
    let initial = match options.start {
        StartState::Zero => 0,
        StartState::HeaderWord => header.header_word(),
    };
    let lead = options.start == StartState::HeaderWord && header.kind == HeaderKind::Metadata;
    let reachable = compressed.len().saturating_mul(5).saturating_add(4);
    let mut samples = Vec::with_capacity(usize::from(lead) + nominal.min(reachable));
    if lead {
        samples.push(initial);
    }
    let mut predictor = initial;
    let (_, produced) = walk(compressed, nominal, options.tail, |codes| {
        for &code in codes {
            predictor = predictor.wrapping_add(ORIGINAL_DPCM_STEPS[usize::from(code)]);
            samples.push(predictor);
        }
    })?;
    if options.tail == Tail::PadSilence {
        samples.resize(usize::from(lead) + nominal, 0);
    }
    Ok(Decoded {
        header,
        samples,
        produced,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A record with empty metadata: the signature, length 0, the audio
    /// marker, the declared count and the header word, then the stream.
    fn record(frames: u32, header_word: i16, stream: &[u8]) -> Vec<u8> {
        let mut bytes = b"t\x50xD\0T".to_vec();
        bytes.extend(frames.to_le_bytes());
        bytes.extend(header_word.to_le_bytes());
        bytes.extend(stream);
        bytes
    }
    fn archive(frames: u32, header_word: i16, stream: &[u8]) -> Result<Vec<i16>, PackedError> {
        decode(&record(frames, header_word, stream), DecodeOptions::ARCHIVE)
            .map(|decoded| decoded.samples)
    }
    fn file(frames: u32, header_word: i16, stream: &[u8]) -> Result<Vec<i16>, PackedError> {
        decode(&record(frames, header_word, stream), DecodeOptions::FILE)
            .map(|decoded| decoded.samples)
    }
    /// The delta codes that a stream expands to.
    fn codes(stream: &[u8], nominal: usize, tail: Tail) -> Result<Vec<u8>, PackedError> {
        let mut out = Vec::new();
        walk(stream, nominal, tail, |run| out.extend_from_slice(run))?;
        Ok(out)
    }

    #[test]
    fn parses_latin1_metadata_and_audio_only_header() {
        let bytes = [
            b't', b'P', b'x', b'D', 4, 0xe9, 13, 10, 0, b'T', 3, 0, 0, 0, 0xfe, 0xff, 0x80,
        ];
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.metadata_text(), "é\r\n");
        assert_eq!(header.metadata, [0xe9, 13, 10]);
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
        // The bound is the count that a header may declare, not a byte total.
        let at_bound = record(MAX_DECODED_SAMPLES as u32, 0, &[]);
        assert!(parse_header(&at_bound).is_ok());
        let past_bound = record(MAX_DECODED_SAMPLES as u32 + 1, 0, &[]);
        for options in [DecodeOptions::ARCHIVE, DecodeOptions::FILE] {
            assert!(matches!(
                decode(&past_bound, options),
                Err(PackedError::AllocationLimit { .. })
            ));
        }
    }

    #[test]
    fn original_steps_match_the_table_builder() {
        // The table builder at RVA 0x3095 of packed32r4.dll.
        let mut levels = [0_i16; 257];
        let mut growth = 2.2_f64;
        let mut factor = 1.0565_f64;
        for index in 1..128 {
            let step = if index < 6 {
                1
            } else {
                (growth - 0.52).trunc() as i16
            };
            levels[128 + index] = levels[127 + index] + step;
            growth *= factor;
            factor -= 0.00022;
        }
        for index in 1..=128 {
            levels[128 - index] = -levels[128 + index];
        }
        let built: [i16; 256] = std::array::from_fn(|index| levels[index] * 2);
        assert_eq!(built, ORIGINAL_DPCM_STEPS);
    }

    #[test]
    fn expands_dictionary_and_redefinition_without_recursion() {
        let input = [
            0xf5, 0x10, 0x81, 0x7f, 0x10, 0xf4, 0x10, 0x82, 0x10, 0xff, 0x10,
        ];
        assert_eq!(
            codes(&input, 7, Tail::PadSilence).unwrap(),
            [0x81, 0x7f, 0x81, 0x7f, 0x82, 0x82, 0x10]
        );
    }

    #[test]
    fn every_dictionary_length_and_both_tails() {
        for len in 1..=5 {
            let mut input = vec![0xf3 + len as u8, 42];
            input.extend(1..=len as u8);
            input.push(42);
            let mut expected: Vec<u8> = (1..=len as u8).collect();
            expected.extend(1..=len as u8);
            assert_eq!(codes(&input, len * 2, Tail::PadSilence).unwrap(), expected);
            assert_eq!(codes(&input, len * 2, Tail::PhysicalEnd).unwrap(), expected);
        }
        // Slot 0 is five unchanged codes. The last run is cut at the count
        // under PadSilence and stays whole under PhysicalEnd.
        assert_eq!(codes(&[0], 3, Tail::PadSilence).unwrap(), [0x80; 3]);
        assert_eq!(codes(&[0], 3, Tail::PhysicalEnd).unwrap(), [0x80; 5]);
        // A stream that ends early produces only what it holds.
        assert_eq!(codes(&[0xff, 0x81], 4, Tail::PadSilence).unwrap(), [0x81]);
        assert_eq!(codes(&[0xff, 0x81], 4, Tail::PhysicalEnd).unwrap(), [0x81]);
    }

    #[test]
    fn archive_output_starts_at_zero_and_pads_silence() {
        // One literal, one defined pair, its reuse, and a literal.
        assert_eq!(
            archive(6, 0, &[0xff, 90, 0xf5, 1, 204, 52, 1, 0xff, 128]).unwrap(),
            [-430, 2882, -430, 2882, -430, -430]
        );
        // The header word does not move the start of an archive record.
        assert_eq!(archive(5, 123, &[0]).unwrap(), [0; 5]);
        assert_eq!(archive(1, 0, &[]).unwrap(), [0]);
        assert_eq!(archive(2, 37, &[0xff, 129]).unwrap(), [2, 0]);
        // The pad is zero samples, not the last level held.
        assert_eq!(archive(4, 0, &[0xff, 255]).unwrap(), [25266, 0, 0, 0]);
        // The last run is cut at the count.
        let stream = [0xf8, 1, 0x81, 0x82, 0x80, 0x7f, 0x83, 1];
        assert_eq!(
            archive(8, 0, &stream).unwrap(),
            [2, 6, 6, 4, 10, 12, 16, 16]
        );
        let decoded = decode(&record(8, 0, &stream), DecodeOptions::ARCHIVE).unwrap();
        assert_eq!(decoded.produced, 8);
        let short = decode(&record(4, 0, &[0xff, 255]), DecodeOptions::ARCHIVE).unwrap();
        assert_eq!(short.produced, 1);
        assert_eq!(short.samples.len(), 4);
    }

    #[test]
    fn predictor_wraps_without_saturation() {
        assert_eq!(
            archive(2, 0, &[0xff, 255, 0xff, 255]).unwrap(),
            [25266, -15004]
        );
        assert_eq!(
            file(1, i16::MAX, &[0xff, 255]).unwrap(),
            [-2, 25264],
            "the doubled header word wraps too"
        );
    }

    #[test]
    fn file_output_starts_at_the_header_word_and_ends_at_the_last_opcode() {
        // The header word 37 doubles to 74 and leads the audio.
        assert_eq!(file(4, 37, &[255, 129]).unwrap(), [74, 76]);
        // The last run stays whole, past the count of 8.
        let stream = [0xf8, 1, 0x81, 0x82, 0x80, 0x7f, 0x83, 1];
        assert_eq!(
            file(8, 37, &stream).unwrap(),
            [74, 76, 80, 80, 78, 84, 86, 90, 90, 88, 94]
        );
        let decoded = decode(&record(8, 37, &stream), DecodeOptions::FILE).unwrap();
        assert_eq!(decoded.produced, 10);
        // The same bytes decode differently as an archive record.
        assert_ne!(
            archive(4, 37, &[255, 129]).unwrap(),
            file(4, 37, &[255, 129]).unwrap()
        );
        // A declared count that the stream never reaches allocates nothing.
        let huge = record(MAX_DECODED_SAMPLES as u32, 37, &[255, 129]);
        let decoded = decode(&huge, DecodeOptions::FILE).unwrap();
        assert_eq!(decoded.samples, [74, 76]);
        assert!(decoded.samples.capacity() < 64);
    }

    #[test]
    fn an_audio_only_header_has_no_loader_sample() {
        let with_metadata = record(2, 37, &[255, 129, 255, 128]);
        let audio_only = &with_metadata[5..];
        let decoded = decode(audio_only, DecodeOptions::FILE).unwrap();
        assert_eq!(decoded.header.kind, HeaderKind::AudioOnly);
        assert_eq!(decoded.samples, [76, 76]);
        assert_eq!(
            decode(&with_metadata, DecodeOptions::FILE).unwrap().samples,
            [74, 76, 76]
        );
    }

    #[test]
    fn malformed_streams_are_errors_in_both_modes() {
        let bad: [(&[u8], u32); 10] = [
            (&[0xff], 1),
            (&[0xf8, 1, 2], 5),
            (&[0xf4, 0, 1], 1),
            (&[0xf4, 0xf4, 1], 1),
            (&[0xf5, 0, 1, 2], 2),
            (&[0xf9], 1),
            (&[0xfa, 1, 2, 3, 4, 5, 6], 1),
            (&[0xfe], 1),
            (&[1], 1),
            (&[0xff, 129, 0xff], 100),
        ];
        for (stream, frames) in bad {
            assert!(archive(frames, 0, stream).is_err(), "archive {stream:?}");
            assert!(file(frames, 0, stream).is_err(), "file {stream:?}");
        }
        assert!(matches!(
            archive(1, 0, &[0xf9]),
            Err(PackedError::ReservedOpcode { opcode: 0xf9, .. })
        ));
        assert!(matches!(
            archive(1, 0, &[0xfa]),
            Err(PackedError::ReservedOpcode { opcode: 0xfa, .. })
        ));
        assert!(matches!(
            archive(2, 0, &[0xf5, 0, 1, 2]),
            Err(PackedError::InvalidDictionaryKey { key: 0, .. })
        ));
        assert!(matches!(
            archive(1, 0, &[1]),
            Err(PackedError::UndefinedDictionaryKey { key: 1, .. })
        ));
        assert!(matches!(
            archive(1, 0, &[0xff]),
            Err(PackedError::TruncatedOpcode { .. })
        ));
        // A malformed opcode after the count is never read.
        assert!(archive(1, 0, &[0xff, 128, 0xf9]).is_ok());
        assert!(decode(&[], DecodeOptions::ARCHIVE).is_err());
        assert!(decode(b"t\x50xD", DecodeOptions::FILE).is_err());
    }
}
