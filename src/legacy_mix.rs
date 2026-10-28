//! Reader for the 0x889 project profile found in factory and custom songs.
//! Unknown preamble data is kept outside the audio arrangement decoder. A match
//! must contain all sixteen numbered, counted track lists and the exact trailer.
use crate::{
    compat::chunks::{self, Event, Header, Reference, Timeline, WaveReference},
    engine::{MAX_CLIPS, MAX_TICKS, Project, TICKS_PER_BAR, Track},
    library::{Library, decode_1252},
    products::Product,
};
use std::path::Path;

const TRAILER: &[u8] = &[17, 0, 0, 0, 0, 0xa9, 0x7a, 0x8a, 1];

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    load_named_bytes(
        path,
        &crate::compat::song_file::read_song_file(path)?,
        library,
    )
}

#[cfg(test)]
pub(crate) fn load_bytes(bytes: &[u8], library: &Library) -> Result<Project, String> {
    load_named_bytes(Path::new("original.mix"), bytes, library)
}

fn load_named_bytes(path: &Path, bytes: &[u8], library: &Library) -> Result<Project, String> {
    let mut reader = Reader { bytes, offset: 0 };
    if reader.u32()? != 0x889 {
        return Err("This original mix version is not supported.".into());
    }
    let _unknown_header = reader.u32()?;
    let source_bpm = reader.u16()?;
    // Original writer 0x508add stores four-byte effective BPM from 0x526748.
    // Pitch handler 0x46cf7e derives that value from output rate and source BPM.
    let playback_bpm = reader.u32()?;
    if source_bpm != 140 || !(30..=300).contains(&playback_bpm) {
        return Err("The original mix has unsupported tempo data.".into());
    }
    let author = reader
        .string()?
        .split('\0')
        .next()
        .unwrap_or_default()
        .to_string();
    let title = reader.string()?;
    let mut tracks = Vec::with_capacity(16);
    for _ in 0..16 {
        let routing = reader.u16()?;
        let volume = reader.u16()?;
        if routing > 3 || volume > 32767 {
            return Err("The original mix has unsupported mixer data.".into());
        }
        tracks.push(Track {
            gain: volume as f32 / 32767.0,
            polarity_inverted: false,
            pan: 0.0,
            left_enabled: matches!(routing, 0 | 1),
            right_enabled: matches!(routing, 0 | 2),
            ..Track::default()
        });
    }
    // The installed profile has ten further bytes, followed by a product name,
    // sample-kit declarations and drum instrument settings. Those controls are
    // not yet reconstructed. The track lists are independently self-delimiting.
    reader.take(10)?;
    let product = reader.string()?;
    // Custom 0x889 files also save the earlier label or an empty label. Admit
    // only those observed labels, then require the entire counted arrangement
    // and exact trailer before resolving any sample or writing imported audio.
    if !matches!(
        product.trim(),
        "\x54echno \x65Jay 2" | "\x54echno \x65Jay" | ""
    ) {
        return Err("The mix belongs to a different product.".into());
    }
    let search_start = reader.offset;
    let search_end = bytes
        .len()
        .saturating_sub(16 * 5 + TRAILER.len())
        .min(search_start + 4096);
    let mut arrangement = None;
    let payload_ranges = chunks::embedded_wave_ranges(bytes)?;
    let mut payload_index = 0;
    // Preamble bytes can resemble a track list. Resolve audio only after a
    // unique complete structural parse, independently of available samples.
    for start in search_start..=search_end {
        while payload_index < payload_ranges.len() && payload_ranges[payload_index].end <= start {
            payload_index += 1;
        }
        if payload_ranges
            .get(payload_index)
            .is_some_and(|range| range.contains(&start))
        {
            continue;
        }
        if bytes.get(start) != Some(&1) {
            continue;
        }
        if let Ok(events) = parse_arrangement(bytes, start) {
            if arrangement.is_some() {
                return Err("The original mix has ambiguous arrangement data.".into());
            }
            arrangement = Some(Timeline { start, events });
        }
    }
    let timeline = arrangement.ok_or(
        "Cannot decode the original mix arrangement. The file is damaged or uses an unsupported \
         profile.",
    )?;
    let header = Header {
        title,
        author,
        bpm: source_bpm as f64,
        effective_bpm: playback_bpm as f64,
        tracks,
        source_notices: Vec::new(),
        settings: Default::default(),
        after_mixer: search_start,
    };
    chunks::build_project(path, bytes, Product::T2, library, header, timeline)
}

