//! File-backed sample admission: compressed sources and canonical PCM WAV.

use crate::packed::{self, DecodeOptions, HeaderKind};
use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct PlaybackError(pub String);
impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for PlaybackError {}
impl From<packed::PackedError> for PlaybackError {
    fn from(e: packed::PackedError) -> Self {
        Self(e.to_string())
    }
}

/// The original loader holds a file's byte length in a signed 32-bit value, so
/// a WAV or packed file past 2 GiB is not a native sample. A file below this
/// bound is read whole and its audio is converted in memory; the work is
/// bounded by the file's own size.
pub const NATIVE_FILE_BYTES: u64 = i32::MAX as u64;

/// How many samples a stream may leave out at its end for the original file
/// loader to still accept it. The audio of a short stream is not padded.
const LOADER_TAIL_SLACK: usize = 4;

/// Original AWaveDauer registration result for admitted canonical PCM WAV.
/// Files shorter than the original 270-byte probe return zero. This value is
/// an estimated 44.1 kHz stereo byte length, not the physical file size.
pub fn wave_duration_bytes(bytes: &[u8]) -> Result<i32, PlaybackError> {
    let wave = admit_wave(bytes)?;
    if bytes.len() < 270 {
        return Ok(0);
    }
    let dword = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as i32;
    let mut length = i32::try_from(bytes.len())
        .map_err(|_| PlaybackError("WAV file exceeds the native signed byte range".into()))?;
    let riff = dword(4);
    let data = dword(40);
    if riff > 0 && length < riff.wrapping_add(riff / 7) && riff.wrapping_sub(riff / 7) < length {
        length = riff;
    }
    if data > 0 && length < data.wrapping_mul(3) && data / 6 < length && data < length {
        length = data;
    }
    if wave.channels == 1 {
        length = length.wrapping_shl(1);
    }
    if u16::from_le_bytes(bytes[34..36].try_into().unwrap()) == 8 {
        length = length.wrapping_shl(1);
    }
    length = match wave.sample_rate {
        11025 => length.wrapping_shl(2),
        22050 => length.wrapping_shl(1),
        _ => length,
    };
    if length <= 0 {
        return Err(PlaybackError(
            "WAV duration is outside the positive native byte range".into(),
        ));
    }
    Ok(length)
}

/// Canonical PCM WAV within the native byte range, at an original rate.
fn admit_wave(bytes: &[u8]) -> Result<crate::wav::PcmWave, PlaybackError> {
    if bytes.len() as u64 > NATIVE_FILE_BYTES {
        return Err(PlaybackError(
            "WAV file exceeds the native signed byte range".into(),
        ));
    }
    let wave = crate::wav::PcmWave::parse(bytes).map_err(|e| PlaybackError(e.to_string()))?;
    if !matches!(wave.sample_rate, 11025 | 22050 | crate::NATIVE_SAMPLE_RATE) {
        return Err(PlaybackError(
            "WAV playback supports 11025, 22050, or 44100 Hz".into(),
        ));
    }
    if bytes.get(12..16) != Some(b"fmt ")
        || bytes.get(16..20) != Some(&16u32.to_le_bytes())
        || bytes.get(36..40) != Some(b"data")
    {
        return Err(PlaybackError(
            "Native WAV admission needs the canonical 16-byte format header".into(),
        ));
    }
    Ok(wave)
}

