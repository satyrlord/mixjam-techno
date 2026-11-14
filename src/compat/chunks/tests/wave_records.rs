use super::*;

#[test]
fn embedded_wave_import_keeps_audio_origin_hash_and_saved_reference() {
    let wave = wave_fixture_frames(44100);
    let source_path = b"C:\\old\\MIXWAVES\\take.wav";
    let mut bytes = header_fixture(Product::D3, 140, 146);
    let after_mixer = bytes.len();
    bytes.extend_from_slice(&((source_path.len() + 2) as u16).to_le_bytes());
    bytes.extend_from_slice(source_path);
    bytes.extend_from_slice(&[0, 1]);
    bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&wave);
    let mut chunk = Vec::new();
    lane(&mut chunk, 1, &[(1, -1, 0)]);
    text_block(&mut chunk, source_path);
    lane(&mut chunk, 2, &[(1, -257, 0)]);
    text_block(&mut chunk, b"c:/OLD/MIXWAVES/TAKE.wav");
    bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
    bytes.extend_from_slice(&chunk);
    bytes.extend_from_slice(&SENTINEL);
    let destination = crate::project_io::data_root()
        .join("imported-audio")
        .join(format!("{:x}.wav", Sha256::digest(&wave)));
    let existed = destination.exists();
    let events = read_timeline(&bytes, after_mixer, Product::D3).unwrap();
    let project = build_project(
        Path::new("embedded.mix"),
        &bytes,
        Product::D3,
        &Library::empty(),
        read_header(&bytes, Product::D3).unwrap(),
        events,
    )
    .unwrap();
    assert_eq!(project.imported_samples.len(), 1);
    let imported = &project.imported_samples[0];
    assert_eq!(imported.origin, Some(Product::D3));
    assert_eq!(imported.path, destination);
    assert_eq!(imported.source_bpm, 140.0);
    assert_eq!(fs::read(&destination).unwrap(), wave);
    assert_eq!(project.clips[0].sample_id, imported.id);
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.clips[1].sample_id, imported.id);
    let mut library = Library::empty();
    library.apply_imports(&project.imported_samples).unwrap();
    let pcm = crate::audio::render_project(&project, &library).unwrap();
    assert!(pcm.samples.iter().any(|value| value.abs() > 0.001));
    let saved = std::env::temp_dir().join(format!(
        "mixjam-techno-embedded-project-{}.json",
        std::process::id()
    ));
    assert!(!saved.exists());
    project.save(&saved).unwrap();
    assert_eq!(Project::load(&saved).unwrap(), project);
    fs::remove_file(&saved).unwrap();
    if !existed {
        fs::remove_file(&destination).unwrap();
    }
}

#[test]
fn embedded_riff_requires_a_matching_path_and_size_record() {
    let wave = wave_fixture();
    for includes_marker in [false, true] {
        let path = b"C:\\old\\MIXWAVES\\take.wav";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(
            &((path.len() + usize::from(includes_marker) * 2) as u16).to_le_bytes(),
        );
        bytes.extend_from_slice(path);
        bytes.extend_from_slice(&[0, 1]);
        bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&wave);
        let found = embedded_waves(&bytes).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].frames, 4);
        assert_eq!(found[0].bytes, wave);
        assert!(matches!(found[0].bytes, Cow::Borrowed(_)));
        let size_offset = 2 + path.len() + 2;
        bytes[size_offset] ^= 1;
        assert!(embedded_waves(&bytes).unwrap().is_empty());
    }
    assert!(embedded_waves(&wave).unwrap().is_empty());
}

#[test]
fn wave_binary_metadata_is_not_an_optional_text_token() {
    for product in [Product::D3, Product::T3] {
        for (category, value) in [
            (1u16, 9u32),
            (2, 256),
            (3, 1),
            (4, 1),
            (5, 1),
            (6, 1),
            (9, 3),
            (10, 1),
            (12, 9),
            (12, 32),
            (12, 45),
        ] {
            let mut chunk = Vec::new();
            lane(&mut chunk, 1, &[(1, -1, 0)]);
            chunk[1..5].copy_from_slice(&2u32.to_le_bytes());
            text_block(&mut chunk, b"C:\\old\\MIXWAVES\\take.wav\0\x01");
            text_block(&mut chunk, b"sample\0\x01");
            text_block(&mut chunk, b"effect\0\x01");
            chunk.extend_from_slice(&category.to_le_bytes());
            chunk.extend_from_slice(&value.to_le_bytes());
            chunk.extend_from_slice(&0u16.to_le_bytes());
            chunk.push(3);
            chunk.extend_from_slice(&(-129i32).to_le_bytes());
            chunk.extend_from_slice(&174u16.to_le_bytes());
            let events = read_chunk(&chunk, product).unwrap();
            assert_eq!(events.len(), 2);
            assert_eq!(events[1].reference, Reference::Native(174));
            assert_eq!(events[1].position, 256);
        }
    }
}

