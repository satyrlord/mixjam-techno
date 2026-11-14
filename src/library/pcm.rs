//! Reading a sound's audio from its source, with the stray end that playback
//! hides.

use super::{AudioSource, Library, decode_1252};
use crate::products::Product;
use legacy_format::packed::{self, DecodeOptions, HeaderKind};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

impl Library {
    /// A sample's audio for playback and render, with a stray end hidden.
    pub fn read_pcm(&self, id: u32) -> Result<crate::audio::Pcm, String> {
        let mut pcm = self.read_original_pcm(id)?;
        crate::audio::hide_stray_end(&mut pcm);
        Ok(pcm)
    }

    /// A sample's audio as its source holds it. Identity and digests read this.
    pub fn read_original_pcm(&self, id: u32) -> Result<crate::audio::Pcm, String> {
        let sample = self
            .sample_by_id(id)
            .ok_or("The sample is not in the library.")?;
        let source = self
            .sources
            .get(&id)
            .ok_or("The sample has no audio source.")?;
        let archive_path = match source {
            AudioSource::Wave {
                path,
                origin: Some(Product::T1),
            } => return read_t1_wave(path),
            AudioSource::Wave { path, .. } => return crate::audio::read_wav(path),
            AudioSource::T1Packed { path } => return read_packed_file(path),
            AudioSource::Archive { path } => path,
        };
        if sample.length > packed::MAX_PACKED_BYTES {
            return Err("The compressed sample is too large.".into());
        }
        let mut archive = fs::File::open(archive_path)
            .map_err(|e| format!("Cannot read the original sample archive: {e}"))?;
        archive
            .seek(SeekFrom::Start(sample.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; sample.length as usize];
        archive.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        let samples = decode_archive_record(&bytes, sample.frame_count)?;
        Ok(crate::audio::Pcm {
            sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
            channels: 1,
            samples: samples
                .into_iter()
                .map(|sample| sample as f32 / 32768.0)
                .collect(),
        })
    }

    pub fn step_synth_sources(&self) -> Result<Vec<(PathBuf, String)>, String> {
        if !self.wav_roots.is_empty() {
            return Ok(self
                .samples
                .iter()
                .filter_map(|sample| {
                    sample
                        .wav_path
                        .as_ref()
                        .map(|path| (path.clone(), sample.name.clone()))
                })
                .collect());
        }
        let packed = self
            .archive_path
            .parent()
            .ok_or("The original archive path is invalid.")?;
        let installation = packed
            .parent()
            .ok_or("The original archive path is invalid.")?;
        let text = decode_1252(
            &fs::read(installation.join("\x65jay").join("\x48ypsound"))
                .map_err(|e| e.to_string())?,
        );
        let mut lines: Vec<_> = text.lines().collect();
        while lines.last() == Some(&"") {
            lines.pop();
        }
        if !lines.len().is_multiple_of(3) {
            return Err("The generator source index has an incomplete record.".into());
        }
        let mut sources = Vec::new();
        for row in lines.as_chunks::<3>().0 {
            let relative = Path::new(row[0]);
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err("The generator source index contains an invalid path.".into());
            }
            let path = packed.join(relative);
            if !path.is_file() {
                return Err(format!("Generator source {} is missing.", path.display()));
            }
            sources.push((path, row[1].trim().to_string()));
        }
        Ok(sources)
    }
}

/// Decode one archive record. The catalog's frame count must be the count that
/// the record's header declares, and the decoded audio must have that length.
/// The check comes first, so a record cannot ask for a longer silent tail than
/// the catalog lists.
fn decode_archive_record(record: &[u8], frame_count: u32) -> Result<Vec<i16>, String> {
    let header = packed::parse_header(record).map_err(|e| e.to_string())?;
    if header.kind != HeaderKind::Metadata {
        return Err("The sample does not contain a complete packed header.".into());
    }
    if header.decoded_samples != frame_count as usize {
        return Err("The decoded sample length does not match the catalog.".into());
    }
    let decoded = packed::decode(record, DecodeOptions::ARCHIVE).map_err(|e| e.to_string())?;
    if decoded.samples.len() != frame_count as usize {
        return Err("The decoded sample length does not match the catalog.".into());
    }
    Ok(decoded.samples)
}

pub(super) fn read_t1_wave(path: &Path) -> Result<crate::audio::Pcm, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len()
        > legacy_format::playback::NATIVE_FILE_BYTES
    {
        return Err("The WAV file is larger than 2 GiB.".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let wave = legacy_format::playback::FileSample::from_wave(PathBuf::from("import.wav"), &bytes)
        .map_err(|e| e.to_string())?;
    let samples = wave
        .samples()
        .iter()
        .map(|sample| *sample as f32 / 32768.0)
        .collect();
    Ok(crate::audio::Pcm {
        sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
        channels: 1,
        samples,
    })
}

/// Decode a stand-alone packed sample file. The library and a Step Synth
/// source file decode it the same way: the predictor starts from the header
/// word, and the audio ends at the last complete opcode, even when the
/// header declares a longer duration. Archive records decode differently.
pub fn read_packed_file(path: &Path) -> Result<crate::audio::Pcm, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > packed::MAX_PACKED_BYTES {
        return Err("The compressed sample is too large.".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let decoded = packed::decode(&bytes, DecodeOptions::FILE).map_err(|e| e.to_string())?;
    Ok(crate::audio::Pcm {
        sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
        channels: 1,
        samples: decoded
            .samples
            .iter()
            .map(|sample| f32::from(*sample) / 32768.0)
            .collect(),
    })
}
