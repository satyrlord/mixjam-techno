//! Reader for the 0x889 project profile found in the eleven installed demos.
//! Unknown preamble data is kept outside the audio arrangement decoder. A match
//! must contain all sixteen numbered, counted track lists and the exact trailer.
use crate::{
    engine::{Clip, Project, TICKS_PER_BAR, Track},
    library::{Library, decode_1252},
};
use std::{fs, path::Path};

const TRAILER: &[u8] = &[17, 0, 0, 0, 0, 0xa9, 0x7a, 0x8a, 1];

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 16 * 1024 * 1024 {
        return Err("The original mix file is too large.".into());
    }
    load_bytes(&fs::read(path).map_err(|e| e.to_string())?, library)
}

pub fn load_bytes(bytes: &[u8], library: &Library) -> Result<Project, String> {
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
            pan: 0.0,
            left_enabled: matches!(routing, 0 | 1),
            right_enabled: matches!(routing, 0 | 2),
        });
    }
    // The installed profile has ten further bytes, followed by a product name,
    // sample-kit declarations and drum instrument settings. Those controls are
    // not yet reconstructed. The track lists are independently self-delimiting.
    reader.take(10)?;
    let product = reader.string()?;
    if product.trim() != "\x54echno \x65Jay 2" {
        return Err("The mix belongs to a different product.".into());
    }
    let search_start = reader.offset;
    let search_end = bytes
        .len()
        .saturating_sub(16 * 5 + TRAILER.len())
        .min(search_start + 4096);
    let mut arrangement = None;
    // Only a complete track list with the exact trailer can identify a missing
    // sample. Preamble bytes can also resemble the beginning of a track list.
    let mut missing: Option<ArrangementError> = None;
    for start in search_start..=search_end {
        if bytes.get(start) != Some(&1) {
            continue;
        }
        match parse_arrangement(bytes, start, library) {
            Ok(clips) => {
                if arrangement.is_some() || missing.is_some() {
                    return Err("The original mix has ambiguous arrangement data.".into());
                }
                arrangement = Some(clips);
            }
            Err(error) if error.missing_sample => {
                if arrangement.is_some() || missing.is_some() {
                    return Err("The original mix has ambiguous arrangement data.".into());
                }
                missing = Some(error);
            }
            Err(_) => {}
        }
    }
    let clips = match (arrangement, missing) {
        (Some(clips), _) => clips,
        (None, Some(error)) => return Err(error.message),
        (None, None) => {
            return Err("Cannot decode the original mix arrangement. The file is damaged or uses an unsupported profile.".into());
        }
    };
    let project = Project {
        title,
        author,
        bpm: source_bpm as f64,
        master_gain: 1.0,
        pitch_ratio: playback_bpm as f64 / source_bpm as f64,
        tracks,
        clips,
        loop_region: None,
        imported_samples: Vec::new(),
        libraries: library.bindings().to_vec(),
        ..Project::default()
    };
    crate::engine::normalize_import(project)
}

#[derive(Debug)]
struct ArrangementError {
    missing_sample: bool,
    message: String,
}

fn parse_arrangement(
    bytes: &[u8],
    start: usize,
    library: &Library,
) -> Result<Vec<Clip>, ArrangementError> {
    let mut reader = Reader {
        bytes,
        offset: start,
    };
    let mut clips = Vec::new();
    match parse_tracks(&mut reader, library, &mut clips) {
        Ok(None) => Ok(clips),
        Ok(Some(message)) => Err(ArrangementError {
            missing_sample: true,
            message,
        }),
        Err(message) => Err(ArrangementError {
            missing_sample: false,
            message,
        }),
    }
}

