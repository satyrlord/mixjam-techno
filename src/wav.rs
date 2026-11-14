//! Reads the layout of a RIFF WAVE file: its format and where its audio is.
//!
//! The reader follows the RIFF rules that sample libraries rely on: a chunk
//! of odd length is followed by a pad byte; a `fmt ` chunk may be longer than
//! the fields it needs; an extensible format names its encoding in its
//! subformat. Audio data that ends inside a frame keeps its whole frames.

use std::io::{self, Read, Seek, SeekFrom};

/// The format tags that the application decodes.
const PCM: u16 = 1;
const IEEE_FLOAT: u16 = 3;
const EXTENSIBLE: u16 = 0xFFFE;

/// The longest WAV file: the RIFF size is 32-bit and the 8-byte RIFF header
/// comes before it. This is a format limit; a longer file is not a WAV.
pub(crate) const MAX_WAV_FILE_BYTES: u64 = u32::MAX as u64 + 8;

/// A WAV's format and the byte span of its whole audio frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WavLayout {
    pub spec: hound::WavSpec,
    /// Storage width can exceed the sample width for classic padded PCM24.
    sample_bytes: u16,
    /// Where the audio data starts in the file.
    pub data_offset: u64,
    /// The audio data in bytes: whole frames only.
    pub data_bytes: u64,
}

impl WavLayout {
    pub fn frame_bytes(&self) -> u64 {
        u64::from(self.spec.channels) * u64::from(self.sample_bytes)
    }
    pub fn frames(&self) -> u64 {
        self.data_bytes / self.frame_bytes()
    }
}

/// Read the layout of the WAV in `file`, whose length is `length` bytes.
pub(crate) fn read_layout(file: &mut (impl Read + Seek), length: u64) -> Result<WavLayout, String> {
    let fail = |text: &str| text.to_string();
    let mut head = [0_u8; 12];
    file.read_exact(&mut head)
        .map_err(|_| fail("This is not a RIFF WAVE file."))?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Err(fail("This is not a RIFF WAVE file."));
    }
    let mut position = 12_u64;
    let mut format = None;
    let mut data = None;
    while position + 8 <= length && (format.is_none() || data.is_none()) {
        file.seek(SeekFrom::Start(position))
            .map_err(|e| e.to_string())?;
        let mut header = [0_u8; 8];
        file.read_exact(&mut header).map_err(|e| e.to_string())?;
        let size = u64::from(u32::from_le_bytes([
            header[4], header[5], header[6], header[7],
        ]));
        let body = position + 8;
        if &header[0..4] != b"data" && size > length.saturating_sub(body) {
            return Err("The WAV file is incomplete: a chunk is cut off.".into());
        }
        match &header[0..4] {
            b"fmt " if format.is_none() => format = Some(read_format(file, size)?),
            b"data" if data.is_none() => data = Some((body, size)),
            _ => {}
        }
        // RIFF pads a chunk of odd length to an even one.
        position = body.saturating_add(size).saturating_add(size & 1);
    }
    let Some((spec, sample_bytes)) = format else {
        return Err(fail("The WAV has no format chunk."));
    };
    let Some((data_offset, declared)) = data else {
        return Err(fail("The WAV has no audio data."));
    };
    let present = length.saturating_sub(data_offset);
    let frame = u64::from(spec.channels) * u64::from(sample_bytes);
    // A data chunk that ends in a partial frame keeps its whole frames. A file
    // that ends before its data does is incomplete, not a variant.
    if present + frame <= declared {
        return Err(fail(
            "The WAV file is incomplete: its audio data is cut off.",
        ));
    }
    let data_bytes = declared.min(present) / frame * frame;
    Ok(WavLayout {
        spec,
        sample_bytes,
        data_offset,
        data_bytes,
    })
}

