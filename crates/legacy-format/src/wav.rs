//! RIFF/WAVE PCM container I/O. Audio processing belongs to the mixer.
//! This module uses the standard container; its validation rules are new Rust
//! bounds checks, not a reproduction of the original's malformed-file behavior.
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveError(pub String);
impl fmt::Display for WaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for WaveError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmWave {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}
impl PcmWave {
    pub fn parse(data: &[u8]) -> Result<Self, WaveError> {
        let error = |s: &str| WaveError(s.into());
        if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(error("Expected RIFF/WAVE audio"));
        }
        let end = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let end = end
            .checked_add(8)
            .filter(|&v| v >= 12 && v <= data.len())
            .ok_or_else(|| error("Incomplete RIFF payload"))?;
        let mut position = 12usize;
        let mut format = None;
        let mut audio = None;
        while position < end {
            let header = data
                .get(position..position + 8)
                .filter(|_| position + 8 <= end)
                .ok_or_else(|| error("Incomplete WAVE chunk header"))?;
            let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            let start = position + 8;
            let limit = start
                .checked_add(size)
                .filter(|&v| v <= end)
                .ok_or_else(|| error("Incomplete WAVE chunk payload"))?;
            let chunk = &data[start..limit];
            match &header[..4] {
                b"fmt " => {
                    if format.is_some() || chunk.len() < 16 {
                        return Err(error("Invalid or duplicate WAVE format chunk"));
                    }
                    let word = |i| u16::from_le_bytes([chunk[i], chunk[i + 1]]);
                    let rate = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
                    let channels = word(2);
                    let bits = word(14);
                    if word(0) != 1
                        || !matches!(channels, 1 | 2)
                        || !matches!(bits, 8 | 16)
                        || rate == 0
                    {
                        return Err(error(
                            "WAVE needs 8-bit or 16-bit PCM with one or two channels",
                        ));
                    }
                    let align = channels * (bits / 8);
                    if word(12) != align
                        || u32::from_le_bytes(chunk[8..12].try_into().unwrap())
                            != rate
                                .checked_mul(u32::from(align))
                                .ok_or_else(|| error("WAVE rate exceeds supported range"))?
                    {
                        return Err(error("Inconsistent WAVE block alignment or byte rate"));
                    }
                    format = Some((rate, channels, bits, align));
                }
                b"data" if audio.is_some() => {
                    return Err(error("Multiple WAVE data chunks are unsupported"));
                }
                b"data" => audio = Some(chunk),
                _ => {}
            }
            position = limit
                .checked_add(size & 1)
                .filter(|&v| v <= end)
                .ok_or_else(|| error("Missing WAVE chunk padding"))?;
        }
        let (sample_rate, channels, bits, align) =
            format.ok_or_else(|| error("Missing WAVE format chunk"))?;
        let audio = audio.ok_or_else(|| error("Missing WAVE data chunk"))?;
        if audio.len() % usize::from(align) != 0 {
            return Err(error("Incomplete WAVE sample frame"));
        }
        let samples = if bits == 16 {
            audio
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect()
        } else {
            audio.iter().map(|&b| (i16::from(b) - 128) * 256).collect()
        };
        Ok(Self {
            sample_rate,
            channels,
            samples,
        })
    }
    pub fn encode(&self) -> Result<Vec<u8>, WaveError> {
        if !matches!(self.channels, 1 | 2)
            || self.sample_rate == 0
            || !self
                .samples
                .len()
                .is_multiple_of(usize::from(self.channels))
        {
            return Err(WaveError(
                "Invalid PCM format or incomplete sample frame".into(),
            ));
        }
        let count = u32::try_from(
            self.samples
                .len()
                .checked_mul(2)
                .ok_or_else(|| WaveError("WAVE is too large".into()))?,
        )
        .map_err(|_| WaveError("WAVE is too large".into()))?;
        let mut out = header(self.sample_rate, self.channels, count)?;
        for sample in &self.samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(out)
    }
}
fn header(sample_rate: u32, channels: u16, data_bytes: u32) -> Result<Vec<u8>, WaveError> {
    let align = channels * 2;
    let rate = sample_rate
        .checked_mul(u32::from(align))
        .ok_or_else(|| WaveError("WAVE rate exceeds supported range".into()))?;
    let riff = data_bytes
        .checked_add(36)
        .ok_or_else(|| WaveError("WAVE is too large".into()))?;
    let mut b = Vec::with_capacity(44);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&riff.to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&align.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_bytes.to_le_bytes());
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm16_stereo_roundtrip() {
        let w = PcmWave {
            sample_rate: 44100,
            channels: 2,
            samples: vec![-32768, 32767, 0, 1, -1, 1000],
        };
        let bytes = w.encode().unwrap();
        assert_eq!(PcmWave::parse(&bytes).unwrap(), w);
    }
    #[test]
    fn accepts_unknown_padded_chunk_and_converts_unsigned8() {
        let mut b = PcmWave {
            sample_rate: 11025,
            channels: 1,
            samples: vec![],
        }
        .encode()
        .unwrap();
        b[34..36].copy_from_slice(&8u16.to_le_bytes());
        b[28..32].copy_from_slice(&11025u32.to_le_bytes());
        b[32..34].copy_from_slice(&1u16.to_le_bytes());
        b[40..44].copy_from_slice(&3u32.to_le_bytes());
        b.extend([0, 128, 255, 0]);
        b.splice(12..12, b"JUNK\x03\0\0\0abc\0".iter().copied());
        let len = b.len() as u32 - 8;
        b[4..8].copy_from_slice(&len.to_le_bytes());
        assert_eq!(PcmWave::parse(&b).unwrap().samples, [-32768, 0, 32512]);
    }
    #[test]
    fn truncated_chunks_and_partial_frames_are_errors() {
        let w = PcmWave {
            sample_rate: 44100,
            channels: 2,
            samples: vec![1, 2],
        };
        let mut bytes = w.encode().unwrap();
        bytes.pop();
        assert!(PcmWave::parse(&bytes).is_err());
        assert!(PcmWave {
            samples: vec![1],
            ..w
        }
        .encode()
        .is_err());
    }
}