#[test]
fn t3_and_d4_wave_records_keep_bar_counts_that_read_as_text() {
    // As saved in the album songs "Infinitrip" (9 bars, a tab) and "Storm"
    // (32 bars, a space). Read as a third text label, the eight bytes took
    // six of the next event, so its sample ID's high byte (39 or 41) was
    // taken for an unsupported source. D4 saves the same record.
    for (product, bars, next_sample) in [
        (Product::T3, 9u32, 0x27fe_u16),
        (Product::T3, 32, 0x27e1),
        (Product::T3, 9, 0x29ec),
        (Product::T3, 126, 0x2900),
        (Product::D4, 9, 0x27fe),
        (Product::D4, 32, 0x27e1),
        (Product::D4, 126, 0x2900),
    ] {
        let mut chunk = Vec::new();
        lane(&mut chunk, 5, &[(1, -1, 10237)]);
        chunk[1..5].copy_from_slice(&3u32.to_le_bytes());
        for (sample, name, bars) in [(None, "PAD001", bars), (Some(next_sample), "PAD003", 4)] {
            if let Some(sample) = sample {
                chunk.push(1);
                chunk.extend_from_slice(&(-1153i32).to_le_bytes());
                chunk.extend_from_slice(&sample.to_le_bytes());
            }
            let file = format!("{name}.WAV");
            text_block(
                &mut chunk,
                format!("D:\\Legacy\\T3\\MIXWAVES\\{file}\0\x01").as_bytes(),
            );
            text_block(&mut chunk, format!("{file}\0\x01").as_bytes());
            text_block(&mut chunk, b"E:\\samples\\Keys\\\0\x01");
            chunk.extend_from_slice(&12u16.to_le_bytes());
            chunk.extend_from_slice(&bars.to_le_bytes());
            chunk.extend_from_slice(&0u16.to_le_bytes());
        }
        chunk.push(3);
        chunk.extend_from_slice(&(-2049i32).to_le_bytes());
        chunk.extend_from_slice(&174u16.to_le_bytes());
        let events = read_chunk(&chunk, product).unwrap();
        assert_eq!(events.len(), 3);
        let Reference::Wave(first) = &events[0].reference else {
            panic!("the first event is not imported audio");
        };
        assert_eq!(first.path, r"D:\Legacy\T3\MIXWAVES\PAD001.WAV");
        assert_eq!(first.declared_ticks, Some(bars * TICKS_PER_BAR));
        assert_eq!(first.metadata["Native label 1"], "PAD001.WAV");
        assert_eq!(first.metadata["Native label 2"], r"E:\samples\Keys\");
        assert_eq!(first.metadata["Native metadata word 1"], "12");
        let Reference::Wave(second) = &events[1].reference else {
            panic!("the second event is not imported audio");
        };
        assert_eq!(events[1].position, 9 * TICKS_PER_BAR);
        assert_eq!(second.declared_ticks, Some(4 * TICKS_PER_BAR));
        assert_eq!(second.metadata["Saved sample ID"], next_sample.to_string());
        assert_eq!(events[2].reference, Reference::Native(174));
        assert_eq!(events[2].position, 16 * TICKS_PER_BAR);
    }
}

#[test]
fn d3_wave_labels_allow_empty_and_ansi_text_but_require_complete_metadata() {
    let mut extension = Vec::new();
    text_block(&mut extension, b"C:\\old\\MIXWAVES\\take.wav\0\x01");
    text_block(&mut extension, b"\0\x01");
    text_block(&mut extension, b"caf\xe9\0\x01");
    extension.extend_from_slice(&[2, 0, 0, 1, 0, 0, 0, 0]);
    let mut reader = Reader {
        bytes: &extension,
        offset: 0,
    };
    assert_eq!(
        read_wave_extension(&mut reader, 1, Product::D3)
            .unwrap()
            .path,
        r"C:\old\MIXWAVES\take.wav"
    );
    assert_eq!(reader.offset, extension.len());
    for bars in [0u32, MAX_TICKS / TICKS_PER_BAR + 1, u32::MAX] {
        let mut invalid = extension.clone();
        let span = invalid.len() - 6;
        invalid[span..span + 4].copy_from_slice(&bars.to_le_bytes());
        let mut reader = Reader {
            bytes: &invalid,
            offset: 0,
        };
        assert!(read_wave_extension(&mut reader, 1, Product::D3).is_err());
    }
    for end in 0..extension.len() {
        let mut reader = Reader {
            bytes: &extension[..end],
            offset: 0,
        };
        assert!(read_wave_extension(&mut reader, 1, Product::D3).is_err());
    }
    let label_start = 2 + u16::from_le_bytes(extension[..2].try_into().unwrap()) as usize;
    extension[label_start + 3] = 2;
    let mut reader = Reader {
        bytes: &extension,
        offset: 0,
    };
    assert!(read_wave_extension(&mut reader, 1, Product::D3).is_err());
}

#[test]
fn house_wave_records_with_two_labels_keep_their_bar_span() {
    // As saved in the album song "Salon": a MIXWAVES path, the file name,
    // the original path, and eight bytes with the bar count.
    let mut extension = Vec::new();
    text_block(
        &mut extension,
        b"D:\\Legacy\\House\\MIXWAVES\\SYNTHPAD011.wav\0\x01",
    );
    text_block(&mut extension, b"SYNTHPAD011.wav\0\x01");
    text_block(
        &mut extension,
        b"E:\\samples\\House\\Sphere\\SYNTHPAD011.wav\0\x01",
    );
    extension.extend_from_slice(&[10, 0, 4, 0, 0, 0, 0, 0]);
    let mut reader = Reader {
        bytes: &extension,
        offset: 0,
    };
    let reference = read_wave_extension(&mut reader, 6, Product::H1).unwrap();
    assert_eq!(reader.offset, extension.len());
    assert_eq!(reference.declared_ticks, Some(4 * TICKS_PER_BAR));
    assert_eq!(reference.metadata["Native label 1"], "SYNTHPAD011.wav");
    // The 12-byte extension still reads as before, not as labels.
    let mut extended = Vec::new();
    text_block(&mut extended, b"C:\\old\\MIXWAVES\\take.wav\0\x01");
    // Its bytes also frame an empty label and a short one; a record follows.
    extended.extend_from_slice(&[2, 0, 0, 1, 4, 0, b'a', b'b', 0, 1, 0, 0]);
    let end = extended.len();
    extended.extend_from_slice(&[3, 0xff, 0xff, 0xff, 0xff, 174, 0, 0]);
    let mut reader = Reader {
        bytes: &extended,
        offset: 0,
    };
    let reference = read_wave_extension(&mut reader, 1, Product::H1).unwrap();
    assert_eq!(reader.offset, end);
    assert_eq!(reference.declared_ticks, None);
}

#[test]
fn d3_embedded_wave_uses_saved_bar_span_for_trims_and_repeated_sources() {
    // Waterworld stores four bars for 3.75 bars of audio. Its right trim
    // leaves a quarter-bar clip, not an empty measured-duration clip.
    let wave = wave_fixture_frames(283500);
    let path = b"C:\\old\\MIXWAVES\\Water002.wav";
    let mut bytes = header_fixture(Product::D3, 140, 140);
    text_block(&mut bytes, &[path.as_slice(), &[0, 1]].concat());
    bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&wave);
    let mut chunk = Vec::new();
    for (track, bars, right) in [(1, 4u32, 480u32), (2, 2, 224), (3, 4, 0)] {
        lane(&mut chunk, track, &[(1, 0, 0)]);
        let trim = chunk.len() - 10;
        chunk[trim + 4..trim + 8].copy_from_slice(&right.to_le_bytes());
        text_block(&mut chunk, &[path.as_slice(), &[0, 1]].concat());
        text_block(&mut chunk, b"Water\0\x01");
        text_block(&mut chunk, b"002\0\x01");
        chunk.extend_from_slice(&10u16.to_le_bytes());
        chunk.extend_from_slice(&bars.to_le_bytes());
        chunk.extend_from_slice(&14185u16.to_le_bytes());
    }
    bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
    bytes.extend_from_slice(&chunk);
    bytes.extend_from_slice(&SENTINEL);
    let header = read_header(&bytes, Product::D3).unwrap();
    let timeline = read_timeline(&bytes, header.after_mixer, Product::D3).unwrap();
    let destination = crate::project_io::data_root()
        .join("imported-audio")
        .join(format!("{:x}.wav", Sha256::digest(&wave)));
    let existed = destination.exists();
    let mut project = build_project(
        Path::new("span.mix"),
        &bytes,
        Product::D3,
        &Library::empty(),
        header,
        timeline,
    )
    .unwrap();
    assert_eq!(project.imported_samples.len(), 1);
    assert_eq!(
        project
            .clips
            .iter()
            .map(|clip| clip.length_ticks)
            .collect::<Vec<_>>(),
        [64, 64, 1024]
    );
    let mut library = Library::empty();
    library.apply_imports(&project.imported_samples).unwrap();
    project.clips.retain(|clip| clip.track == 2);
    let rendered = crate::audio::render_project(&project, &library).unwrap();
    assert_eq!(rendered.frames(), 302400);
    assert!(
        rendered.samples[..283500 * 2]
            .iter()
            .any(|value| value.abs() > 0.001)
    );
    assert!(
        rendered.samples[283500 * 2..]
            .iter()
            .all(|value| *value == 0.0)
    );
    if !existed {
        fs::remove_file(destination).unwrap();
    }
}

