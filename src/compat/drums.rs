//! Observed D2/D3/4 drum import projection. Original pattern and pad bytes remain available.
//! The dry renderer follows the documented sixteen-step, one-bar drum matrix.
//! It applies the Dance 2 Groove Generator volume and no other saved effect.
use crate::{
    audio::Pcm,
    library::{Library, decode_1252},
    products::Product,
};
use serde::{Deserialize, Serialize};

const RENDERER_POLICY: &str = "dry-drum-import-v3: bit-zero-first; sixteen steps per bar; equal-temperament pitch; per-pad reverse and L/R routing; native 1.25 fader curve with Q16 truncation; independent hits; one-bar output; final saturation; then Groove Generator volume as floor(word / 4) / 8192; saved filter, echo, and distortion named and not applied";
const GEN3_RENDERER_POLICY: &str = "dry-drum-import-v2: bit-zero-first; sixteen pads and sixteen steps per bar; equal-temperament pitch; per-pad reverse and linear pan attenuation; explicit dry linear stored/32767 pad gain projection; independent hits; one-bar output; final saturation; opaque native effects retained without interpretation";

pub(super) fn renderer_policy(product: Product) -> &'static str {
    if product == Product::D2 {
        RENDERER_POLICY
    } else {
        GEN3_RENDERER_POLICY
    }
}