fn parse_arrangement(bytes: &[u8], start: usize) -> Result<Vec<Event>, String> {
    let mut reader = Reader {
        bytes,
        offset: start,
    };
    let mut events = Vec::new();
    for track in 0..16 {
        if reader.u8()? as usize != track + 1 {
            return Err("Invalid track marker.".into());
        }
        let count = reader.u32()? as usize;
        if count > MAX_CLIPS - events.len()
            || count > reader.bytes.len().saturating_sub(reader.offset) / 7
        {
            return Err("Invalid clip count.".into());
        }
        for _ in 0..count {
            // Source 0 is a product sound, 1 an imported WAV, and 3 a Sample
            // Kit sound. Releases before the Yelsi editions saved kit sounds
            // by their Dance eJay 2 index ID (kit archives 4-6).
            let source = reader.u8()?;
            if !matches!(source, 0 | 1 | 3) {
                return Err("Unsupported clip source.".into());
            }
            let stored_position = reader.i32()?;
            let sample_id = reader.u16()?;
            let native_position = if stored_position < 0 {
                !stored_position as u32
            } else {
                stored_position as u32
            };
            // Native duration writer 0x4fd1d5 divides stored positions by 128.
            // Common projects use 256 ticks per bar, so convert exactly once.
            let start_tick = native_position
                .checked_mul(2)
                .filter(|value| *value <= MAX_TICKS)
                .ok_or("Invalid native clip position.")?;
            let (left_trim, right_trim, gain_delta) = if stored_position < 0 {
                (0, 0, 0)
            } else {
                (reader.u32()?, reader.u32()?, reader.i16()?)
            };
            let left_trim = left_trim
                .checked_mul(2)
                .filter(|value| *value <= MAX_TICKS)
                .ok_or("Invalid native clip trim.")?;
            let right_trim = right_trim
                .checked_mul(2)
                .filter(|value| *value <= MAX_TICKS)
                .ok_or("Invalid native clip trim.")?;
            let reference = match source {
                1 => {
                    let mut reference = read_wave_extension(&mut reader)?;
                    reference
                        .metadata
                        .insert("Saved sample ID".into(), sample_id.to_string());
                    Reference::Wave(reference)
                }
                3 => Reference::Kit(sample_id),
                _ => Reference::Native(sample_id),
            };
            events.push(Event {
                reference,
                track,
                position: start_tick,
                left_trim,
                right_trim,
                gain: (32767i32 + gain_delta as i32) as f32 / 32767.0,
            });
        }
    }
    if reader.take(TRAILER.len())? != TRAILER {
        return Err("Invalid mix trailer.".into());
    }
    if reader.offset != reader.bytes.len() {
        read_wave_appendix(&mut reader)?;
    }
    Ok(events)
}

/// Releases before the Yelsi editions append the song's imported WAV files
/// after the trailer, with a count even when it is zero. Each record is the
/// saved path, the WAV size and the whole RIFF file; the block ends the file.
/// The shared embedded-WAV scan resolves the audio, so this only checks that
/// the block is complete.
fn read_wave_appendix(reader: &mut Reader<'_>) -> Result<(), String> {
    let count = reader.u32()? as usize;
    if count > reader.bytes.len().saturating_sub(reader.offset) / 52 {
        return Err("Invalid embedded WAV count.".into());
    }
    for _ in 0..count {
        if !is_saved_wave_path(&reader.string()?) {
            return Err("Invalid embedded WAV path.".into());
        }
        let size = reader.u32()? as usize;
        let wav = reader.take(size)?;
        if size < 44
            || &wav[..4] != b"RIFF"
            || &wav[8..12] != b"WAVE"
            || u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize + 8 != size
        {
            return Err("Invalid embedded WAV.".into());
        }
    }
    if reader.offset != reader.bytes.len() {
        return Err("Invalid data after the embedded WAV files.".into());
    }
    Ok(())
}

