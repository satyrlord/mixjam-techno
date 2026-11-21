use super::*;

#[test]
#[ignore = "Set MIXJAM_TECHNO_SONG_FIXTURES to a folder that holds saved T3 and D4 songs."]
fn saved_t3_and_d4_songs_read_every_imported_audio_record() {
    let root = std::env::var_os("MIXJAM_TECHNO_SONG_FIXTURES")
        .expect("Set MIXJAM_TECHNO_SONG_FIXTURES to the saved T3 and D4 song folder.");
    let products = [Product::T3, Product::D4];
    let (mut songs, mut records) = ([0; 2], [0; 2]);
    let mut folders = vec![PathBuf::from(root)];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
                continue;
            }
            let mut start = [0; 4];
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
                || fs::File::open(&path)
                    .and_then(|mut file| file.read_exact(&mut start))
                    .is_err()
            {
                continue;
            }
            let Some(index) = products
                .iter()
                .position(|product| profile(*product).unwrap().0.to_le_bytes() == start)
            else {
                continue;
            };
            let product = products[index];
            let bytes = read_song_file(&path).unwrap();
            let header = read_header(&bytes, product).unwrap();
            let timeline = read_timeline(&bytes, header.after_mixer, product)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let embedded = embedded_wave_records(&bytes).unwrap();
            let (mut imported, mut found) = (0, 0);
            for event in &timeline.events {
                let Reference::Wave(wave) = &event.reference else {
                    continue;
                };
                // Every saved record holds its two labels and bar count.
                assert!(wave.declared_ticks.is_some(), "{}", path.display());
                assert!(wave.metadata.contains_key("Native label 2"));
                imported += 1;
                found += usize::from(embedded.iter().any(|source| {
                    normalize_saved_path(&source.path) == normalize_saved_path(&wave.path)
                }));
            }
            println!(
                "{}: {} events, {imported} imported-audio events, {found} with embedded audio",
                path.display(),
                timeline.events.len()
            );
            songs[index] += 1;
            records[index] += imported;
        }
    }
    assert!(
        songs.iter().sum::<usize>() > 0,
        "The folder holds no T3 or D4 songs."
    );
    for (index, product) in products.iter().enumerate() {
        println!(
            "{} {} songs, {} imported-audio events",
            songs[index],
            product.label(),
            records[index]
        );
    }
}

#[test]
fn counted_events_keep_native_ids_positions_and_complete_trim_fields() {
    let mut bytes = Vec::new();
    lane(&mut bytes, 1, &[(3, -257, 2520)]);
    lane(&mut bytes, 2, &[(0, 128, 2618)]);
    let trim = bytes.len() - 10;
    bytes[trim..trim + 4].copy_from_slice(&7u32.to_le_bytes());
    bytes[trim + 4..trim + 8].copy_from_slice(&11u32.to_le_bytes());
    bytes[trim + 8..].copy_from_slice(&(-1000i16).to_le_bytes());
    let events = read_chunk(&bytes, Product::H1).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].position, 512);
    assert_eq!(events[0].track, 0);
    assert_eq!(events[0].reference, Reference::Native(2520));
    assert_eq!(events[1].position, 256);
    assert_eq!((events[1].left_trim, events[1].right_trim), (14, 22));
    assert_eq!(events[1].gain, 31767.0 / 32767.0);
    assert_ne!(
        Product::H1.global_sample_id(2520).unwrap(),
        Product::D2.global_sample_id(2520).unwrap()
    );
    assert!(read_chunk(&bytes[..bytes.len() - 1], Product::H1).is_err());
}

#[test]
fn utility_records_cannot_return_partial_success_or_overflow_tracks() {
    let mut bytes = Vec::new();
    lane(&mut bytes, 1, &[(0, -1, 174)]);
    lane(&mut bytes, 17, &[(6, -1, 7200)]);
    let error = read_chunk(&bytes, Product::D2).unwrap_err();
    assert!(error.contains("generated pattern"));
    let mut empty_extra = Vec::new();
    lane(&mut empty_extra, 1, &[]);
    lane(&mut empty_extra, 33, &[]);
    assert!(read_chunk(&empty_extra, Product::D3).unwrap().is_empty());
    let mut occupied_extra = Vec::new();
    lane(&mut occupied_extra, 1, &[]);
    lane(&mut occupied_extra, 33, &[(3, -1, 1)]);
    assert_eq!(
        read_chunk(&occupied_extra, Product::D3).unwrap()[0].track,
        32
    );
    let mut overflow = Vec::new();
    lane(&mut overflow, 65, &[(3, -1, 1)]);
    assert!(read_chunk(&overflow, Product::D4).is_err());
    let mut impossible_count = vec![1];
    impossible_count.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(read_chunk(&impossible_count, Product::T3).is_err());
}