/// The format from a `fmt ` chunk of `size` bytes at the reader's position.
fn read_format(file: &mut impl Read, size: u64) -> Result<(hound::WavSpec, u16), String> {
    if size < 16 {
        return Err("The WAV format chunk is too short.".into());
    }
    let mut fields = vec![0_u8; size.min(40) as usize];
    file.read_exact(&mut fields).map_err(|e| e.to_string())?;
    let u16_at = |at: usize| u16::from_le_bytes([fields[at], fields[at + 1]]);
    let mut tag = u16_at(0);
    let channels = u16_at(2);
    let sample_rate = u32::from_le_bytes([fields[4], fields[5], fields[6], fields[7]]);
    let byte_rate = u32::from_le_bytes([fields[8], fields[9], fields[10], fields[11]]);
    let block = u16_at(12);
    let bits = u16_at(14);
    let extensible = tag == EXTENSIBLE;
    if tag == EXTENSIBLE {
        if fields.len() < 40 {
            return Err("The WAV extensible format chunk is too short.".into());
        }
        let extension = u64::from(u16_at(16));
        if extension < 22 || extension > size - 18 {
            return Err("The WAV extensible format length is invalid.".into());
        }
        // PCM and IEEE float share this GUID suffix. All 16 bytes identify
        // the encoding; the first two alone can name an unrelated subtype.
        if fields[26..40] != [0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71] {
            return Err("This WAV encoding is not supported.".into());
        }
        tag = u16_at(24);
        let valid_bits = u16_at(18);
        // Zero retains the container width, as in the earlier reader.
        if valid_bits > bits || (tag == IEEE_FLOAT && valid_bits != 0 && valid_bits != bits) {
            return Err("The WAV valid sample width is invalid.".into());
        }
    }
    let sample_format = match (tag, bits) {
        (PCM, 8 | 16 | 24 | 32) => hound::SampleFormat::Int,
        (IEEE_FLOAT, 32) => hound::SampleFormat::Float,
        _ => return Err("This WAV encoding is not supported.".into()),
    };
    if !(1..=2).contains(&channels) || sample_rate == 0 || sample_rate > 384_000 {
        return Err(
            "The WAV must contain mono or stereo audio with a supported sample rate and size."
                .into(),
        );
    }
    let sample_bytes = block / channels;
    let padded_pcm24 = !extensible && tag == PCM && bits == 24 && sample_bytes == 4;
    if !block.is_multiple_of(channels)
        || (sample_bytes != bits / 8 && !padded_pcm24)
        || byte_rate != sample_rate * u32::from(block)
    {
        return Err("The WAV frame size or byte rate does not match its format.".into());
    }
    Ok((
        hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: bits,
            sample_format,
        },
        sample_bytes,
    ))
}

