//! The counted song layout: the mixer header and the timeline of events that
//! follows it, for each product profile.

use super::{
    Reader,
    waves::{d2_embedded_sources, embedded_wave_ranges, read_wave_extension},
};
use crate::{
    compat::{drums, settings},
    engine::{MAX_CLIPS, MAX_TICKS, TRACK_COUNT, Track},
    library::decode_1252,
    products::Product,
};
use std::collections::BTreeMap;

pub(super) const SENTINEL: [u8; 4] = [0xa9, 0x7a, 0x8a, 0x01];

pub(crate) struct Header {
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) bpm: f64,
    pub(crate) effective_bpm: f64,
    pub(crate) tracks: Vec<Track>,
    pub(crate) source_notices: Vec<String>,
    pub(crate) settings: settings::Saved,
    pub(crate) after_mixer: usize,
}

pub(super) fn profile(product: Product) -> Result<(u32, usize, usize), String> {
    match product {
        Product::D2 => Ok((0xa19, 16, 4)),
        Product::T3 => Ok((0x2572, 32, 8)),
        Product::D3 => Ok((0x2571, 32, 8)),
        Product::H1 => Ok((0x11d6, 24, 8)),
        Product::D4 => Ok((0x2d41, 48, 8)),
        _ => Err("This product uses a different native song reader.".into()),
    }
}

pub(super) fn read_header(bytes: &[u8], product: Product) -> Result<Header, String> {
    let (app_id, track_count, track_bytes) = profile(product)?;
    // These native mixer arrays include the generated-audio lane after the
    // ordinary sample lanes. Timeline admission still uses the core count.
    let mixer_count =
        track_count + usize::from(matches!(product, Product::D3 | Product::D4 | Product::T3));
    let mut reader = Reader { bytes, offset: 0 };
    if reader.u32()? != app_id {
        return Err("The native song belongs to a different product or profile.".into());
    }
    reader.u32()?;
    let bpm = f64::from(reader.u16()?);
    let effective_bpm = f64::from(reader.u32()?);
    if bpm <= 0.0 || effective_bpm <= 0.0 {
        return Err("The native song has invalid tempo data.".into());
    }
    if mixer_count > TRACK_COUNT {
        return Err("The native song declares more than 64 tracks.".into());
    }
    let author = first_text(reader.block()?);
    let title_block = reader.block()?;
    let title = first_text(title_block);
    let settings = settings::parse(title_block);
    let mut tracks = vec![Track::default(); TRACK_COUNT];
    let mut native_switches = Vec::new();
    let mut source_notices = Vec::new();
    for track in &mut tracks[..mixer_count] {
        let control = reader.u16()?;
        let gain = reader.u16()?;
        let signed_gain = match product {
            Product::D2 => native_fader_gain(gain, 1.25),
            _ => native_fader_gain(gain, 1.33),
        };
        track.set_legacy_gain(signed_gain);
        if track_bytes == 4 {
            if control > 3 {
                return Err("The native track has invalid output routing.".into());
            }
            track.left_enabled = matches!(control, 0 | 1);
            track.right_enabled = matches!(control, 0 | 2);
        } else {
            track.pan = decode_pan(control)?;
            track.left_enabled = track.pan < 1.0;
            track.right_enabled = track.pan > -1.0;
            native_switches.push((reader.u16()?, reader.u16()?));
        }
    }
    // The native dispatcher first excludes non-solo lanes when any saved solo
    // switch is 1, then excludes mute=1 lanes. A muted solo lane stays silent,
    // which is the track Mute and Solo rule. Another value has no equivalent: a
    // solo of 2 keeps a lane audible without starting solo playback. Such a
    // song keeps the earlier conversion of the switches to L/R outputs.
    let exact = native_switches
        .iter()
        .all(|&(mute, solo)| mute <= 1 && solo <= 1);
    let any_solo = native_switches.iter().any(|(_, solo)| *solo == 1);
    for (index, (mute, solo)) in native_switches.into_iter().enumerate() {
        let track = &mut tracks[index];
        if exact {
            track.muted = mute == 1;
            track.solo = solo == 1;
        } else {
            let audible = mute != 1 && (!any_solo || solo != 0);
            track.left_enabled &= audible;
            track.right_enabled &= audible;
        }
        if mute != 0 || solo != 0 {
            source_notices.push(if exact {
                format!("Native track {}: mute={mute}, solo={solo}.", index + 1)
            } else {
                format!(
                    "Native track {}: mute={mute}, solo={solo}. A saved value other than 0 or 1 \
                     has no Mute or Solo equivalent, so this song's mute and solo settings were \
                     applied to the L/R outputs.",
                    index + 1
                )
            });
        }
    }
    Ok(Header {
        title,
        author,
        bpm,
        effective_bpm,
        tracks,
        source_notices,
        settings,
        after_mixer: reader.offset,
    })
}

