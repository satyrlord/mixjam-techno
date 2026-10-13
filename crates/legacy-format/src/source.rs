//! Predecoded packed audio with original dictionary-run boundaries retained.
//! The native decoder finishes a whole run even when it crosses a chunk limit.
//! Declared-length WAV decoding remains separate in packed::original_pcm.
use crate::packed::{self, PackedError};

#[derive(Debug, Clone)]
pub struct SampleSource {
    initial_sample: i16,
    file_initial_applied: bool,
    pub(crate) pcm: Vec<i16>,
    /// Exclusive output positions after each original compressed opcode.
    pub(crate) run_ends: Vec<usize>,
    /// Compressed bytes consumed at the corresponding run boundary.
    pub(crate) compressed_ends: Vec<usize>,
}
impl SampleSource {
    pub fn samples(&self) -> &[i16] {
        &self.pcm
    }
    /// Original full-header file loader sets a pending first sample before
    /// calling the decoder (54AB/583E -> 51CB -> 67A8). The isolated codec
    /// oracle starts with that flag clear, so the conversion is explicit.
    pub fn with_file_loader_initial_sample(mut self) -> Self {
        if self.file_initial_applied {
            return self;
        }
        self.pcm.insert(0, self.initial_sample);
        for end in &mut self.run_ends {
            *end += 1;
        }
        self.run_ends.insert(0, 1);
        self.compressed_ends.insert(0, 0);
        self.file_initial_applied = true;
        self
    }
    pub fn from_packed(bytes: &[u8]) -> Result<Self, PackedError> {
        let decoded = packed::decode_file(bytes)?;
        let actual = decoded.codes.len() + decoded.clipped_samples - decoded.padded_samples;
        let compressed = &bytes
            [decoded.header.audio_offset..decoded.header.audio_offset + decoded.consumed_bytes];
        let codes = if decoded.clipped_samples > 0 {
            packed::expand(compressed, actual)?.codes
        } else {
            decoded.codes[..actual].to_vec()
        };
        let mut accumulator = (decoded.header.unknown_field as i16).wrapping_mul(2);
        let pcm = codes
            .iter()
            .map(|&code| {
                accumulator =
                    accumulator.wrapping_add(packed::ORIGINAL_DPCM_STEPS[usize::from(code)]);
                accumulator
            })
            .collect();
        let mut lengths = [0usize; 256];
        lengths[0] = 5;
        let mut position = 0;
        let mut count = 0;
        let mut run_ends = Vec::new();
        let mut compressed_ends = Vec::new();
        while position < compressed.len() && count < actual {
            let opcode = compressed[position];
            position += 1;
            let length = match opcode {
                0xf4..=0xf8 => {
                    let length = usize::from(opcode - 0xf3);
                    let key = compressed[position];
                    lengths[usize::from(key)] = length;
                    position += length + 1;
                    length
                }
                0xff => {
                    position += 1;
                    1
                }
                key => lengths[usize::from(key)],
            };
            // decode_file has already checked every consumed opcode and key.
            count += length;
            run_ends.push(count);
            compressed_ends.push(position);
        }
        debug_assert_eq!(count, actual);
        Ok(Self {
            initial_sample: (decoded.header.unknown_field as i16).wrapping_mul(2),
            file_initial_applied: false,
            pcm,
            run_ends,
            compressed_ends,
        })
    }
    /// Decode the physical audio contribution for the shared library adapter.
    /// A complete opcode at EOF can precede the declared duration. This path
    /// keeps complete final runs and does not invent an unwritten tail. The
    /// strict container decoder and original FileSample admission stay separate.
    /// Metadata headers receive the recovered pending initial sample. Audio-only
    /// headers use isolated-codec initial state as a shared-adapter extension;
    /// the original main sequencer rejects that header variant.
    pub fn from_packed_file_audio(bytes: &[u8]) -> Result<Self, PackedError> {
        let header = packed::parse_header(bytes)?;
        let compressed = &bytes[header.audio_offset..];
        let mut lengths = [0usize; 256];
        lengths[0] = 5;
        let mut position = 0;
        let mut actual = 0usize;
        let mut run_ends = Vec::new();
        let mut compressed_ends = Vec::new();
        while position < compressed.len() && actual < header.decoded_samples {
            let start = position;
            let opcode = compressed[position];
            position += 1;
            let length = match opcode {
                0xf4..=0xf8 => {
                    let length = usize::from(opcode - 0xf3);
                    let body = compressed
                        .get(position..position + length + 1)
                        .ok_or(PackedError::TruncatedOpcode { offset: start })?;
                    let key = body[0];
                    if !(1..=0xf3).contains(&key) {
                        return Err(PackedError::InvalidDictionaryKey {
                            offset: position,
                            key,
                        });
                    }
                    lengths[usize::from(key)] = length;
                    position += length + 1;
                    length
                }
                0xff => {
                    if position == compressed.len() {
                        return Err(PackedError::TruncatedOpcode { offset: start });
                    }
                    position += 1;
                    1
                }
                0xf9..=0xfe => {
                    return Err(PackedError::ReservedOpcode {
                        offset: start,
                        opcode,
                    });
                }
                key => {
                    let length = lengths[usize::from(key)];
                    if length == 0 {
                        return Err(PackedError::UndefinedDictionaryKey { offset: start, key });
                    }
                    length
                }
            };
            actual += length;
            if actual > packed::MAX_DECODED_SAMPLES {
                return Err(PackedError::AllocationLimit { requested: actual });
            }
            run_ends.push(actual);
            compressed_ends.push(position);
        }
        // Expand only the validated physical contribution. A nominal header
        // cannot cause allocation or padding for samples absent from the file.
        let codes = packed::expand(&compressed[..position], actual)?.codes;
        let initial_sample = (header.unknown_field as i16).wrapping_mul(2);
        let mut accumulator = initial_sample;
        let pcm = codes
            .iter()
            .map(|&code| {
                accumulator =
                    accumulator.wrapping_add(packed::ORIGINAL_DPCM_STEPS[usize::from(code)]);
                accumulator
            })
            .collect();
        let source = Self {
            initial_sample,
            file_initial_applied: false,
            pcm,
            run_ends,
            compressed_ends,
        };
        Ok(if header.kind == packed::HeaderKind::Metadata {
            source.with_file_loader_initial_sample()
        } else {
            source
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(count: u32, compressed: &[u8]) -> Vec<u8> {
        let mut b = b"t\x50xD\0T".to_vec();
        b.extend(count.to_le_bytes());
        b.extend(37u16.to_le_bytes());
        b.extend(compressed);
        b
    }
    #[test]
    fn native_run_boundaries_survive_chunk_limits_and_declared_end() {
        let s = SampleSource::from_packed(&fixture(8, &[0xf8, 1, 0x81, 0x82, 0x80, 0x7f, 0x83, 1]))
            .unwrap();
        assert_eq!(s.pcm.len(), 10);
        assert_eq!(s.run_ends, [5, 10]);
        assert_eq!(s.compressed_ends, [7, 8]);
        assert_eq!(
            s.pcm[..8],
            packed::original_pcm(
                &packed::decode_file(&fixture(8, &[0xf8, 1, 0x81, 0x82, 0x80, 0x7f, 0x83, 1]))
                    .unwrap()
            )
        );
    }
    #[test]
    fn short_stream_omits_unwritten_silence_from_live_contribution() {
        let b = fixture(4, &[0xff, 0x81]);
        let s = SampleSource::from_packed(&b).unwrap();
        assert_eq!(s.pcm.len(), 1);
        assert_eq!(s.pcm[0], 76);
        assert_eq!(
            packed::original_pcm(&packed::decode_file(&b).unwrap()),
            [76, 0, 0, 0]
        );
    }
    #[test]
    fn file_loader_pending_initial_is_a_separate_zero_input_run() {
        let s = SampleSource::from_packed(&fixture(4, &[255, 129]))
            .unwrap()
            .with_file_loader_initial_sample();
        assert_eq!(s.pcm, [74, 76]);
        assert_eq!(s.run_ends, [1, 2]);
        assert_eq!(s.compressed_ends, [0, 2]);
    }
    #[test]
    fn file_audio_stops_at_complete_physical_eof_without_nominal_padding() {
        let b = fixture(packed::MAX_DECODED_SAMPLES as u32, &[255, 129]);
        let s = SampleSource::from_packed_file_audio(&b).unwrap();
        assert_eq!(s.pcm, [74, 76]);
        assert_eq!(s.run_ends, [1, 2]);
        assert_eq!(s.compressed_ends, [0, 2]);
        assert!(matches!(
            packed::decode_file(&b),
            Err(PackedError::ImpossibleExpansion { .. })
        ));
        assert!(matches!(
            SampleSource::from_packed_file_audio(&fixture(
                packed::MAX_DECODED_SAMPLES as u32 + 1,
                &[255, 129]
            )),
            Err(PackedError::AllocationLimit { .. })
        ));
    }
    #[test]
    fn file_audio_retains_the_final_whole_run_and_metadata_initial_once() {
        let b = fixture(8, &[0xf8, 1, 0x81, 0x82, 0x80, 0x7f, 0x83, 1]);
        let s = SampleSource::from_packed_file_audio(&b).unwrap();
        assert_eq!(s.pcm.len(), 11);
        assert_eq!(s.run_ends, [1, 6, 11]);
        assert_eq!(s.compressed_ends, [0, 7, 8]);
        assert_eq!(s.clone().with_file_loader_initial_sample().pcm, s.pcm);
        assert_eq!(
            s.pcm,
            SampleSource::from_packed(&b)
                .unwrap()
                .with_file_loader_initial_sample()
                .pcm
        );
    }
    #[test]
    fn file_audio_audio_only_keeps_isolated_codec_initial_state() {
        let metadata = fixture(2, &[255, 129, 255, 128]);
        let s = SampleSource::from_packed_file_audio(&metadata[5..]).unwrap();
        assert_eq!(s.pcm, [76, 76]);
        assert_eq!(s.run_ends, [1, 2]);
        assert_eq!(s.compressed_ends, [2, 4]);
        assert_eq!(
            s.pcm,
            packed::original_pcm(&packed::decode_file(&metadata[5..]).unwrap())
        );
        assert!(crate::playback::FileSample::from_bytes(
            "audio-only.\x70xd".into(),
            &metadata[5..]
        )
        .is_err());
    }
    #[test]
    fn file_audio_rejects_malformed_opcodes_before_physical_eof() {
        for compressed in [
            &[255][..],
            &[248, 1, 2],
            &[244, 0, 128],
            &[249],
            &[1],
            &[255, 129, 255],
        ] {
            assert!(
                SampleSource::from_packed_file_audio(&fixture(100_000, compressed)).is_err(),
                "{compressed:?}"
            );
        }
        assert!(SampleSource::from_packed_file_audio(b"t\x50xD\0T").is_err());
    }
}