/// A sample path that the original's ANSI path field can hold.
fn admit_path(path: &Path) -> Result<(), PlaybackError> {
    let text = path
        .to_str()
        .ok_or_else(|| PlaybackError("Sample path is not Unicode text".into()))?;
    if text.chars().any(|c| u32::from(c) > 255) || text.chars().count() > 179 || text.contains('\0')
    {
        return Err(PlaybackError(
            "Sample path exceeds the admitted original ANSI path range".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct FileSample {
    pcm: Vec<i16>,
}
impl FileSample {
    pub fn from_bytes(path: PathBuf, bytes: &[u8]) -> Result<Self, PlaybackError> {
        if bytes.starts_with(b"RIFF") {
            return Self::from_wave(path, bytes);
        }
        let header = packed::parse_header(bytes)?;
        if header.kind != HeaderKind::Metadata || bytes[4] >= 128 {
            return Err(PlaybackError(
                "File playback needs a metadata packed with a nonnegative native length byte"
                    .into(),
            ));
        }
        admit_path(&path)?;
        if bytes.len() as u64 > NATIVE_FILE_BYTES {
            return Err(PlaybackError(
                "Sample file exceeds the native signed byte range".into(),
            ));
        }
        // The loader reads the declared count of samples. It accepts a stream
        // that ends a few samples early and keeps what the stream holds.
        let decoded = packed::decode(bytes, DecodeOptions::FILE)?;
        let missing = header.decoded_samples.saturating_sub(decoded.produced);
        if missing > LOADER_TAIL_SLACK {
            return Err(packed::PackedError::IncompleteAudio {
                missing_samples: missing,
            }
            .into());
        }
        Ok(Self {
            pcm: decoded.samples,
        })
    }
    /// Original canonical PCM WAV path: first source lane, half gain, and
    /// nearest sample hold at the admitted 11.025/22.05/44.1 kHz rates.
    pub fn from_wave(path: PathBuf, bytes: &[u8]) -> Result<Self, PlaybackError> {
        let wave = admit_wave(bytes)?;
        admit_path(&path)?;
        let physical = i64::try_from(bytes.len())
            .map_err(|_| PlaybackError("WAV file exceeds the native signed byte range".into()))?;
        let riff = i64::from(u32::from_le_bytes(bytes[4..8].try_into().unwrap()));
        let data = i64::from(u32::from_le_bytes(bytes[40..44].try_into().unwrap()));
        let mut effective = physical;
        if riff + riff / 9 > effective && riff > effective / 4 && riff - riff / 9 < effective {
            effective = riff;
        }
        if data * 3 > effective && data > 1000 && data + 9000 > effective && data < effective {
            effective = data + 44;
        }
        if effective < 44 || effective > physical {
            return Err(PlaybackError(
                "Native WAV length heuristic exceeds the source file".into(),
            ));
        }
        let bits = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
        let align = usize::from(wave.channels * (bits / 8));
        let input = &bytes[44..effective as usize];
        if !input.len().is_multiple_of(align) {
            return Err(PlaybackError(
                "Native WAV length leaves an incomplete source frame".into(),
            ));
        }
        let repeat = (crate::NATIVE_SAMPLE_RATE / wave.sample_rate) as usize;
        let count = input.len() / align;
        let total = count
            .checked_mul(repeat)
            .ok_or_else(|| PlaybackError("The converted WAV has too many frames.".into()))?;
        if total == 0 {
            return Err(PlaybackError(
                "WAV source has no native playable frames".into(),
            ));
        }
        let mut pcm = Vec::with_capacity(total);
        for frame in input.chunks_exact(align) {
            let sample = if bits == 8 {
                (i16::from(frame[0]) - 128) * 128
            } else {
                i16::from_le_bytes(frame[..2].try_into().unwrap()) >> 1
            };
            pcm.extend(std::iter::repeat_n(sample, repeat));
        }
        Ok(Self { pcm })
    }
    /// Expose the recovered file-loader output for the shared editor adapter.
    pub fn samples(&self) -> &[i16] {
        &self.pcm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_admission_rejects_unknown_headers_empty_audio_and_non_pcm() {
        let empty = crate::wav::PcmWave {
            sample_rate: 44100,
            channels: 1,
            samples: Vec::new(),
        }
        .encode()
        .unwrap();
        assert!(FileSample::from_wave("empty.wav".into(), &empty).is_err());
        let mut pcm = crate::wav::PcmWave {
            sample_rate: 44100,
            channels: 1,
            samples: vec![1; 1024],
        }
        .encode()
        .unwrap();
        pcm[20..22].copy_from_slice(&3u16.to_le_bytes());
        assert!(FileSample::from_wave("float.wav".into(), &pcm).is_err());
        pcm[20..22].copy_from_slice(&1u16.to_le_bytes());
        pcm.splice(12..12, b"JUNK\x04\0\0\0test".iter().copied());
        let length = pcm.len() as u32 - 8;
        pcm[4..8].copy_from_slice(&length.to_le_bytes());
        assert!(crate::wav::PcmWave::parse(&pcm).is_ok());
        assert!(FileSample::from_wave("metadata.wav".into(), &pcm).is_err());
    }

    fn packed(count: u32, stream: &[u8]) -> Vec<u8> {
        let mut bytes = b"t\x50xD\0T".to_vec();
        bytes.extend(count.to_le_bytes());
        bytes.extend(37u16.to_le_bytes());
        bytes.extend(stream);
        bytes
    }

    #[test]
    fn a_packed_file_starts_at_the_header_word_and_keeps_the_whole_last_run() {
        let stream = [0xf8, 1, 0x81, 0x82, 0x80, 0x7f, 0x83, 1];
        let sample = FileSample::from_bytes("a.pxd".into(), &packed(8, &stream)).unwrap();
        assert_eq!(
            sample.samples(),
            [74, 76, 80, 80, 78, 84, 86, 90, 90, 88, 94]
        );
    }

    #[test]
    fn a_packed_file_may_end_four_samples_early_but_not_five() {
        // One literal makes one sample. A count of five leaves four missing.
        let sample = FileSample::from_bytes("a.pxd".into(), &packed(5, &[255, 129])).unwrap();
        assert_eq!(sample.samples(), [74, 76]);
        assert!(matches!(
            FileSample::from_bytes("a.pxd".into(), &packed(6, &[255, 129])),
            Err(PlaybackError(text)) if text == "Incomplete packed audio: 5 samples missing"
        ));
        // A huge count over a tiny stream fails without a large allocation.
        assert!(FileSample::from_bytes(
            "a.pxd".into(),
            &packed(packed::MAX_DECODED_SAMPLES as u32, &[255, 129])
        )
        .is_err());
    }

    #[test]
    fn a_packed_file_rejects_malformed_opcodes_and_audio_only_headers() {
        for stream in [&[0xf9u8][..], &[0xff], &[1], &[0xf5, 0, 1, 2]] {
            assert!(FileSample::from_bytes("a.pxd".into(), &packed(2, stream)).is_err());
        }
        let metadata = packed(2, &[255, 129, 255, 128]);
        assert!(FileSample::from_bytes("a.pxd".into(), &metadata[5..]).is_err());
    }

    #[test]
    fn a_wav_past_128_mib_is_a_native_sample() {
        let data_bytes = 129 * 1024 * 1024u32;
        let mut wav = crate::wav::PcmWave {
            sample_rate: 44100,
            channels: 1,
            samples: Vec::new(),
        }
        .encode()
        .unwrap();
        wav[4..8].copy_from_slice(&(data_bytes + 36).to_le_bytes());
        wav[40..44].copy_from_slice(&data_bytes.to_le_bytes());
        wav.resize(44 + data_bytes as usize, 0);
        let sample = FileSample::from_wave("long.wav".into(), &wav).unwrap();
        assert_eq!(sample.samples().len(), data_bytes as usize / 2);
        assert!(wave_duration_bytes(&wav).unwrap() > 0);
    }
}