#[test]
fn paired_native_labels_survive_shared_project_and_library_validation() {
    let directory = asset_test_directory("paired-labels");
    let path = directory.join("source.wav");
    fs::write(&path, wave_fixture()).unwrap();
    let group = "É".repeat(200);
    let variant = "ß".repeat(200);
    let name = format!("{group} {variant}");
    assert_eq!(name.chars().count(), 401);
    let sample = ImportedSample {
        id: IMPORTED_ID_START,
        name: name.clone(),
        group_name: group.clone(),
        path: path.clone(),
        source_bpm: 140.0,
        source_sample_rate: Some(44100),
        origin: Some(Product::D2),
    };
    crate::engine::validate_import_metadata(&sample).unwrap();
    let project = Project {
        imported_samples: vec![sample].into(),
        ..Project::default()
    };
    project.validate().unwrap();
    let mut library = Library::empty();
    library.apply_imports(&project.imported_samples).unwrap();
    let actual = library.sample_by_id(IMPORTED_ID_START).unwrap();
    assert_eq!(actual.name, name);
    assert_eq!(actual.group_name, group);
    fs::remove_file(path).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
#[ignore = "Requires the local installed D2 song fixtures."]
fn installed_d2_pattern_headers_are_complete_before_audio_conversion() {
    let installation = crate::project_io::default_root(Product::D2)
        .expect("The D2 fixture installation is missing.");
    let folder = installation.join("D2/MIX");
    let mut total_events = 0;
    let mut songs = 0;
    let mut drum_songs = 0;
    let (mut filters, mut echoes) = (0, 0);
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
        {
            continue;
        }
        songs += 1;
        let bytes = read_song_file(&path).unwrap();
        let header = read_header(&bytes, Product::D2).unwrap();
        assert_eq!(header.settings, settings::Saved::default());
        let timeline = read_timeline(&bytes, header.after_mixer, Product::D2).unwrap();
        let kit =
            drums::parse_kit(&bytes, header.after_mixer, timeline.start, Product::D2).unwrap();
        assert_eq!(kit.pads.len(), 10);
        assert_eq!(kit.source_end + 18, timeline.start);
        let effects = kit.groove_effects.unwrap();
        filters += usize::from(effects.filter_on);
        echoes += usize::from(effects.echo_on);
        assert!(!effects.distortion_on);
        assert!((13_527..=25_250).contains(&effects.volume));
        let patterns: Vec<_> = timeline
            .events
            .iter()
            .filter_map(|event| {
                if let Reference::Drum(pattern) = &event.reference {
                    Some((event, pattern))
                } else {
                    None
                }
            })
            .collect();
        if !patterns.is_empty() {
            drum_songs += 1;
        }
        for (event, pattern) in &patterns {
            assert_eq!(event.track, 16);
            assert!(pattern.native_id >= 7200);
            assert_eq!(event.position % 256, 0);
        }
        total_events += patterns.len();
    }
    assert_eq!((songs, drum_songs, total_events), (13, 8, 449));
    assert_eq!((filters, echoes), (3, 4));
}

#[test]
#[ignore = "Requires the local installed D3/D4 song fixtures."]
fn installed_sixteen_pad_headers_and_all_patterns_are_complete() {
    for (product, expected_songs, expected_events, expected_unique) in
        [(Product::D3, 1, 23, 3), (Product::D4, 2, 86, 10)]
    {
        let installation = crate::project_io::default_root(product).unwrap();
        let mut songs = 0;
        let mut events = 0;
        let mut unique = std::collections::HashSet::new();
        for entry in fs::read_dir(installation.join("Mix")).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mix"))
            {
                continue;
            }
            let bytes = read_song_file(&path).unwrap();
            let header = read_header(&bytes, product).unwrap();
            let timeline = read_timeline(&bytes, header.after_mixer, product).unwrap();
            let patterns: Vec<_> = timeline
                .events
                .iter()
                .filter_map(|event| match &event.reference {
                    Reference::Drum(pattern) => Some((event, pattern)),
                    _ => None,
                })
                .collect();
            if patterns.is_empty() {
                continue;
            }
            songs += 1;
            let kit = drums::parse_kit(&bytes, header.after_mixer, timeline.start, product)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(kit.pads.len(), 16);
            assert_eq!(kit.header_end, Some(timeline.start));
            assert_eq!(kit.raw_header, bytes[kit.source_offset..timeline.start]);
            for pad in &kit.pads {
                assert_eq!(pad.opaque_native_settings.len(), 10);
                assert_eq!(pad.source_paths.len(), 2);
            }
            for (event, pattern) in patterns {
                assert_eq!(event.track, profile(product).unwrap().1);
                assert_eq!(pattern.masks.len(), 16);
                assert_eq!(event.position % TICKS_PER_BAR, 0);
                events += 1;
                unique.insert((path.clone(), pattern.clone()));
            }
        }
        assert_eq!(
            (songs, events, unique.len()),
            (expected_songs, expected_events, expected_unique)
        );
    }
}