/// Decode the whole frames of `layout` from `file`, calling `visit` with each
/// sample as a float in -1..1 (integers) or as stored (floats).
pub(crate) fn visit_samples(
    file: &mut (impl Read + Seek),
    layout: &WavLayout,
    mut visit: impl FnMut(f32) -> Result<(), String>,
) -> Result<(), String> {
    file.seek(SeekFrom::Start(layout.data_offset))
        .map_err(|e| e.to_string())?;
    let width = usize::from(layout.sample_bytes);
    let mut remaining = layout.data_bytes;
    let mut buffer = vec![0_u8; 64 * 1024 / width * width];
    let divisor = (1_u64 << (layout.spec.bits_per_sample - 1)) as f32;
    while remaining > 0 {
        let take = (buffer.len() as u64).min(remaining) as usize;
        let chunk = &mut buffer[..take];
        file.read_exact(chunk)
            .map_err(|e: io::Error| e.to_string())?;
        for bytes in chunk.chunks_exact(width) {
            let value = match (layout.spec.sample_format, width) {
                (hound::SampleFormat::Float, _) => {
                    f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
                }
                // 8-bit WAV samples are unsigned around 128.
                (_, 1) => (i32::from(bytes[0]) - 128) as f32 / divisor,
                (_, 2) => f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / divisor,
                (_, _) if layout.spec.bits_per_sample == 24 => {
                    (i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) >> 8) as f32 / divisor
                }
                _ => i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32 / divisor,
            };
            visit(value)?;
        }
        remaining -= take as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
        out
    }

    fn fmt(tag: u16, channels: u16, rate: u32, bits: u16, extra: usize) -> Vec<u8> {
        let block = channels * bits / 8;
        let mut body = Vec::new();
        body.extend(tag.to_le_bytes());
        body.extend(channels.to_le_bytes());
        body.extend(rate.to_le_bytes());
        body.extend((rate * u32::from(block)).to_le_bytes());
        body.extend(block.to_le_bytes());
        body.extend(bits.to_le_bytes());
        body.extend(vec![0_u8; extra]);
        chunk(b"fmt ", &body)
    }

    fn wav(chunks: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = chunks.concat();
        let mut out = b"RIFF".to_vec();
        out.extend((body.len() as u32 + 4).to_le_bytes());
        out.extend(b"WAVE");
        out.extend(body);
        out
    }

    fn decode(bytes: &[u8]) -> Result<(WavLayout, Vec<f32>), String> {
        let mut file = Cursor::new(bytes);
        let layout = read_layout(&mut file, bytes.len() as u64)?;
        let mut samples = Vec::new();
        visit_samples(&mut file, &layout, |value| {
            samples.push(value);
            Ok(())
        })?;
        Ok((layout, samples))
    }

    fn pcm16(values: &[i16]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn an_odd_chunk_before_the_audio_is_padded() {
        // Broadcast WAVs often carry a 603-byte `bext` chunk before `data`.
        let bytes = wav(&[
            fmt(PCM, 1, 44_100, 16, 0),
            chunk(b"bext", &[7; 603]),
            chunk(b"data", &pcm16(&[16_384, -16_384])),
        ]);
        let (layout, samples) = decode(&bytes).unwrap();
        assert_eq!(layout.frames(), 2);
        assert_eq!(samples, [0.5, -0.5]);
    }

    #[test]
    fn audio_that_ends_in_a_partial_frame_keeps_its_whole_frames() {
        let mut data = vec![0, 0, 64, 0, 0, 192];
        data.push(1);
        let bytes = wav(&[fmt(PCM, 1, 48_000, 24, 0), chunk(b"data", &data)]);
        let (layout, samples) = decode(&bytes).unwrap();
        assert_eq!(layout.frames(), 2);
        assert_eq!(samples, [0.5, -0.5]);
    }

    #[test]
    fn a_long_float_format_chunk_reads_its_first_fields() {
        // A 40-byte float `fmt ` chunk with a zero cbSize, as some editors write.
        let audio: Vec<u8> = [0.25_f32, -1.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let bytes = wav(&[
            chunk(b"JUNK", &[0; 92]),
            fmt(IEEE_FLOAT, 1, 48_000, 32, 24),
            chunk(b"data", &audio),
            chunk(b"regn", &[0; 92]),
        ]);
        let (layout, samples) = decode(&bytes).unwrap();
        assert_eq!(layout.spec.sample_format, hound::SampleFormat::Float);
        assert_eq!(samples, [0.25, -1.0]);
    }

    #[test]
    fn an_extensible_format_uses_its_subformat() {
        let mut extensible = fmt(EXTENSIBLE, 2, 44_100, 16, 24);
        // cbSize 22, then valid bits, the channel mask, and the PCM subformat GUID.
        extensible[24..26].copy_from_slice(&22_u16.to_le_bytes());
        extensible[26..28].copy_from_slice(&16_u16.to_le_bytes());
        extensible[32..34].copy_from_slice(&PCM.to_le_bytes());
        extensible[36..48].copy_from_slice(&[
            0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
        ]);
        let bytes = wav(&[extensible, chunk(b"data", &pcm16(&[8_192, -8_192]))]);
        let (layout, samples) = decode(&bytes).unwrap();
        assert_eq!((layout.spec.channels, layout.frames()), (2, 1));
        assert_eq!(samples, [0.25, -0.25]);
    }

    #[test]
    fn invalid_extensible_headers_are_rejected() {
        let mut valid = fmt(EXTENSIBLE, 1, 44_100, 16, 24);
        valid[24..26].copy_from_slice(&22_u16.to_le_bytes());
        valid[26..28].copy_from_slice(&16_u16.to_le_bytes());
        valid[32..48].copy_from_slice(&[
            1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
        ]);
        for (at, value) in [(24, 0), (24, 24), (26, 17), (34, 1), (47, 0)] {
            let mut invalid = valid.clone();
            invalid[at] = value;
            let bytes = wav(&[invalid, chunk(b"data", &pcm16(&[1, 2]))]);
            assert!(decode(&bytes).is_err(), "accepted field {at} = {value}");
        }
    }

    #[test]
    fn extensible_float_and_left_aligned_pcm_decode_at_container_width() {
        for (tag, valid_bits, audio) in [
            (PCM, 24_u16, [0x4000_0000_u32, 0xc000_0000]),
            (PCM, 0, [0x4000_0000, 0xc000_0000]),
            (IEEE_FLOAT, 32, [0.5_f32.to_bits(), (-0.5_f32).to_bits()]),
        ] {
            let mut format = fmt(EXTENSIBLE, 2, 48_000, 32, 28);
            format[24..26].copy_from_slice(&26_u16.to_le_bytes());
            format[26..28].copy_from_slice(&valid_bits.to_le_bytes());
            format[32..34].copy_from_slice(&tag.to_le_bytes());
            format[36..48].copy_from_slice(&[0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]);
            let data: Vec<u8> = audio.into_iter().flat_map(u32::to_le_bytes).collect();
            // A complete format after data is supported too.
            let bytes = wav(&[chunk(b"data", &data), format]);
            let (layout, samples) = decode(&bytes).unwrap();
            assert_eq!(layout.frames(), 1);
            assert_eq!(samples, [0.5, -0.5]);
        }
    }

    #[test]
    fn inconsistent_frame_alignment_is_rejected() {
        for block in [0_u16, 2, 8] {
            let mut format = fmt(PCM, 2, 44_100, 16, 0);
            format[20..22].copy_from_slice(&block.to_le_bytes());
            let bytes = wav(&[format, chunk(b"data", &pcm16(&[1, 2, 3, 4]))]);
            assert!(decode(&bytes).is_err(), "accepted block alignment {block}");
        }
    }

    #[test]
    fn classic_pcm24_in_four_byte_containers_keeps_its_stride() {
        let mut format = fmt(PCM, 1, 44_100, 24, 0);
        format[16..20].copy_from_slice(&176_400_u32.to_le_bytes());
        format[20..22].copy_from_slice(&4_u16.to_le_bytes());
        let bytes = wav(&[
            format,
            chunk(b"data", &[0, 0, 64, 0, 0, 0, 192, 0, 0, 0, 64, 0]),
        ]);
        let (layout, samples) = decode(&bytes).unwrap();
        assert_eq!(layout.frames(), 3);
        assert_eq!(samples, [0.5, -0.5, 0.5]);
    }

    #[test]
    fn a_truncated_format_after_audio_is_rejected() {
        let mut bytes = wav(&[
            chunk(b"data", &pcm16(&[1, 2])),
            fmt(PCM, 1, 44_100, 16, 100),
        ]);
        bytes.truncate(bytes.len() - 50);
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn eight_bit_audio_is_unsigned() {
        let bytes = wav(&[fmt(PCM, 1, 22_050, 8, 0), chunk(b"data", &[192, 64])]);
        assert_eq!(decode(&bytes).unwrap().1, [0.5, -0.5]);
    }

    #[test]
    fn unusable_files_are_rejected() {
        let cases = [
            (Vec::new(), "not a RIFF WAVE"),
            (
                b"\x00\x05\x16\x07\x00\x02\x00\x00Mac OS X".to_vec(),
                "not a RIFF WAVE",
            ),
            (
                wav(&[fmt(2, 2, 44_100, 4, 34), chunk(b"data", &[0; 16])]),
                "encoding is not supported",
            ),
            (wav(&[fmt(PCM, 1, 44_100, 16, 0)]), "no audio data"),
            (wav(&[chunk(b"data", &[0; 4])]), "no format chunk"),
        ];
        for (bytes, error) in cases {
            let result = decode(&bytes);
            assert!(
                result.as_ref().is_err_and(|text| text.contains(error)),
                "{result:?}"
            );
        }
        // A file that ends long before its declared audio does is incomplete.
        let mut cut = wav(&[fmt(PCM, 1, 44_100, 16, 0), chunk(b"data", &[0; 400])]);
        cut.truncate(cut.len() - 200);
        assert!(decode(&cut).unwrap_err().contains("incomplete"));
    }
}