/// The form in which the original saves a WAV path: a drive path that ends in
/// .wav and fits its 260-byte field.
fn is_saved_wave_path(path: &str) -> bool {
    path.len() <= 260
        && path.as_bytes().get(1) == Some(&b':')
        && path.to_ascii_lowercase().ends_with(".wav")
        && !path.contains('\0')
}

fn read_wave_extension(reader: &mut Reader<'_>) -> Result<WaveReference, String> {
    let mut metadata = std::collections::BTreeMap::new();
    // Original writer 0x50A5B5, 0x50A602 and 0x50A650 writes three strings.
    // 0x50A6A0..0x50A741 then writes u16/u32/u16 metadata. The middle
    // value is the nominal bar count: 0x42FDDE..0x42FDE1 multiplies it by 128.
    let path = reader.string()?;
    if !is_saved_wave_path(&path) {
        return Err("The T2 WAV reference is invalid.".into());
    }
    for index in 0..2 {
        let label = reader.string()?;
        if label.len() > 260 {
            return Err("The T2 WAV label is too long.".into());
        }
        metadata.insert(format!("Native label {}", index + 1), label);
    }
    metadata.insert("Native metadata word 1".into(), reader.u16()?.to_string());
    let bars = reader.u32()?;
    metadata.insert("Declared bars".into(), bars.to_string());
    let declared_ticks = bars
        .checked_mul(TICKS_PER_BAR)
        .filter(|ticks| (1..=MAX_TICKS).contains(ticks))
        .ok_or("The T2 WAV span exceeds the supported timeline.")?;
    metadata.insert("Native metadata word 2".into(), reader.u16()?.to_string());
    Ok(WaveReference {
        path,
        declared_ticks: Some(declared_ticks),
        metadata,
    })
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
            .ok_or("Invalid mix length.")?;
        let data = self
            .bytes
            .get(self.offset..end)
            .ok_or("The original mix is truncated.")?;
        self.offset = end;
        Ok(data)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, String> {
        let size = self.u16()? as usize;
        let bytes = self.take(size)?;
        let text = bytes
            .strip_suffix(&[0, 1])
            .ok_or("The mix has an unsupported text field.")?;
        Ok(decode_1252(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// A temporary folder that is removed even when the test fails.
    struct Folder(PathBuf);
    impl Folder {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn text(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&((value.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
        bytes.extend_from_slice(&[0, 1]);
    }

    fn custom_song(product: &str, records: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x889u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&140u16.to_le_bytes());
        bytes.extend_from_slice(&140u32.to_le_bytes());
        text(&mut bytes, "Composer");
        text(&mut bytes, "Custom song");
        for _ in 0..16 {
            bytes.extend_from_slice(&0u16.to_le_bytes());
            bytes.extend_from_slice(&32767u16.to_le_bytes());
        }
        bytes.extend_from_slice(&[0; 10]);
        text(&mut bytes, product);
        for track in 1..=16 {
            bytes.push(track);
            let count = if track == 1 { records.len() as u32 } else { 0 };
            bytes.extend_from_slice(&count.to_le_bytes());
            if track == 1 {
                for record in records {
                    bytes.extend_from_slice(record);
                }
            }
        }
        bytes.extend_from_slice(TRAILER);
        bytes
    }

    fn wave_record(path: &str, position: i32, bars: u32) -> Vec<u8> {
        let mut bytes = vec![1];
        bytes.extend_from_slice(&position.to_le_bytes());
        bytes.extend_from_slice(&10200u16.to_le_bytes());
        if position >= 0 {
            bytes.extend_from_slice(&128u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&0i16.to_le_bytes());
        }
        text(&mut bytes, path);
        text(&mut bytes, "");
        text(&mut bytes, "");
        bytes.extend_from_slice(&10u16.to_le_bytes());
        bytes.extend_from_slice(&bars.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes
    }

    #[test]
    fn observed_product_labels_require_a_complete_supported_arrangement() {
        for label in ["Techno eJay 2 ", "Techno eJay ", ""] {
            let bytes = custom_song(label, &[]);
            assert!(load_bytes(&bytes, &Library::empty()).is_ok(), "{label:?}");
            let mut invalid = bytes.clone();
            *invalid.last_mut().unwrap() = 0;
            assert!(load_bytes(&invalid, &Library::empty()).is_err());
            let mut unknown_source = vec![2];
            unknown_source.extend_from_slice(&(-1i32).to_le_bytes());
            unknown_source.extend_from_slice(&174u16.to_le_bytes());
            assert!(load_bytes(&custom_song(label, &[unknown_source]), &Library::empty()).is_err());
        }
        for label in ["Dance eJay 2", "Another product", "Techno eJay 3"] {
            assert_eq!(
                load_bytes(&custom_song(label, &[]), &Library::empty()).unwrap_err(),
                "The mix belongs to a different product."
            );
        }
    }

    #[test]
    fn framed_wave_records_preserve_nominal_span_trim_and_source_identity() {
        let folder = Folder::new("t2-custom-wave");
        let root = folder.0.as_path();
        let path = root.join("custom-take.wav");
        let mut samples = vec![0; 37_800];
        samples.extend(vec![8192; 37_800]);
        fs::write(
            &path,
            legacy_format::wav::PcmWave {
                sample_rate: 22_050,
                channels: 1,
                samples,
            }
            .encode()
            .unwrap(),
        )
        .unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(root, 140.0).unwrap();
        let records = [
            wave_record(r"C:\old\custom-take.wav", 0, 2),
            wave_record(r"C:\old\custom-take.wav", -129, 3),
        ];
        let bytes = custom_song("", &records);
        let project = load_bytes(&bytes, &library).unwrap();
        assert_eq!(project.imported_samples.len(), 1);
        assert_eq!(project.imported_samples[0].origin, Some(Product::T2));
        assert_eq!(project.clips[0].sample_id, project.clips[1].sample_id);
        assert_eq!(project.clips[0].offset_frames, 37_800);
        assert_eq!(project.clips[0].length_ticks, 256);
        assert_eq!(project.clips[1].start_tick, 256);
        assert_eq!(project.clips[1].length_ticks, 768);
        let saved = root.join("saved.json");
        project.save(&saved).unwrap();
        fs::remove_file(&path).unwrap();
        let (reopened, restored_library) =
            crate::project_io::open(&saved, &Library::empty()).unwrap();
        assert_eq!(reopened, project);
        let pcm = crate::audio::render_project(&reopened, &restored_library).unwrap();
        assert!(
            pcm.samples[..75_600 * 2]
                .iter()
                .all(|value| (*value - 0.25).abs() < 0.0001)
        );
        // The second record has two bars of PCM in a nominal three-bar tile.
        assert!(
            pcm.samples[3 * 75_600 * 2..]
                .iter()
                .all(|value| *value == 0.0)
        );
    }

    fn wave_appendix(waves: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = (waves.len() as u32).to_le_bytes().to_vec();
        for (path, wav) in waves {
            text(&mut bytes, path);
            bytes.extend_from_slice(&(wav.len() as u32).to_le_bytes());
            bytes.extend_from_slice(wav);
        }
        bytes
    }

    #[test]
    fn embedded_pcm_cannot_supply_an_arrangement_candidate() {
        // These are valid PCM16 samples and also a complete empty arrangement.
        // The leading byte keeps the PCM byte count even.
        let mut pcm = vec![0];
        for track in 1..=16 {
            pcm.push(track);
            pcm.extend_from_slice(&0u32.to_le_bytes());
        }
        pcm.extend_from_slice(TRAILER);
        let wav = legacy_format::wav::PcmWave {
            sample_rate: 44_100,
            channels: 1,
            samples: pcm
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                .collect(),
        }
        .encode()
        .unwrap();
        let path = r"C:\absent\timeline-audio.wav";
        let arrangement_start = custom_song("Techno eJay 2 ", &[]).len() - 16 * 5 - TRAILER.len();
        let mut bytes = custom_song("Techno eJay 2 ", &[wave_record(path, -1, 1)]);
        bytes.extend(wave_appendix(&[(path, &wav)]));
        let project = load_bytes(&bytes, &Library::empty()).unwrap();
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.imported_samples.len(), 1);
        assert!(project.missing_samples.is_empty());

        // The same audio cannot hide damage to the real arrangement.
        bytes[arrangement_start] = 99;
        assert!(load_bytes(&bytes, &Library::empty()).is_err());
    }

    #[test]
    fn early_release_wave_appendix_supplies_the_imported_audio() {
        let wav = legacy_format::wav::PcmWave {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![8192; 75_600],
        }
        .encode()
        .unwrap();
        let path = r"c:\ejay\MIXWAVES\absent-take.wav";
        let mut bytes = custom_song("Techno eJay 2 ", &[wave_record(path, -1, 1)]);
        bytes.extend(wave_appendix(&[(path, &wav)]));
        let project = load_bytes(&bytes, &Library::empty()).unwrap();
        assert!(project.missing_samples.is_empty());
        assert_eq!(project.imported_samples.len(), 1);
        assert_eq!(project.clips.len(), 1);

        let mut empty = custom_song("Techno eJay 2 ", &[]);
        empty.extend(wave_appendix(&[]));
        assert!(
            load_bytes(&empty, &Library::empty())
                .unwrap()
                .clips
                .is_empty()
        );

        let mut extra = bytes.clone();
        extra.push(0);
        let mut wrong_count = custom_song("Techno eJay 2 ", &[wave_record(path, -1, 1)]);
        wrong_count.extend(wave_appendix(&[(path, &wav)]));
        let count_at = wrong_count.len() - wav.len() - 4 - (path.len() + 4) - 4;
        wrong_count[count_at] = 2;
        let mut not_wave = custom_song("Techno eJay 2 ", &[]);
        not_wave.extend(wave_appendix(&[(path, &[0; 64])]));
        let mut invalid_path = custom_song("Techno eJay 2 ", &[wave_record(path, -1, 1)]);
        invalid_path.extend(wave_appendix(&[("c:\\ejay\\\0absent-take.wav", &wav)]));
        for invalid in [
            &bytes[..bytes.len() - 1],
            &extra,
            &wrong_count,
            &not_wave,
            &invalid_path,
        ] {
            let error = load_bytes(invalid, &Library::empty()).unwrap_err();
            assert!(
                error.contains("Cannot decode the original mix arrangement"),
                "{error}"
            );
        }
    }

    #[test]
    fn sample_kit_clips_resolve_by_their_dance_ejay_2_index_id() {
        let record = |source: u8| {
            let mut bytes = vec![source];
            bytes.extend_from_slice(&(-1i32).to_le_bytes());
            bytes.extend_from_slice(&6295u16.to_le_bytes());
            bytes
        };
        let library = Library::empty();
        let kit = load_bytes(&custom_song("Techno eJay 2 ", &[record(3)]), &library).unwrap();
        assert_eq!(
            kit.clips[0].sample_id,
            Product::D2.global_sample_id(6295).unwrap()
        );
        assert_eq!(
            kit.missing_samples[0].reason,
            library.describe_missing(Product::D2, 6295)
        );
        // The same number as a product sound is not a kit sound.
        let native = load_bytes(&custom_song("Techno eJay 2 ", &[record(0)]), &library).unwrap();
        assert_eq!(
            native.clips[0].sample_id,
            Product::T2.global_sample_id(6295).unwrap()
        );
    }

    #[test]
    fn missing_wave_diagnostic_requires_the_complete_arrangement() {
        let bytes = custom_song(
            "Techno eJay ",
            &[wave_record(r"C:\absent\missing-t2-take.wav", -1, 2)],
        );
        let project = load_bytes(&bytes, &Library::empty()).unwrap();
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.clips[0].length_ticks, 512);
        assert_eq!(project.missing_samples.len(), 1);
        assert!(
            project.missing_samples[0]
                .reason
                .contains("missing-t2-take.wav")
        );
        assert_eq!(project.missing_samples[0].metadata["Declared bars"], "2");
        for end in [bytes.len() - 1, bytes.len() - TRAILER.len()] {
            let error = load_bytes(&bytes[..end], &Library::empty()).unwrap_err();
            assert!(
                error.contains("Cannot decode the original mix arrangement"),
                "{error}"
            );
        }
        let record = wave_record(r"C:\absent\missing-t2-take.wav", -1, 0);
        let error = load_bytes(&custom_song("", &[record]), &Library::empty()).unwrap_err();
        assert!(
            error.contains("Cannot decode the original mix arrangement"),
            "{error}"
        );
    }

    fn song_with_missing_sample(preamble: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x889u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&140u16.to_le_bytes());
        bytes.extend_from_slice(&140u32.to_le_bytes());
        text(&mut bytes, "Composer");
        text(&mut bytes, "Missing sample");
        bytes.extend_from_slice(&[0; 74]);
        text(&mut bytes, "\x54echno \x65Jay 2");
        bytes.extend_from_slice(preamble);
        for track in 1..=16 {
            bytes.push(track);
            bytes.extend_from_slice(&u32::from(track == 1).to_le_bytes());
            if track == 1 {
                bytes.push(0);
                bytes.extend_from_slice(&(-129i32).to_le_bytes());
                bytes.extend_from_slice(&174u16.to_le_bytes());
            }
        }
        bytes.extend_from_slice(TRAILER);
        bytes
    }

    #[test]
    fn missing_sample_diagnostic_ignores_partial_tracks_in_the_preamble() {
        let mut preamble = vec![1];
        preamble.extend_from_slice(&1u32.to_le_bytes());
        preamble.push(0);
        preamble.extend_from_slice(&(-129i32).to_le_bytes());
        preamble.extend_from_slice(&9999u16.to_le_bytes());
        let library = Library::empty();
        let expected = library.describe_missing(Product::D2, 174);
        for prefix in [&[][..], preamble.as_slice()] {
            let project = load_bytes(&song_with_missing_sample(prefix), &library).unwrap();
            assert_eq!(project.missing_samples.len(), 1);
            assert_eq!(project.missing_samples[0].reason, expected);
            assert_eq!(
                project.clips[0].sample_id,
                Product::D2.global_sample_id(174).unwrap()
            );
        }
    }

    #[test]
    fn missing_sample_diagnostic_requires_the_exact_trailer() {
        let mut bytes = song_with_missing_sample(&[]);
        *bytes.last_mut().unwrap() = 0;
        let error = load_bytes(&bytes, &Library::empty()).unwrap_err();
        assert!(
            error.contains("Cannot decode the original mix arrangement"),
            "{error}"
        );
    }

    #[test]
    fn t2_native_ids_import_from_a_wav_only_provider() {
        let folder = Folder::new("t2-wav");
        let root = folder.0.as_path();
        let path = root.join("sample.wav");
        fs::write(
            &path,
            legacy_format::wav::PcmWave {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![1000; 4],
            }
            .encode()
            .unwrap(),
        )
        .unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(root, 140.0).unwrap();
        library
            .register_wav_product(
                Product::T2,
                root,
                &[crate::catalog::WaveMetadata {
                    native_id: 174,
                    code: "sample".into(),
                    name: "Sample".into(),
                    group_name: "Test".into(),
                    bars: 2,
                    source_bpm: 140.0,
                    frame_count: 4,
                    pcm_digest: None,
                }],
            )
            .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x889u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&140u16.to_le_bytes());
        bytes.extend_from_slice(&140u32.to_le_bytes());
        text(&mut bytes, "Composer");
        text(&mut bytes, "Standalone song");
        bytes.extend_from_slice(&[0; 64]);
        bytes.extend_from_slice(&[0; 10]);
        text(&mut bytes, "\x54echno \x65Jay 2");
        for track in 1..=16 {
            bytes.push(track);
            bytes.extend_from_slice(&u32::from(track == 1).to_le_bytes());
            if track == 1 {
                bytes.push(0);
                bytes.extend_from_slice(&(-129i32).to_le_bytes());
                bytes.extend_from_slice(&174u16.to_le_bytes());
            }
        }
        bytes.extend_from_slice(TRAILER);
        let project = load_bytes(&bytes, &library).unwrap();
        assert_eq!(project.clips[0].sample_id, 174);
        assert_eq!(project.clips[0].start_tick, 256);
        assert_eq!(project.clips[0].length_ticks, 512);
    }

    #[test]
    fn trimmed_wav_sources_render_the_same_audio_at_different_sample_rates() {
        let folder = Folder::new("t2-trim");
        let root = folder.0.as_path();
        let path = root.join("sample.wav");
        for sample_rate in [22_050, 44_100, 48_000] {
            let bar_frames = (f64::from(sample_rate) * 240.0 / 140.0).round() as usize;
            let mut samples = vec![0; bar_frames];
            samples.extend(vec![8192; bar_frames]);
            fs::write(
                &path,
                legacy_format::wav::PcmWave {
                    sample_rate,
                    channels: 1,
                    samples,
                }
                .encode()
                .unwrap(),
            )
            .unwrap();
            let mut library = Library::empty();
            library.import_wav_folder(root, 140.0).unwrap();
            assert_eq!(
                library
                    .register_wav_product(
                        Product::T2,
                        root,
                        &[crate::catalog::WaveMetadata {
                            native_id: 174,
                            code: "sample".into(),
                            name: "Sample".into(),
                            group_name: "Test".into(),
                            bars: 2,
                            source_bpm: 140.0,
                            frame_count: 151_200,
                            pcm_digest: None,
                        }],
                    )
                    .unwrap(),
                1
            );
            let mut bytes = Vec::new();
            for track in 1..=16 {
                bytes.push(track);
                bytes.extend_from_slice(&u32::from(track == 1).to_le_bytes());
                if track == 1 {
                    bytes.push(0);
                    bytes.extend_from_slice(&0i32.to_le_bytes());
                    bytes.extend_from_slice(&174u16.to_le_bytes());
                    bytes.extend_from_slice(&128u32.to_le_bytes());
                    bytes.extend_from_slice(&0u32.to_le_bytes());
                    bytes.extend_from_slice(&0i16.to_le_bytes());
                }
            }
            bytes.extend_from_slice(TRAILER);
            let events = parse_arrangement(&bytes, 0).unwrap();
            let project = chunks::build_project(
                Path::new("trim.mix"),
                &bytes,
                Product::T2,
                &library,
                Header {
                    title: "Trim".into(),
                    author: String::new(),
                    bpm: 140.0,
                    effective_bpm: 140.0,
                    tracks: vec![Track::default(); 16],
                    source_notices: Vec::new(),
                    settings: Default::default(),
                    after_mixer: 0,
                },
                Timeline { start: 0, events },
            )
            .unwrap();
            assert_eq!(project.clips[0].offset_frames as usize, bar_frames);
            assert_eq!(project.clips[0].length_ticks, TICKS_PER_BAR);
            let pcm = crate::audio::render_project(&project, &library).unwrap();
            assert_eq!(pcm.frames(), 75_600);
            // The first bar is silent; every frame must come from the second bar.
            assert!(
                pcm.samples
                    .iter()
                    .all(|sample| (*sample - 0.25).abs() < 0.0001)
            );
        }
    }

    #[test]
    fn missing_time_stretcher_copies_are_rebuilt_from_the_original_sound() {
        let folder = Folder::new("t2-stretch");
        let root = folder.0.as_path();
        // Two bars at 180 BPM, as a Rave loop.
        let samples = (0..117_600)
            .map(|i| ((i % 100) * 200 - 10_000) as i16)
            .collect();
        fs::write(
            root.join("r1la037.wav"),
            legacy_format::wav::PcmWave {
                sample_rate: 44_100,
                channels: 1,
                samples,
            }
            .encode()
            .unwrap(),
        )
        .unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(root, 180.0).unwrap();
        library
            .register_wav_product(
                Product::T1,
                root,
                &[crate::catalog::WaveMetadata {
                    native_id: 1193,
                    code: "bn/r1la037.pxd".into(),
                    name: "birdy 1".into(),
                    group_name: "loop".into(),
                    bars: 2,
                    source_bpm: 180.0,
                    frame_count: 117_600,
                    pcm_digest: None,
                }],
            )
            .unwrap();
        let record = |file: &str, folder: &str| {
            let mut bytes = vec![1];
            bytes.extend_from_slice(&0i32.to_le_bytes());
            bytes.extend_from_slice(&10211u16.to_le_bytes());
            bytes.extend_from_slice(&[0; 10]);
            text(&mut bytes, r"c:\ejay\MIXWAVES\time018.wav");
            text(&mut bytes, file);
            text(&mut bytes, folder);
            bytes.extend_from_slice(&10u16.to_le_bytes());
            bytes.extend_from_slice(&2u32.to_le_bytes());
            bytes.extend_from_slice(&10200u16.to_le_bytes());
            bytes
        };
        let stretched = record("R1LA037.WAV", r"F:\samples\Rave\LOOP\-1[time]");
        let project = load_bytes(&custom_song("Techno eJay ", &[stretched]), &library).unwrap();
        assert!(project.missing_samples.is_empty());
        let sample = &project.imported_samples[0];
        assert_eq!(sample.name, "birdy 1 140 BPM");
        assert_eq!(sample.source_bpm, 140.0);
        // Two bars at 140 BPM, at the original pitch.
        assert_eq!(
            crate::audio::read_wav(&sample.path).unwrap().frames(),
            151_200
        );
        assert_eq!(project.clips[0].length_ticks, 512);
        // A plain import, or a name the library lacks, stays missing.
        for (file, folder) in [
            ("R1LA037.WAV", r"F:\samples\Rave\LOOP\"),
            ("R1LA999.WAV", r"F:\samples\Rave\LOOP\-1[time]"),
        ] {
            let song = custom_song("Techno eJay ", &[record(file, folder)]);
            let project = load_bytes(&song, &library).unwrap();
            assert_eq!(project.missing_samples.len(), 1, "{file} {folder}");
        }
    }

    #[test]
    fn reader_rejects_truncated_fields() {
        let mut reader = Reader {
            bytes: &[3, 0, b'A'],
            offset: 0,
        };
        assert!(reader.string().is_err());
        let mut reader = Reader {
            bytes: &[1],
            offset: 0,
        };
        assert!(reader.u32().is_err());
    }

    #[test]
    #[ignore = "Requires the locally installed original project fixtures."]
    fn all_eleven_installed_demos_decode_without_missing_ids() {
        let root = crate::project_io::default_root(Product::T2).expect("T2 fixtures");
        let mut library = Library::empty();
        library
            .register(
                Product::T2,
                &root,
                &crate::project_io::reference_samples_root(),
            )
            .unwrap();
        let expected = [
            ("clouds", 577),
            ("edge", 302),
            ("e_funk", 298),
            ("far", 397),
            ("hool", 460),
            ("melodesk", 684),
            ("soap", 356),
            ("start", 467),
            ("string", 206),
            ("summer", 513),
            ("wambam", 171),
        ];
        for (name, count) in expected {
            let path = root.join("\x65Jay").join("mix").join(format!("{name}.mix"));
            let project = load(&path, &library).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(project.clips.len(), count, "{name}");
            assert_eq!(project.tracks.len(), 64);
            let bytes = fs::read(&path).unwrap();
            let original_bpm = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as f64;
            assert_eq!(
                project.bpm,
                crate::engine::nearest_supported_bpm(original_bpm).unwrap(),
                "{name}"
            );
            assert_eq!(project.pitch_ratio, 1.0);
            assert!(
                load_bytes(&bytes[..bytes.len() - 1], &library).is_err(),
                "{name} accepts truncation"
            );
        }
        let start = load(&root.join("\x65Jay/mix/start.mix"), &library).unwrap();
        assert_eq!(start.title, "here we go");
        assert_eq!(start.author, "mc magic");
        assert_eq!(start.pitch_ratio, 1.0);
        assert_eq!(start.bpm, 140.0);
        assert!(
            start
                .import_notices
                .iter()
                .any(|notice| notice.source_bpm == 146.0 && notice.target_bpm == 140.0)
        );
        assert_eq!(start.clips[0].sample_id, 13818);
        assert_eq!(start.clips[0].start_tick, 40 * 256);
        assert!(
            start
                .clips
                .iter()
                .any(|c| c.start_tick % TICKS_PER_BAR != 0)
        );
        assert!(start.tracks[6].left_enabled && !start.tracks[6].right_enabled);
        assert!(!start.tracks[7].left_enabled && start.tracks[7].right_enabled);
    }
}