#[test]
#[ignore = "Requires the local installed song fixtures."]
fn installed_counted_profiles_have_explicit_complete_results() {
    let cases = [
        (Product::D2, "D2/MIX", (13, 13)),
        (Product::D3, "MIX", (17, 17)),
        (Product::D4, "Mix", (12, 12)),
        (Product::H1, "Mix", (14, 14)),
        (Product::T3, "Mix", (20, 20)),
    ];
    for (product, mix_folder, expected) in cases {
        let installation = crate::project_io::default_root(product)
            .expect("The installed song fixtures are missing.");
        let folder = installation.join(mix_folder);
        if product == Product::D4 {
            let placeholder = folder.join(".mix");
            assert_eq!(fs::metadata(&placeholder).unwrap().len(), 2);
            assert!(read_header(&read_song_file(&placeholder).unwrap(), product).is_err());
        }
        let mut library = Library::empty();
        library
            .register(product, &installation, Path::new(""))
            .unwrap();
        let mut parsed = 0;
        let mut drum_audio_projects = 0;
        let mut failures = Vec::new();
        let paths: Vec<_> = fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
            })
            .collect();
        assert_eq!(paths.len(), expected.0);
        for path in paths {
            let bytes = read_song_file(&path).unwrap();
            let result = read_header(&bytes, product).and_then(|header| {
                let events = read_timeline(&bytes, header.after_mixer, product)?;
                build_project(&path, &bytes, product, &library, header, events)
            });
            match result {
                Ok(project) => {
                    project.validate().unwrap();
                    assert_eq!(project.tracks.len(), TRACK_COUNT);
                    assert_eq!(project.pitch_ratio, 1.0);
                    if product == Product::D3
                        && path
                            .file_stem()
                            .is_some_and(|name| name.eq_ignore_ascii_case("all the way up"))
                    {
                        let sample_id = Product::D3.global_sample_id(13633).unwrap();
                        let trimmed = project
                            .clips
                            .iter()
                            .find(|clip| clip.sample_id == sample_id && clip.start_tick == 3040)
                            .unwrap();
                        assert_eq!(trimmed.length_ticks, 48);
                    }
                    assert!(project.clips.iter().all(|clip| {
                        Product::sample_key(clip.sample_id).map_or_else(
                            || {
                                project.imported_samples.iter().any(|sample| {
                                    sample.id == clip.sample_id && sample.origin == Some(product)
                                })
                            },
                            |(owner, _)| owner == product,
                        )
                    }));
                    let utility_track = profile(product).unwrap().1;
                    if project.clips.iter().any(|clip| {
                        clip.track == utility_track && clip.sample_id >= IMPORTED_ID_START
                    }) {
                        library.apply_imports(&project.imported_samples).unwrap();
                        let drum = project
                            .clips
                            .iter()
                            .find(|clip| {
                                clip.track == utility_track && clip.sample_id >= IMPORTED_ID_START
                            })
                            .unwrap()
                            .clone();
                        assert_eq!(drum.length_ticks, TICKS_PER_BAR);
                        let mut representative = project.clone();
                        representative.loop_region = None;
                        representative.clips = vec![Clip {
                            start_tick: 0,
                            ..drum
                        }];
                        let pcm = crate::audio::render_project(&representative, &library).unwrap();
                        pcm.validate().unwrap();
                        assert!(
                            pcm.samples.iter().any(|sample| sample.abs() > 0.0001),
                            "{} drum audio is silent",
                            path.display()
                        );
                        // Render length rounds up, as a bar need not
                        // be a whole number of frames (130 BPM).
                        assert_eq!(pcm.frames(), (44100.0 * 240.0 / project.bpm).ceil() as u64);
                        drum_audio_projects += 1;
                    }
                    if parsed == 0 {
                        let clip = project
                            .clips
                            .iter()
                            .find(|clip| {
                                let track = &project.tracks[clip.track];
                                clip.gain > 0.0
                                    && track.gain > 0.0
                                    && track.audible(project.any_solo())
                                    && (track.left_enabled || track.right_enabled)
                            })
                            .unwrap()
                            .clone();
                        let mut representative = project.clone();
                        representative.loop_region = None;
                        representative.clips = vec![Clip {
                            start_tick: 0,
                            length_ticks: clip.length_ticks.min(TICKS_PER_BAR * 4),
                            ..clip
                        }];
                        let pcm = crate::audio::render_project(&representative, &library)
                            .unwrap_or_else(|error| panic!("{} render: {error}", path.display()));
                        pcm.validate().unwrap();
                        assert_eq!(pcm.channels, 2);
                        assert_eq!(pcm.sample_rate, 44100);
                        assert!(pcm.frames() > 0);
                        let peak = pcm
                            .samples
                            .iter()
                            .fold(0.0f32, |peak, value| peak.max(value.abs()));
                        assert!(peak > 0.0001, "{} rendered silence", path.display());
                        eprintln!(
                            "{} representative: {} sample {}, {} stereo frames, peak {peak}",
                            product.label(),
                            path.file_name().unwrap().to_string_lossy(),
                            representative.clips[0].sample_id,
                            pcm.frames()
                        );
                    }
                    parsed += 1;
                }
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        eprintln!("{}: {parsed} complete projects", product.label());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert_eq!(parsed, expected.1);
        assert_eq!(
            drum_audio_projects,
            match product {
                Product::D2 => 8,
                Product::D3 => 1,
                Product::D4 => 2,
                _ => 0,
            }
        );
    }
}