fn pad_count(product: Product) -> Result<usize, String> {
    match product {
        Product::D2 => Ok(10),
        Product::D3 | Product::D4 => Ok(16),
        _ => Err("This product's generated drum profile is not supported.".into()),
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub(crate) struct Pattern {
    pub native_id: u16,
    pub masks: Vec<u16>,
    pub group: String,
    pub variant: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Pad {
    pub native_id: u16,
    pub name: String,
    pub pitch_semitones: i16,
    pub reverse: bool,
    pub gain_native: u16,
    pub routing_native: u16,
    pub raw_settings: [u8; 8],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pan_native: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opaque_native_settings: Vec<u8>,
}

/// The nine words that Dance 2 saves after its ten pads, in file order. The
/// editor sends each to the audio engine unchanged (`DFilterLed`, `DCutOff`,
/// `DResonance`, `DEchoLed`, `DEchoTime`, `DFeedback`, `DDistortionLed`,
/// `DDistortion`, `DVolume`). A saved switch is 0 or 65535.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct GrooveEffects {
    pub filter_on: bool,
    pub filter_cutoff: u16,
    pub filter_resonance: u16,
    pub echo_on: bool,
    pub echo_time: u16,
    pub echo_feedback: u16,
    pub distortion_on: bool,
    pub distortion: u16,
    pub volume: u16,
}

impl GrooveEffects {
    fn read(bytes: &[u8]) -> Option<Self> {
        let words: [u8; 18] = bytes.try_into().ok()?;
        let word = |index: usize| u16::from_le_bytes([words[index * 2], words[index * 2 + 1]]);
        Some(Self {
            filter_on: word(0) != 0,
            filter_cutoff: word(1),
            filter_resonance: word(2),
            echo_on: word(3) != 0,
            echo_time: word(4),
            echo_feedback: word(5),
            distortion_on: word(6) != 0,
            distortion: word(7),
            volume: word(8),
        })
    }

    // D2 pxd32d4 mixer, RVA 0x7d9f: each drum sample is multiplied by the
    // signed volume word shifted right by 2, then shifted right by 13.
    fn volume_gain(&self) -> f32 {
        f32::from((self.volume as i16) >> 2) / 8192.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Kit {
    pub pads: Vec<Pad>,
    pub source_offset: usize,
    pub source_end: usize,
    pub opaque_native_settings: Vec<u8>,
    /// Dance 2 only: `opaque_native_settings` by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groove_effects: Option<GrooveEffects>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_end: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_header: Vec<u8>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or("The native drum record is truncated.")?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn wrapped_text(&mut self) -> Result<String, String> {
        let length = usize::from(self.u16()?);
        if !(2..=260).contains(&length) {
            return Err("The native drum label has an unsupported length.".into());
        }
        let value = self.take(length)?;
        if !value.ends_with(&[0, 1]) {
            return Err("The native drum label marker is invalid.".into());
        }
        let text = decode_1252(&value[..length - 2]);
        if text.chars().any(char::is_control) {
            return Err("The native drum label has control characters.".into());
        }
        Ok(text)
    }
    fn bare_text(&mut self, maximum: usize) -> Result<String, String> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err("The native drum string exceeds its supported length.".into());
        }
        let text = decode_1252(self.take(length)?);
        if text.chars().any(char::is_control) {
            return Err("The native drum string has control characters.".into());
        }
        Ok(text)
    }
}

pub(super) fn parse_pattern(
    bytes: &[u8],
    native_id: u16,
    product: Product,
) -> Result<(Pattern, usize), String> {
    let mut reader = Reader { bytes, offset: 0 };
    let mut masks = vec![0; pad_count(product)?];
    for mask in &mut masks {
        *mask = reader.u16()?;
    }
    let group = reader.wrapped_text()?;
    let variant = reader.wrapped_text()?;
    Ok((
        Pattern {
            native_id,
            masks,
            group,
            variant,
        },
        reader.offset,
    ))
}

pub(super) fn parse_kit(
    bytes: &[u8],
    lower_bound: usize,
    timeline_start: usize,
    product: Product,
) -> Result<Kit, String> {
    if lower_bound > timeline_start || timeline_start > bytes.len() {
        return Err("The native drum header range is invalid.".into());
    }
    let mut found = None;
    let count = pad_count(product)?;
    let earliest =
        lower_bound.max(timeline_start.saturating_sub(if count == 10 { 2774 } else { 65536 }));
    for start in earliest..timeline_start {
        let candidate = if count == 10 {
            parse_kit_at(bytes, start, timeline_start)
        } else {
            parse_gen3_kit_at(bytes, start, timeline_start)
        };
        if let Ok(kit) = candidate {
            if found.is_some() {
                return Err("The native drum pad header is ambiguous.".into());
            }
            found = Some(kit);
        }
    }
    found.ok_or_else(|| "The native drum pad header is not supported.".into())
}

fn parse_kit_at(bytes: &[u8], start: usize, end: usize) -> Result<Kit, String> {
    let mut reader = Reader {
        bytes: &bytes[..end],
        offset: start,
    };
    let mut pads = Vec::with_capacity(10);
    for index in 0..10 {
        let native_id = reader.u16()?;
        let name = reader.wrapped_text()?;
        let raw_settings: [u8; 8] = reader.take(8)?.try_into().unwrap();
        let pitch_semitones = i16::from_le_bytes(raw_settings[..2].try_into().unwrap());
        let reverse = u16::from_le_bytes(raw_settings[2..4].try_into().unwrap());
        let gain_native = u16::from_le_bytes(raw_settings[4..6].try_into().unwrap());
        let routing_native = u16::from_le_bytes(raw_settings[6..].try_into().unwrap());
        if !(-12..=12).contains(&pitch_semitones)
            || !matches!(reverse, 0 | 65535)
            || gain_native > 32767
            || routing_native > 3
        {
            return Err("The native drum pad settings are not supported.".into());
        }
        pads.push(Pad {
            native_id,
            name,
            pitch_semitones,
            reverse: reverse != 0,
            gain_native,
            routing_native,
            raw_settings,
            pan_native: None,
            source_paths: Vec::new(),
            opaque_native_settings: Vec::new(),
        });
        if index < 9 && !reader.wrapped_text()?.is_empty() {
            return Err("The native drum pad separator is invalid.".into());
        }
    }
    let source_end = reader.offset;
    let opaque_native_settings = reader.take(18)?.to_vec();
    if reader.offset != end {
        return Err("The native drum header does not end at the timeline.".into());
    }
    Ok(Kit {
        pads,
        source_offset: start,
        source_end,
        groove_effects: GrooveEffects::read(&opaque_native_settings),
        opaque_native_settings,
        header_end: None,
        raw_header: Vec::new(),
    })
}

// Gen 3 uses bare strings, sixteen pads, paired cached source paths, and an
// opaque ten-byte extension per pad. Keep these bytes instead of naming DSP.
fn parse_gen3_kit_at(bytes: &[u8], start: usize, end: usize) -> Result<Kit, String> {
    let mut reader = Reader {
        bytes: &bytes[..end],
        offset: start,
    };
    let mut pads = Vec::with_capacity(16);
    for _ in 0..16 {
        let native_id = reader.u16()?;
        let name = reader.bare_text(260)?;
        let raw_settings: [u8; 8] = reader.take(8)?.try_into().unwrap();
        let pitch_semitones = i16::from_le_bytes(raw_settings[..2].try_into().unwrap());
        let pan = u16::from_le_bytes(raw_settings[2..4].try_into().unwrap());
        let reverse = u16::from_le_bytes(raw_settings[4..6].try_into().unwrap());
        let gain_native = u16::from_le_bytes(raw_settings[6..].try_into().unwrap());
        let first_path = reader.bare_text(4096)?;
        let second_path = reader.bare_text(4096)?;
        if native_id == 0
            || name.is_empty()
            || !(-12..=12).contains(&pitch_semitones)
            || pan > 255
            || !matches!(reverse, 0 | 65535)
            || gain_native > 32767
            || first_path.is_empty()
            || first_path != second_path
        {
            return Err("The native sixteen-pad drum header is not supported.".into());
        }
        let opaque_native_settings = reader.take(10)?.to_vec();
        pads.push(Pad {
            native_id,
            name,
            pitch_semitones,
            reverse: reverse != 0,
            gain_native,
            routing_native: 0,
            raw_settings,
            pan_native: Some(pan),
            source_paths: vec![first_path, second_path],
            opaque_native_settings,
        });
    }
    let source_end = reader.offset;
    // Observed files retain the selected pad once more, followed by opaque state.
    // The selected pad identity/label anchors the end of the sixteen-pad list.
    let selected_id = reader.u16()?;
    let selected_name = reader.bare_text(260)?;
    if !pads
        .iter()
        .any(|pad| pad.native_id == selected_id && pad.name == selected_name)
        || !(18..=256).contains(&end.saturating_sub(reader.offset))
    {
        return Err("The native drum header footer is not supported.".into());
    }
    Ok(Kit {
        pads,
        source_offset: start,
        source_end,
        opaque_native_settings: bytes[source_end..end].to_vec(),
        groove_effects: None,
        header_end: Some(end),
        raw_header: bytes[start..end].to_vec(),
    })
}

struct PreparedPad {
    pcm: Pcm,
    pitch: f64,
    gain: f32,
    reverse: bool,
    left: f32,
    right: f32,
}
pub(super) struct PreparedKit {
    pads: Vec<Option<PreparedPad>>,
    volume: f32,
}

/// A notice for a Dance 2 kit whose saved effects the renderer does not apply.
pub(super) fn effects_notice(kit: &Kit) -> Option<String> {
    let effects = kit.groove_effects?;
    let active: Vec<_> = [
        (effects.filter_on, "filter"),
        (effects.echo_on, "echo"),
        (effects.distortion_on, "distortion"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| name)
    .collect();
    (!active.is_empty()).then(|| {
        format!(
            "Saved Groove Generator effects are not applied: {}. Drum patterns play without them.",
            active.join(", ")
        )
    })
}

pub(super) fn prepare_kit<'a>(
    kit: &Kit,
    library: &Library,
    product: Product,
    patterns: impl IntoIterator<Item = &'a Pattern>,
) -> Result<PreparedKit, String> {
    if kit.pads.len() != pad_count(product)? {
        return Err("The native drum kit has an invalid pad count.".into());
    }
    let mut active = vec![false; kit.pads.len()];
    for pattern in patterns {
        if pattern.masks.len() != kit.pads.len() {
            return Err("The native drum kit and pattern pad counts differ.".into());
        }
        for (used, mask) in active.iter_mut().zip(&pattern.masks) {
            *used |= *mask != 0;
        }
    }
    let mut pads = Vec::with_capacity(kit.pads.len());
    for (pad, used) in kit.pads.iter().zip(active) {
        if !used {
            pads.push(None);
            continue;
        }
        if pad.native_id == 0 {
            return Err("An active drum pattern pad has no sample.".into());
        }
        let id = product.global_sample_id(u32::from(pad.native_id))?;
        let pcm = library
            .read_pcm(id)
            .map_err(|error| format!("Cannot load drum pad {}: {error}", pad.name))?;
        pcm.validate()?;
        if pcm.frames() == 0 {
            return Err("A native drum pad has no audio frames.".into());
        }
        let (left, right) = if let Some(pan) = pad.pan_native {
            let pan = (f32::from(pan) - 128.0) / if pan <= 128 { 128.0 } else { 127.0 };
            (1.0 - pan.max(0.0), 1.0 + pan.min(0.0))
        } else {
            (
                if matches!(pad.routing_native, 0 | 1) {
                    1.0
                } else {
                    0.0
                },
                if matches!(pad.routing_native, 0 | 2) {
                    1.0
                } else {
                    0.0
                },
            )
        };
        pads.push(Some(PreparedPad {
            pcm,
            pitch: 2.0f64.powf(f64::from(pad.pitch_semitones) / 12.0),
            // D2 packed32d4 RDrum, RVA 0xf9c6, uses the 1.25 fader curve and
            // signed Q16 output.
            gain: if product == Product::D2 {
                super::chunks::native_fader_gain(pad.gain_native, 1.25)
            } else {
                f32::from(pad.gain_native) / 32767.0
            },
            reverse: pad.reverse,
            left,
            right,
        }));
    }
    // A kit saved before the words had names holds only the bytes.
    let volume = if product == Product::D2 {
        GrooveEffects::read(&kit.opaque_native_settings)
            .ok_or("The native drum kit has no Groove Generator settings.")?
            .volume_gain()
    } else {
        1.0
    };
    Ok(PreparedKit { pads, volume })
}

pub(super) fn render(prepared: &PreparedKit, pattern: &Pattern, bpm: f64) -> Result<Pcm, String> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err("The native drum tempo must be finite and positive.".into());
    }
    if !matches!(prepared.pads.len(), 10 | 16) || prepared.pads.len() != pattern.masks.len() {
        return Err("The native drum kit and pattern pad counts differ.".into());
    }
    let rate = 44100;
    let exact_bar_frames = f64::from(rate) * 240.0 / bpm;
    if !(1.0..=32_000_000.0).contains(&exact_bar_frames) {
        return Err("The native drum pattern duration is not supported.".into());
    }
    let frames = exact_bar_frames.round() as usize;
    let mut output = vec![0.0f32; frames * 2];
    for (pad_index, &mask) in pattern.masks.iter().enumerate() {
        if mask == 0 {
            continue;
        }
        let pad = prepared.pads[pad_index]
            .as_ref()
            .ok_or("An active drum pattern pad has no sample.")?;
        let source_frames = pad.pcm.frames() as usize;
        let step = f64::from(pad.pcm.sample_rate) / f64::from(rate) * pad.pitch;
        for note in 0..16 {
            if mask & (1u16 << note) == 0 {
                continue;
            }
            let start = (exact_bar_frames * f64::from(note) / 16.0).round() as usize;
            for frame in start..frames {
                let position = (frame - start) as f64 * step;
                if position >= source_frames as f64 {
                    break;
                }
                let first = position.floor() as usize;
                let next = (first + 1).min(source_frames - 1);
                let fraction = (position - first as f64) as f32;
                let first = if pad.reverse {
                    source_frames - 1 - first
                } else {
                    first
                };
                let next = if pad.reverse {
                    source_frames - 1 - next
                } else {
                    next
                };
                for channel in 0..2 {
                    let channel_gain = if channel == 0 { pad.left } else { pad.right };
                    if channel_gain == 0.0 {
                        continue;
                    }
                    let source_channel = if pad.pcm.channels == 1 { 0 } else { channel };
                    let stride = usize::from(pad.pcm.channels);
                    let a = pad.pcm.samples[first * stride + source_channel];
                    let b = pad.pcm.samples[next * stride + source_channel];
                    output[frame * 2 + channel] +=
                        (a + (b - a) * fraction) * pad.gain * channel_gain;
                }
            }
        }
    }
    for value in &mut output {
        *value = value.clamp(-1.0, 1.0) * prepared.volume;
    }
    let pcm = Pcm {
        sample_rate: rate,
        channels: 2,
        samples: output,
    };
    pcm.validate()?;
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wrapped(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&((value.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
        bytes.extend_from_slice(&[0, 1]);
    }
    fn fixture_kit() -> Vec<u8> {
        let mut bytes = Vec::new();
        for index in 0..10 {
            bytes.extend_from_slice(&(1518u16 + index).to_le_bytes());
            wrapped(&mut bytes, "Pad");
            bytes.extend_from_slice(&0i16.to_le_bytes());
            bytes.extend_from_slice(&0u16.to_le_bytes());
            bytes.extend_from_slice(&32767u16.to_le_bytes());
            bytes.extend_from_slice(&0u16.to_le_bytes());
            if index < 9 {
                wrapped(&mut bytes, "");
            }
        }
        bytes.extend_from_slice(&[7; 18]);
        bytes
    }
    fn prepared_pad(
        pcm: Vec<f32>,
        pitch: f64,
        reverse: bool,
        gain: f32,
        left: bool,
        right: bool,
    ) -> PreparedKit {
        let mut pads: Vec<Option<PreparedPad>> = (0..10).map(|_| None).collect();
        pads[0] = Some(PreparedPad {
            pcm: Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: pcm,
            },
            pitch,
            gain,
            reverse,
            left: if left { 1.0 } else { 0.0 },
            right: if right { 1.0 } else { 0.0 },
        });
        PreparedKit { pads, volume: 1.0 }
    }
    fn pattern(mask: u16) -> Pattern {
        let mut masks = vec![0; 10];
        masks[0] = mask;
        Pattern {
            native_id: 7203,
            masks,
            group: "Start".into(),
            variant: "1".into(),
        }
    }

    #[test]
    fn drum_pad_audio_uses_wav_provider_without_original_archive() {
        let root = std::env::temp_dir().join(format!(
            "mixjam-techno-wav-drum-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("drum.wav");
        let wave = legacy_format::wav::PcmWave {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![8192; 4],
        }
        .encode()
        .unwrap();
        std::fs::write(&path, wave).unwrap();
        let metadata = crate::catalog::WaveMetadata {
            native_id: 1518,
            code: "drum".into(),
            name: "Drum".into(),
            group_name: "Drum".into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 4,
            pcm_digest: None,
        };
        let mut library = Library::empty();
        library.import_wav_folder(&root, 140.0).unwrap();
        library
            .register_wav_product(Product::D2, &root, &[metadata])
            .unwrap();
        let bytes = fixture_kit();
        let mut kit = parse_kit(&bytes, 0, bytes.len(), Product::D2).unwrap();
        let first = pattern(1);
        // Unplayed pads keep their saved IDs even when their WAVs are absent.
        let prepared = prepare_kit(&kit, &library, Product::D2, [&first]).unwrap();
        let rendered = render(&prepared, &first, 140.0).unwrap();
        // A later pattern can use a different pad. Prepare the union of all hits.
        kit.pads[1].native_id = kit.pads[0].native_id;
        let mut second = pattern(0);
        second.masks[1] = 1;
        let prepared = prepare_kit(&kit, &library, Product::D2, [&first, &second]).unwrap();
        assert_eq!(
            render(&prepared, &second, 140.0).unwrap().samples,
            rendered.samples
        );
        let mut missing = pattern(0);
        missing.masks[2] = 1;
        assert!(prepare_kit(&kit, &library, Product::D2, [&first, &missing]).is_err());
        kit.pads[2].native_id = 0;
        assert!(prepare_kit(&kit, &library, Product::D2, [&missing]).is_err());
        assert!(rendered.samples[0] > 0.0);
        assert_eq!(rendered.samples[8], 0.0);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn masks_preserve_all_sixteen_bits_and_variable_labels() {
        let mut bytes = Vec::new();
        for mask in [0x8001u16, 0xffff, 0x1111, 0, 0, 0, 0, 0, 0, 0] {
            bytes.extend_from_slice(&mask.to_le_bytes());
        }
        wrapped(&mut bytes, "Variable group");
        wrapped(&mut bytes, "");
        let (parsed, consumed) = parse_pattern(&bytes, 7335, Product::D2).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(parsed.masks[..3], [0x8001, 0xffff, 0x1111]);
        assert_eq!(parsed.native_id, 7335);
        assert_eq!(parsed.group, "Variable group");
        assert!(parsed.variant.is_empty());
        assert!(parse_pattern(&bytes[..bytes.len() - 1], 7335, Product::D2).is_err());
    }

    #[test]
    fn kit_has_exact_ten_records_and_retains_opaque_bytes() {
        let bytes = fixture_kit();
        let kit = parse_kit(&bytes, 0, bytes.len(), Product::D2).unwrap();
        assert_eq!(kit.pads.len(), 10);
        assert_eq!(kit.pads[0].native_id, 1518);
        assert_eq!(kit.pads[9].native_id, 1527);
        assert_eq!(kit.opaque_native_settings, [7; 18]);
        assert_eq!(kit.groove_effects.unwrap().volume, 0x0707);
        assert!(parse_kit(&bytes[..bytes.len() - 1], 0, bytes.len() - 1, Product::D2).is_err());
        let mut malformed = bytes;
        malformed[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(parse_kit(&malformed, 0, malformed.len(), Product::D2).is_err());
    }

    #[test]
    fn groove_effect_words_have_names_and_the_volume_scales_the_render() {
        // The block of the factory song SENNER.MIX.
        let words = [65535u16, 24421, 11611, 65535, 7517, 7313, 0, 32110, 25250];
        let mut bytes = fixture_kit();
        let start = bytes.len() - 18;
        for (index, word) in words.iter().enumerate() {
            bytes[start + index * 2..start + index * 2 + 2].copy_from_slice(&word.to_le_bytes());
        }
        let mut kit = parse_kit(&bytes, 0, bytes.len(), Product::D2).unwrap();
        let effects = kit.groove_effects.unwrap();
        assert_eq!(
            effects,
            GrooveEffects {
                filter_on: true,
                filter_cutoff: 24421,
                filter_resonance: 11611,
                echo_on: true,
                echo_time: 7517,
                echo_feedback: 7313,
                distortion_on: false,
                distortion: 32110,
                volume: 25250,
            }
        );
        assert_eq!(effects.volume_gain(), 6312.0 / 8192.0);
        assert_eq!(
            serde_json::to_value(&kit).unwrap()["groove_effects"]["echo_feedback"],
            7313
        );
        assert_eq!(
            effects_notice(&kit).unwrap(),
            "Saved Groove Generator effects are not applied: filter, echo. Drum patterns play without them."
        );

        let mut prepared = prepared_pad(vec![0.5], 1.0, false, 1.0, true, true);
        prepared.volume = effects.volume_gain();
        let pcm = render(&prepared, &pattern(1), 140.0).unwrap();
        assert_eq!(pcm.samples[0], 0.5 * 6312.0 / 8192.0);
        // The volume follows the saturation of the summed pads.
        let mut loud = prepared_pad(vec![3.0], 1.0, false, 1.0, true, true);
        loud.volume = 0.5;
        assert_eq!(render(&loud, &pattern(1), 140.0).unwrap().samples[0], 0.5);

        // An earlier saved kit has the bytes and no names. No switch, no notice.
        kit.groove_effects = None;
        let saved: Kit = serde_json::from_value(serde_json::to_value(&kit).unwrap()).unwrap();
        assert_eq!(saved, kit);
        assert!(effects_notice(&saved).is_none());
        let mut off = fixture_kit();
        let start = off.len() - 18;
        off[start..].fill(0);
        let off = parse_kit(&off, 0, off.len(), Product::D2).unwrap();
        assert!(effects_notice(&off).is_none());
        assert_eq!(off.groove_effects.unwrap().volume_gain(), 0.0);
    }

    #[test]
    fn sixteen_pad_records_retain_paths_settings_and_entire_header() {
        fn bare(bytes: &mut Vec<u8>, text: &str) {
            bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        let mut bytes = vec![99; 7];
        for index in 0..16u16 {
            bytes.extend_from_slice(&(16000 + index).to_le_bytes());
            bare(&mut bytes, &format!("Pad{index}"));
            bytes.extend_from_slice(&(-2i16).to_le_bytes());
            bytes.extend_from_slice(&128u16.to_le_bytes());
            bytes.extend_from_slice(&65535u16.to_le_bytes());
            bytes.extend_from_slice(&26000u16.to_le_bytes());
            bare(&mut bytes, "C:\\Temp\\pad.tmp");
            bare(&mut bytes, "C:\\Temp\\pad.tmp");
            bytes.extend_from_slice(&[index as u8; 10]);
        }
        let source_end = bytes.len();
        bytes.extend_from_slice(&16015u16.to_le_bytes());
        bare(&mut bytes, "Pad15");
        bytes.extend_from_slice(&[3; 38]);
        let kit = parse_kit(&bytes, 7, bytes.len(), Product::D3).unwrap();
        assert_eq!(kit.pads.len(), 16);
        assert_eq!(kit.pads[15].opaque_native_settings, [15; 10]);
        assert_eq!(kit.pads[0].pan_native, Some(128));
        assert!(kit.pads[0].reverse);
        assert_eq!(kit.source_offset, 7);
        assert_eq!(kit.source_end, source_end);
        assert_eq!(kit.header_end, Some(bytes.len()));
        assert_eq!(kit.raw_header, bytes[7..]);
        assert_eq!(kit.opaque_native_settings, bytes[source_end..]);
        let mut invalid = bytes.clone();
        invalid[source_end..source_end + 2].copy_from_slice(&1u16.to_le_bytes());
        assert!(parse_kit(&invalid, 7, invalid.len(), Product::D4).is_err());
        assert!(parse_kit(&bytes[..source_end], 7, source_end, Product::D3).is_err());
    }

    #[test]
    fn sixteen_pad_pattern_last_mask_and_last_step_render() {
        let mut bytes = vec![0; 30];
        bytes.extend_from_slice(&0x8001u16.to_le_bytes());
        wrapped(&mut bytes, "Group");
        wrapped(&mut bytes, "Variant");
        let (pattern, consumed) = parse_pattern(&bytes, 7200, Product::D4).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(pattern.masks.len(), 16);
        let mut prepared = prepared_pad(vec![0.25], 1.0, false, 1.0, true, true);
        let pad = prepared.pads[0].take();
        prepared.pads.resize_with(16, || None);
        prepared.pads[15] = pad;
        let pcm = render(&prepared, &pattern, 140.0).unwrap();
        let onsets: Vec<_> = pcm
            .samples
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, value)| value[0] != 0.0)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(onsets, [0, 70875]);
        let mut invalid = pattern;
        invalid.masks.pop();
        assert!(render(&prepared, &invalid, 140.0).is_err());
    }

    #[test]
    fn sixteenth_masks_place_first_last_and_quarter_notes_in_one_bar() {
        let prepared = prepared_pad(vec![0.25], 1.0, false, 1.0, true, true);
        let pcm = render(&prepared, &pattern(0x1111), 140.0).unwrap();
        assert_eq!(pcm.frames(), 75600);
        let active: Vec<_> = pcm
            .samples
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, sample)| sample[0] != 0.0)
            .map(|(frame, _)| frame)
            .collect();
        assert_eq!(active, [0, 18900, 37800, 56700]);
        let edge = render(&prepared, &pattern(0x8001), 140.0).unwrap();
        let active: Vec<_> = edge
            .samples
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, sample)| sample[0] != 0.0)
            .map(|(frame, _)| frame)
            .collect();
        assert_eq!(active, [0, 70875]);
    }

    #[test]
    fn pad_pitch_reverse_gain_and_routing_apply_to_each_trigger() {
        let prepared = prepared_pad(vec![0.1, 0.2, 0.4, 0.8], 2.0, true, 0.5, true, false);
        let pcm = render(&prepared, &pattern(1), 140.0).unwrap();
        assert_eq!(&pcm.samples[..6], &[0.4, 0.0, 0.1, 0.0, 0.0, 0.0]);
        assert!(render(&prepared, &pattern(1), f64::NAN).is_err());
        let missing = PreparedKit {
            pads: (0..10).map(|_| None).collect(),
            volume: 1.0,
        };
        assert!(render(&missing, &pattern(1), 140.0).is_err());
    }
}