fn parse_tracks(
    reader: &mut Reader,
    library: &Library,
    clips: &mut Vec<Clip>,
) -> Result<Option<String>, String> {
    let mut missing = None;
    for track in 0..16 {
        if reader.u8()? as usize != track + 1 {
            return Err("Invalid track marker.".into());
        }
        let count = reader.u32()? as usize;
        if count > 100_000 || count > reader.bytes.len().saturating_sub(reader.offset) / 7 {
            return Err("Invalid clip count.".into());
        }
        for _ in 0..count {
            if reader.u8()? != 0 {
                return Err("Unsupported clip source.".into());
            }
            let stored_position = reader.i32()?;
            let sample_id = reader.u16()? as u32;
            let native_position = if stored_position < 0 {
                !stored_position as u32
            } else {
                stored_position as u32
            };
            // Native duration writer 0x4fd1d5 divides stored positions by 128.
            // Common projects use 256 ticks per bar, so convert exactly once.
            let start_tick = native_position
                .checked_mul(2)
                .ok_or("Invalid native clip position.")?;
            let (left_trim, right_trim, gain_delta) = if stored_position < 0 {
                (0, 0, 0)
            } else {
                (reader.u32()?, reader.u32()?, reader.i16()?)
            };
            let left_trim = left_trim
                .checked_mul(2)
                .ok_or("Invalid native clip trim.")?;
            let right_trim = right_trim
                .checked_mul(2)
                .ok_or("Invalid native clip trim.")?;
            let Some(sample) = library.sample_by_id(sample_id) else {
                missing.get_or_insert_with(|| {
                    library.describe_missing(crate::products::Product::T2, sample_id)
                });
                continue;
            };
            let full_ticks = sample
                .bars
                .checked_mul(TICKS_PER_BAR)
                .ok_or("Invalid native sample span.")?;
            let length_ticks = full_ticks
                .checked_sub(left_trim)
                .and_then(|n| n.checked_sub(right_trim))
                .filter(|n| *n > 0)
                .ok_or("Invalid clip trim.")?;
            let offset_frames = (f64::from(left_trim) * f64::from(sample.sample_rate) * 240.0
                / (sample.source_bpm * f64::from(TICKS_PER_BAR)))
            .round();
            if !offset_frames.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&offset_frames) {
                return Err("Invalid native clip audio offset.".into());
            }
            clips.push(Clip {
                id: clips.len() as u64 + 1,
                sample_id,
                track,
                start_tick,
                length_ticks,
                offset_frames: offset_frames as u32,
                gain: (32767i32 + gain_delta as i32) as f32 / 32767.0,
            });
        }
    }
    if reader.bytes.get(reader.offset..) != Some(TRAILER) {
        return Err("Invalid mix trailer.".into());
    }
    Ok(missing)
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

    fn song_with_missing_sample(preamble: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x889u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&140u16.to_le_bytes());
        bytes.extend_from_slice(&140u32.to_le_bytes());
        for text in ["Composer", "Missing sample"] {
            bytes.extend_from_slice(&((text.len() + 2) as u16).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(&[0, 1]);
        }
        bytes.extend_from_slice(&[0; 74]);
        let product = "\x54echno \x65Jay 2";
        bytes.extend_from_slice(&((product.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(product.as_bytes());
        bytes.extend_from_slice(&[0, 1]);
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
        let expected = library.describe_missing(crate::products::Product::T2, 174);
        for prefix in [&[][..], preamble.as_slice()] {
            assert_eq!(
                load_bytes(&song_with_missing_sample(prefix), &library).unwrap_err(),
                expected
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
        let root = std::env::temp_dir().join(format!(
            "t2-wav-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
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
        library.import_wav_folder(&root, 140.0).unwrap();
        library
            .register_wav_product(
                crate::products::Product::T2,
                &root,
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
        fn text(bytes: &mut Vec<u8>, value: &str) {
            bytes.extend_from_slice(&((value.len() + 2) as u16).to_le_bytes());
            bytes.extend_from_slice(value.as_bytes());
            bytes.extend_from_slice(&[0, 1]);
        }
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
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn trimmed_wav_sources_render_the_same_audio_at_different_sample_rates() {
        let root = std::env::temp_dir().join(format!(
            "t2-trim-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
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
            library.import_wav_folder(&root, 140.0).unwrap();
            assert_eq!(
                library
                    .register_wav_product(
                        crate::products::Product::T2,
                        &root,
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
            let clips = parse_arrangement(&bytes, 0, &library).unwrap();
            assert_eq!(clips[0].offset_frames as usize, bar_frames);
            assert_eq!(clips[0].length_ticks, TICKS_PER_BAR);
            let project = Project {
                clips,
                master_gain: 1.0,
                ..Project::default()
            };
            let pcm = crate::audio::render_project(&project, &library).unwrap();
            assert_eq!(pcm.frames(), 75_600);
            // The first bar is silent; every frame must come from the second bar.
            assert!(
                pcm.samples
                    .iter()
                    .all(|sample| (*sample - 0.25).abs() < 0.0001)
            );
        }
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn reader_rejects_truncated_and_foreign_files() {
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
        let root =
            crate::project_io::default_root(crate::products::Product::T2).expect("T2 fixtures");
        let library = Library::open(&root).unwrap();
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
