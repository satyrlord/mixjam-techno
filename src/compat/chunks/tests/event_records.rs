use super::*;

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
