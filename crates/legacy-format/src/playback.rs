//! File-backed sample admission: compressed sources and canonical PCM WAV.

use crate::{
    packed::{self, HeaderKind},
    source::SampleSource,
};
use std::{
    fmt, fs,
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

/// Original AWaveDauer registration result for admitted canonical PCM WAV.
/// Files shorter than the original 270-byte probe return zero. This value is
/// an estimated 44.1 kHz stereo byte length, not the physical file size.
pub fn wave_duration_bytes(bytes: &[u8]) -> Result<i32, PlaybackError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(PlaybackError(
            "WAV source exceeds the 128 MiB playback bound".into(),
        ));
    }
    let wave = crate::wav::PcmWave::parse(bytes).map_err(|e| PlaybackError(e.to_string()))?;
    if !matches!(wave.sample_rate, 11025 | 22050 | 44100) {
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

#[derive(Debug)]
pub struct FileSample {
    path: PathBuf,
    pcm: Vec<i16>,
}
impl FileSample {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PlaybackError> {
        let path = path.as_ref();
        let bytes = fs::read(path)
            .map_err(|e| PlaybackError(format!("Cannot read sample {}: {e}", path.display())))?;
        Self::from_bytes(path.to_path_buf(), &bytes)
    }
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
        let text = path
            .to_str()
            .ok_or_else(|| PlaybackError("Sample path is not Unicode text".into()))?;
        if text.chars().any(|c| u32::from(c) > 255)
            || text.chars().count() > 179
            || text.contains('\0')
        {
            return Err(PlaybackError(
                "Sample path exceeds the admitted original ANSI path range".into(),
            ));
        }
        if bytes.len() > i32::MAX as usize {
            return Err(PlaybackError(
                "Sample file exceeds the native signed byte range".into(),
            ));
        }
        let source = SampleSource::from_packed(bytes)?.with_file_loader_initial_sample();
        Ok(Self {
            path,
            pcm: source.pcm,
        })
    }
    /// Original canonical PCM WAV path: first source lane, half gain, and
    /// nearest sample hold at the admitted 11.025/22.05/44.1 kHz rates.
    pub fn from_wave(path: PathBuf, bytes: &[u8]) -> Result<Self, PlaybackError> {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err(PlaybackError(
                "WAV source exceeds the 128 MiB playback bound".into(),
            ));
        }
        let wave = crate::wav::PcmWave::parse(bytes).map_err(|e| PlaybackError(e.to_string()))?;
        if !matches!(wave.sample_rate, 11025 | 22050 | 44100) {
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
        let text = path
            .to_str()
            .ok_or_else(|| PlaybackError("Sample path is not Unicode text".into()))?;
        if text.chars().any(|c| u32::from(c) > 255)
            || text.chars().count() > 179
            || text.contains('\0')
        {
            return Err(PlaybackError(
                "Sample path exceeds the admitted original ANSI path range".into(),
            ));
        }
        let physical = i64::try_from(bytes.len())
            .map_err(|_| PlaybackError("WAV file exceeds the native signed byte range".into()))?;
        if physical > 128 * 1024 * 1024 {
            return Err(PlaybackError(
                "WAV source exceeds the 128 MiB playback bound".into(),
            ));
        }
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
        let repeat = (44100 / wave.sample_rate) as usize;
        let count = input.len() / align;
        let total = count
            .checked_mul(repeat)
            .filter(|&n| n <= packed::MAX_DECODED_SAMPLES)
            .ok_or_else(|| {
                PlaybackError("Converted WAV exceeds the playback frame bound".into())
            })?;
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
        Ok(Self { path, pcm })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Expose the recovered file-loader output for the shared editor adapter.
    pub fn samples(&self) -> &[i16] {
        &self.pcm
    }
    /// Cached full-source contribution length, including packed initial/final runs.
    /// Scheduled ends and the original EOF release rule can stop playback sooner.
    pub fn frame_count(&self) -> usize {
        self.pcm.len()
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
}