#[test]
fn wave_preflight_matches_pcm_encodings_counts_and_channel_alignment() {
    let directory = asset_test_directory("wav-preflight");
    for bits in [8, 16, 24, 32] {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44100,
                    bits_per_sample: bits,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for sample in [0i32, 100, -100, 0] {
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }
        let bytes = cursor.into_inner();
        assert_eq!(wave_info(&bytes).unwrap(), (4, 44100));
        let path = directory.join(format!("pcm{bits}.wav"));
        fs::write(&path, &bytes).unwrap();
        assert_eq!(crate::audio::read_wav(&path).unwrap().frames(), 4);
        fs::remove_file(path).unwrap();
    }
    // A data chunk that declares far more audio than the file holds is an
    // error, however long the declared audio is.
    let mut excessive = wave_fixture();
    let data = excessive
        .windows(4)
        .position(|bytes| bytes == b"data")
        .unwrap();
    excessive[data + 4..data + 8].copy_from_slice(&(64_000_001u32 * 2).to_le_bytes());
    assert!(wave_info(&excessive).is_err());
    let mut unaligned = wave_fixture_frames(3);
    unaligned[22..24].copy_from_slice(&2u16.to_le_bytes());
    unaligned[28..32].copy_from_slice(&176400u32.to_le_bytes());
    unaligned[32..34].copy_from_slice(&4u16.to_le_bytes());
    assert!(wave_info(&unaligned).is_err());
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn an_external_wave_past_64_mib_is_read_whole() {
    // The reader once stopped at 64 MiB. The file is sparse.
    let directory = asset_test_directory("long-external");
    let path = directory.join("long.wav");
    let length = 64 * 1024 * 1024 + 1;
    fs::File::create(&path).unwrap().set_len(length).unwrap();
    assert_eq!(read_external_wave(&path).unwrap().len() as u64, length);
    fs::remove_file(path).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn saved_wave_paths_require_full_identity_and_preserve_duplicate_checks() {
    let bytes = wave_fixture();
    let same = || Wave {
        source_path: r"C:\original\Kick.wav".into(),
        bytes: bytes.clone().into(),
        frames: 4,
        sample_rate: 44100,
    };
    let embedded = [same(), same()];
    let wave = source_wave("c:/ORIGINAL/kick.wav", &embedded, &Library::empty()).unwrap();
    assert_eq!(wave.bytes, bytes);
    let different = Wave {
        source_path: "C:/original/KICK.wav".into(),
        bytes: b"different".to_vec().into(),
        frames: 4,
        sample_rate: 44100,
    };
    assert!(
        source_wave(
            r"C:\original\Kick.wav",
            &[same(), different],
            &Library::empty()
        )
        .is_err()
    );
    assert!(source_wave(r"C:\missing-user\Kick.wav", &[same()], &Library::empty()).is_err());
}

#[test]
fn missing_user_wave_does_not_bind_an_unrelated_installation_basename() {
    let directory = asset_test_directory("wave-identity");
    let installation = directory.join("installation/MIXWAVES");
    fs::create_dir_all(&installation).unwrap();
    let unrelated = installation.join("kick.wav");
    fs::write(&unrelated, wave_fixture()).unwrap();
    let missing = directory.join("missing-user/kick.wav");
    assert!(source_wave(&missing.to_string_lossy(), &[], &Library::empty()).is_err());
    // Selecting the WAV folder explicitly makes its unique filename usable
    // after the original user's absolute path has become unavailable.
    let mut library = Library::empty();
    library.import_wav_folder(&installation, 140.0).unwrap();
    let moved = source_wave(&missing.to_string_lossy(), &[], &library).unwrap();
    assert_eq!(moved.bytes, wave_fixture());
    fs::remove_file(&unrelated).unwrap();
    fs::remove_dir(&installation).unwrap();
    fs::remove_dir(directory.join("installation")).unwrap();
    fs::remove_dir(&directory).unwrap();
}