#[test]
fn d2_drum_patterns_prepare_kit_once_and_keep_partial_missing_pad_fallback() {
    fn wrapped(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&((value.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
        bytes.extend_from_slice(&[0, 1]);
    }
    fn d2_pattern(bytes: &mut Vec<u8>, position: i32, id: u16, masks: &[u16; 10], label: &str) {
        bytes.push(2);
        bytes.extend_from_slice(&position.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
        for mask in masks {
            bytes.extend_from_slice(&mask.to_le_bytes());
        }
        wrapped(bytes, "Groove");
        wrapped(bytes, label);
    }
    let directory = asset_test_directory("d2-drum-prepare");
    let pad_wav = directory.join("pad.wav");
    fs::write(&pad_wav, wave_fixture_frames(16)).unwrap();
    let pad_id = Product::D2.global_sample_id(1518).unwrap();
    let library = Library::fixture(vec![crate::library::Sample {
        id: pad_id,
        code: "pad".into(),
        name: "Pad".into(),
        bars: 1,
        group_name: "Drum".into(),
        offset: 0,
        length: 0,
        frame_count: 16,
        sample_rate: 44_100,
        source_bpm: 140.0,
        channels: 1,
        wav_path: Some(pad_wav.clone()),
        digest: None,
    }]);
    let mut base = header_fixture(Product::D2, 140, 140);
    for index in 0..10 {
        let native_id = if index == 1 { 1518u16 } else { 1518u16 + index };
        base.extend_from_slice(&native_id.to_le_bytes());
        wrapped(&mut base, "Pad");
        base.extend_from_slice(&0i16.to_le_bytes());
        base.extend_from_slice(&0u16.to_le_bytes());
        base.extend_from_slice(&32767u16.to_le_bytes());
        base.extend_from_slice(&0u16.to_le_bytes());
        if index < 9 {
            wrapped(&mut base, "");
        }
    }
    base.extend_from_slice(&[7; 18]);

    let build = |include_missing: bool| {
        let mut bytes = base.clone();
        let after_mixer = header_fixture(Product::D2, 140, 140).len();
        let mut chunk = Vec::new();
        lane(&mut chunk, 1, &[]);
        chunk.push(17);
        chunk.extend_from_slice(&(if include_missing { 3u32 } else { 2u32 }).to_le_bytes());
        d2_pattern(&mut chunk, -1, 7201, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0], "A");
        d2_pattern(&mut chunk, -257, 7202, &[0, 4, 0, 0, 0, 0, 0, 0, 0, 0], "B");
        if include_missing {
            d2_pattern(&mut chunk, -513, 7203, &[0, 0, 1, 0, 0, 0, 0, 0, 0, 0], "C");
        }
        bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&SENTINEL);
        let timeline = read_timeline(&bytes, after_mixer, Product::D2).unwrap();
        let project = build_project(
            Path::new("drums.mix"),
            &bytes,
            Product::D2,
            &library,
            read_header(&bytes, Product::D2).unwrap(),
            timeline,
        )
        .unwrap();
        let result = (
            project.imported_samples.len(),
            project.missing_samples.len(),
            project
                .imported_samples
                .iter()
                .map(|s| s.path.clone())
                .collect::<Vec<_>>(),
        );
        for sample in &project.imported_samples {
            let _ = fs::remove_file(&sample.path);
        }
        result
    };
    let (valid_imports, valid_missing, _) = build(false);
    let (mixed_imports, mixed_missing, _) = build(true);
    assert_eq!((valid_imports, valid_missing), (2, 0));
    assert_eq!((mixed_imports, mixed_missing), (2, 1));
    fs::remove_file(&pad_wav).unwrap();
    fs::remove_dir(&directory).unwrap();
}