pub(super) fn first_text(bytes: &[u8]) -> String {
    decode_1252(bytes.split(|byte| *byte == 0).next().unwrap_or_default())
        .trim_end_matches('\u{1}')
        .to_string()
}

// AMitte in the original D3, D4, T3, and House engines snaps the end and
// center regions, then uses Q8 balance factors with denominator 128 on both sides.
pub(super) fn decode_pan(value: u16) -> Result<f32, String> {
    if value > 255 {
        return Err("The native track pan exceeds its supported range.".into());
    }
    Ok(match value {
        0..=14 => -1.0,
        117..=139 => 0.0,
        241..=255 => 1.0,
        _ => (f32::from(value) - 128.0) / 128.0,
    })
}

#[derive(Debug, PartialEq)]
pub(crate) enum Reference {
    Native(u16),
    /// A D2 Sample Kit sound (kit archives 4-6), by its D2 index ID.
    Kit(u16),
    Wave(WaveReference),
    Drum(drums::Pattern),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct WaveReference {
    pub(crate) path: String,
    pub(crate) declared_ticks: Option<u32>,
    pub(crate) metadata: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Event {
    pub(crate) track: usize,
    pub(crate) position: u32,
    pub(crate) left_trim: u32,
    pub(crate) right_trim: u32,
    pub(crate) gain: f32,
    pub(crate) reference: Reference,
}

pub(crate) struct Timeline {
    pub(crate) start: usize,
    pub(crate) events: Vec<Event>,
}

pub(super) fn read_timeline(
    bytes: &[u8],
    lower_bound: usize,
    product: Product,
) -> Result<Timeline, String> {
    let last_marker = bytes
        .windows(4)
        .rposition(|value| value == SENTINEL)
        .filter(|offset| *offset >= lower_bound)
        .ok_or("The native song has no timeline end marker.")?;
    let mut result = None;
    let mut last_error = None;
    let mut payload_ranges: Vec<_> = if product == Product::D2 {
        Vec::new()
    } else {
        embedded_wave_ranges(bytes)?
    };
    let mut payload_index = 0;
    // Audio payloads can contain the marker bytes. Match a complete counted
    // timeline to its own length and marker, not the last marker in the file.
    // Each rejected candidate can cost a full timeline parse, so a file made
    // of candidates must not be searched without limit.
    const MAX_REJECTED_CANDIDATES: usize = 4096;
    let mut rejected = 0;
    let mut cursor = lower_bound;
    while cursor < last_marker.saturating_sub(4) {
        while payload_index < payload_ranges.len() && payload_ranges[payload_index].end <= cursor {
            payload_index += 1;
        }
        if let Some(range) = payload_ranges.get(payload_index)
            && range.contains(&cursor)
        {
            cursor = range.end;
            continue;
        }
        let offset = cursor;
        cursor += 1;
        let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let Some(marker) = offset.checked_add(length).filter(|end| *end <= last_marker) else {
            continue;
        };
        if length < 9
            || bytes.get(marker..marker + 4) != Some(&SENTINEL)
            || bytes.get(offset + 4) != Some(&1)
        {
            continue;
        }
        match read_chunk(&bytes[offset + 4..marker], product) {
            Ok(events) => {
                if product == Product::D2 {
                    match d2_embedded_sources(bytes, offset) {
                        Ok(sources) => {
                            payload_ranges.extend(sources.into_iter().map(|source| source.range))
                        }
                        Err(error) => {
                            last_error = Some(error);
                            continue;
                        }
                    }
                }
                if result.is_some() {
                    return Err("The native song has ambiguous timeline data.".into());
                }
                result = Some(Timeline {
                    start: offset,
                    events,
                });
            }
            Err(error) => {
                last_error = Some(error);
                rejected += 1;
                if rejected > MAX_REJECTED_CANDIDATES {
                    return Err("The native song has too many invalid timeline records.".into());
                }
            }
        }
    }
    result.ok_or_else(|| {
        last_error.unwrap_or_else(|| "The native counted timeline profile is not supported.".into())
    })
}

pub(super) fn read_chunk(bytes: &[u8], product: Product) -> Result<Vec<Event>, String> {
    let core_tracks = profile(product)?.1;
    let maximum_lane = core_tracks
        + usize::from(matches!(
            product,
            Product::D2 | Product::D3 | Product::D4 | Product::H1
        ));
    let mut reader = Reader { bytes, offset: 0 };
    let mut events = Vec::new();
    let mut previous_lane = 0;
    while reader.offset < bytes.len() {
        if reader.offset + 1 == bytes.len() && bytes[reader.offset] == 1 {
            reader.offset += 1;
            break;
        }
        let lane = usize::from(reader.u8()?);
        let count = reader.u32()? as usize;
        if lane <= previous_lane
            || lane > maximum_lane
            || count > MAX_CLIPS
            || count > bytes.len().saturating_sub(reader.offset) / 7
        {
            return Err("The native timeline has an invalid lane or event count.".into());
        }
        previous_lane = lane;
        if lane > TRACK_COUNT && count > 0 {
            return Err("The native song uses more than 64 tracks.".into());
        }
        for _ in 0..count {
            if events.len() >= MAX_CLIPS {
                return Err("The native song has too many events.".into());
            }
            let class = reader.u8()?;
            let position = reader.i32()?;
            let sample = reader.u16()?;
            if class == 2
                && matches!(product, Product::D2 | Product::D3 | Product::D4)
                && lane == core_tracks + 1
            {
                if position >= 0 {
                    return Err("The native drum pattern uses unsupported trim encoding.".into());
                }
                let (pattern, consumed) =
                    drums::parse_pattern(&reader.bytes[reader.offset..], sample, product)?;
                reader.take(consumed)?;
                events.push(Event {
                    track: lane - 1,
                    position: native_position(position)?,
                    left_trim: 0,
                    right_trim: 0,
                    gain: 1.0,
                    reference: Reference::Drum(pattern),
                });
                continue;
            }
            let mut event = event_tuple(lane - 1, class, position, sample, &mut reader, product)?;
            if class == 1 {
                let mut reference = read_wave_extension(&mut reader, lane, product)?;
                reference
                    .metadata
                    .insert("Saved sample ID".into(), sample.to_string());
                event.reference = Reference::Wave(reference);
            }
            events.push(event);
        }
    }
    if previous_lane == 0 || reader.offset != bytes.len() {
        return Err("The native timeline is incomplete.".into());
    }
    Ok(events)
}

fn event_tuple(
    track: usize,
    class: u8,
    position: i32,
    sample: u16,
    reader: &mut Reader<'_>,
    product: Product,
) -> Result<Event, String> {
    if class == 2 || class == 6 {
        return Err(format!(
            "{} track {} contains an unsupported generated pattern (source {}).",
            product.label(),
            track + 1,
            class
        ));
    }
    if class != 0
        && class != 3
        && !(class == 1
            && matches!(
                product,
                Product::D2 | Product::D3 | Product::D4 | Product::T3 | Product::H1
            ))
    {
        return Err(format!(
            "{} track {} uses unsupported source {}.",
            product.label(),
            track + 1,
            class
        ));
    }
    let native_position = native_position(position)?;
    let (left_trim, right_trim, gain) = if position >= 0 {
        let left = reader.u32()?;
        let right = reader.u32()?;
        let gain = (32767.0 + f32::from(reader.i16()?)) / 32767.0;
        if !(0.0..=2.0).contains(&gain) {
            return Err("The native clip has invalid volume data.".into());
        }
        (native_tick_count(left)?, native_tick_count(right)?, gain)
    } else {
        (0, 0, 1.0)
    };
    Ok(Event {
        track,
        position: native_position,
        left_trim,
        right_trim,
        gain,
        reference: Reference::Native(sample),
    })
}

// Native RWaveParam faders use signed input and truncate to signed Q16 gain.
pub(in crate::compat) fn native_fader_gain(stored: u16, scale: f64) -> f32 {
    let value = f64::from(stored as i16) / 32767.0 * scale;
    let value = if value > 1.0 {
        value * value * value
    } else {
        value
    };
    ((value * 65536.0).trunc() / 65536.0) as f32
}

pub(super) fn native_position(position: i32) -> Result<u32, String> {
    let position = if position < 0 {
        -i64::from(position) - 1
    } else {
        i64::from(position)
    };
    native_tick_count(
        u32::try_from(position).map_err(|_| "The native position exceeds the timeline.")?,
    )
}

pub(super) fn native_tick_count(value: u32) -> Result<u32, String> {
    // All admitted counted profiles have native duration writers with 128 units per bar.
    value
        .checked_mul(2)
        .filter(|value| *value <= MAX_TICKS)
        .ok_or_else(|| "The native position or trim exceeds the timeline.".into())
}
