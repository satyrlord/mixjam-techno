use super::*;
use crate::products::IMPORTED_ID_START;
fn mix() -> Vec<u8> {
    let mut b = vec![0; TRAILER_OFFSET];
    b[..2].copy_from_slice(&[6, 10]);
    b
}
fn place(b: &mut [u8], row: usize, track: usize, sample: u16, offset: u16) {
    let cell = row * TRACKS + track;
    b[2 + cell * 2..4 + cell * 2].copy_from_slice(&sample.to_le_bytes());
    b[2 + CELLS * 2 + cell * 2..4 + CELLS * 2 + cell * 2].copy_from_slice(&offset.to_le_bytes());
}
fn resolve(id: u16) -> Result<(u32, u32, String), String> {
    if (731..2083).contains(&id) {
        Ok((
            Product::D1.global_sample_id(u32::from(id))?,
            2,
            "base.\x70xd".into(),
        ))
    } else {
        Err("Unknown fixture sample.".into())
    }
}
fn wrapped(b: &mut Vec<u8>, text: &[u8]) {
    b.extend(((text.len() + 2) as u16).to_le_bytes());
    b.extend(text);
    b.extend([0, 1]);
}
#[test]
fn wav_only_provider_keeps_d1_native_ids_and_nominal_bars() {
    let root = std::env::temp_dir().join(format!(
        "d1-wav-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let wave_path = root.join("base.wav");
    fs::write(
        &wave_path,
        legacy_format::wav::PcmWave {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![1000; 37_800],
        }
        .encode()
        .unwrap(),
    )
    .unwrap();
    let mut library = Library::empty();
    library.import_wav_folder(&root, 140.0).unwrap();
    library
        .register_wav_product(
            Product::D1,
            &root,
            &[crate::catalog::WaveMetadata {
                native_id: 731,
                code: "base.\x70xd".into(),
                label_key: crate::catalog::label_key("Sample"),
                group_name: "Test".into(),
                bars: 2,
                source_bpm: 140.0,
                frame_count: 75_600,
                pcm_digest: None,
            }],
        )
        .unwrap();
    let mut bytes = mix();
    place(&mut bytes, 0, 0, 731, 0);
    let path = root.join("song.mix");
    fs::write(&path, bytes).unwrap();
    let project = load(&path, &library).unwrap();
    assert_eq!(
        project.clips[0].sample_id,
        Product::D1.global_sample_id(731).unwrap()
    );
    assert_eq!(project.clips[0].start_tick, 0);
    assert_eq!(project.clips[0].length_ticks, 512);
    fs::remove_file(path).unwrap();
    fs::remove_file(wave_path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn matrix_positions_use_sixteenth_ticks_and_native_duration_caps_at_140_bpm() {
    let mut b = mix();
    place(&mut b, 0, 0, 731, 4);
    place(&mut b, 1, 0, 732, 12);
    let p = convert(&b, Path::new("fixture.mix"), &Library::empty(), resolve).unwrap();
    assert_eq!(p.bpm, 140.0);
    assert_eq!(p.tracks.len(), 64);
    assert_eq!((p.clips[0].start_tick, p.clips[0].length_ticks), (64, 384));
    assert_eq!((p.clips[1].start_tick, p.clips[1].length_ticks), (448, 576));
    assert!(p.import_notices.is_empty());
}
#[test]
fn factory_ids_above_t1_import_boundary_remain_native_and_routes_survive() {
    let mut b = mix();
    place(&mut b, 0, 0, 1989, 0);
    b.extend([8, 10, 1, 0]);
    wrapped(&mut b, b"\x44ance \x65Jay 1.01");
    b.extend([0, 0, 8, 0]);
    for route in [1u16, 2, 3, 0, 0, 0, 0, 0] {
        b.extend(route.to_le_bytes());
    }
    let p = convert(&b, Path::new("fixture.mix"), &Library::empty(), resolve).unwrap();
    assert_eq!(
        p.clips[0].sample_id,
        Product::D1.global_sample_id(1989).unwrap()
    );
    assert!(p.imported_samples.is_empty());
    assert!(!p.tracks[0].right_enabled);
    assert!(!p.tracks[1].left_enabled);
    assert!(!p.tracks[2].left_enabled && !p.tracks[2].right_enabled);
}
#[test]
fn invalid_guards_offsets_footer_counts_and_truncation_fail_without_partial_project() {
    let mut b = mix();
    place(&mut b, 350, 0, 731, 0);
    assert!(convert(&b, Path::new("bad.mix"), &Library::empty(), resolve).is_err());
    let mut b = mix();
    place(&mut b, 0, 0, 731, 16);
    assert!(convert(&b, Path::new("bad.mix"), &Library::empty(), resolve).is_err());
    assert!(convert(&b[..100], Path::new("bad.mix"), &Library::empty(), resolve).is_err());
    let mut b = mix();
    place(&mut b, 0, 0, 2083, 0);
    assert!(convert(&b, Path::new("bad.mix"), &Library::empty(), resolve).is_err());
    assert!(parse_footer(&[8, 10, 3, 0], &BTreeSet::new()).is_err());
}
#[test]
fn imported_footer_lengths_use_original_ansi_bytes_and_d1_import_ids() {
    let mut footer = vec![8, 10, 1, 0];
    wrapped(&mut footer, b"\x44ance \x65Jay 1.01");
    footer.extend(1u16.to_le_bytes());
    let fields: [&[u8]; 4] = [b"caf\xe9", b"take", b"tone.wav", b"Wave"];
    let payload = 8 + fields.iter().map(|field| field.len()).sum::<usize>();
    footer.extend((payload as u16).to_le_bytes());
    for field in fields {
        wrapped(&mut footer, field);
    }
    footer.extend(8u16.to_le_bytes());
    footer.extend([0; 16]);
    let parsed = parse_footer(&footer, &BTreeSet::from([2083])).unwrap();
    assert_eq!(parsed.imports[0].0, 2083);
    assert_eq!(parsed.imports[0].1[0], "café");
    assert!(parse_footer(&footer[..footer.len() - 1], &BTreeSet::from([2083])).is_err());
    assert!(parse_footer(&footer, &BTreeSet::new()).is_err());
}
#[test]
fn a_bad_saved_path_makes_one_placeholder_and_never_reaches_the_file_system() {
    let paths = [
        r"\\attacker\share\a.wav",
        "//attacker/share/b.wav",
        r"\\.\pipe\c.wav",
        r"\\?\UNC\attacker\share\d.wav",
        r"\root-relative.wav",
        "bad\u{7}byte.wav",
        r"C:\music\COM1",
        "nul",
        "",
    ];
    let mut b = mix();
    for (index, _) in paths.iter().enumerate() {
        place(&mut b, index * 2, 0, 2083 + index as u16, 0);
    }
    b.extend([8, 10, 1, 0]);
    wrapped(&mut b, b"\x44ance \x65Jay 1.01");
    b.extend((paths.len() as u16).to_le_bytes());
    for path in paths {
        let fields: [&[u8]; 4] = [b"take", b"one", path.as_bytes(), b"Wave"];
        let payload = 8 + fields.iter().map(|field| field.len()).sum::<usize>();
        b.extend((payload as u16).to_le_bytes());
        for field in fields {
            wrapped(&mut b, field);
        }
    }
    b.extend(8u16.to_le_bytes());
    b.extend([0; 16]);
    let p = convert(
        &b,
        Path::new("d:/songs/hostile.mix"),
        &Library::empty(),
        resolve,
    )
    .unwrap();
    assert!(p.imported_samples.is_empty());
    assert_eq!(p.missing_samples.len(), paths.len());
    assert_eq!(p.clips.len(), paths.len());
    for (missing, path) in p.missing_samples.iter().zip(paths) {
        // A rejected path never becomes a "missing file" or "cannot open" reason.
        assert!(
            missing.reason.contains("needs a full drive path")
                || missing.reason.contains("path is invalid"),
            "{path}: {}",
            missing.reason
        );
        assert_eq!(missing.metadata["saved_path"], path);
        assert!(matches!(
            &missing.reference,
            MissingReference::Generated { kind, data, .. }
                if kind == super::super::UNRESOLVED_IMPORT_PATH && data["saved_path"] == path
        ));
    }
}
#[test]
fn shipped_kit_names_resolve_and_unknown_names_preserve_silent_recovery_details() {
    for (kit, names) in [
        vec![
            "DanceMachine Sample-Kit Vol. 1",
            "gung -SAMPLE BOX- [Raps && Vocals]",
        ],
        vec![
            "DanceMachine Sample-Kit Vol. 2",
            "gung -SAMPLE BOX- [Drams && Synthies]",
        ],
        vec!["gung -SAMPLE BOX- [Space Sounds]"],
    ]
    .into_iter()
    .enumerate()
    {
        for name in names
            .into_iter()
            .chain(std::iter::once("Unknown replacement kit"))
        {
            let source_id = KIT_REGIONS[kit].0;
            let mut bytes = mix();
            place(&mut bytes, 0, 0, source_id, 0);
            bytes.extend([8, 10, 2, 0]);
            wrapped(&mut bytes, b"Kit demo");
            bytes.extend(3u16.to_le_bytes());
            for (index, (first, last)) in KIT_REGIONS.into_iter().enumerate() {
                bytes.extend(first.to_le_bytes());
                bytes.extend(last.to_le_bytes());
                wrapped(&mut bytes, if index == kit { name.as_bytes() } else { b"" });
            }
            bytes.extend([0, 0, 8, 0]);
            bytes.extend([0; 16]);
            let project = convert(&bytes, Path::new("kit.mix"), &Library::empty(), |id| {
                Ok((
                    Product::D1.global_sample_id(u32::from(id))?,
                    2,
                    format!("dmkit{}/sample.pxd", kit + 1),
                ))
            })
            .unwrap();
            assert_eq!(project.clips.len(), 1);
            if name == "Unknown replacement kit" {
                assert_eq!(project.missing_samples.len(), 1);
                assert_eq!(project.missing_samples[0].metadata["kit_name"], name);
                assert!(matches!(&project.missing_samples[0].reference,
                    MissingReference::Generated { kind, .. } if kind == "unresolved-kit"));
                assert!(project.missing_samples[0].id >= IMPORTED_ID_START);
            } else {
                assert!(project.missing_samples.is_empty(), "{name}");
                assert_eq!(
                    project.clips[0].sample_id,
                    Product::D1.global_sample_id(u32::from(source_id)).unwrap()
                );
                assert_eq!(project.clips[0].length_ticks, 2 * TICKS_PER_BAR);
            }
        }
    }
}

#[test]
fn remapped_kit_ranges_preserve_sources_but_invalid_ranges_reject() {
    let build = |first: u16, last: u16| {
        let mut bytes = mix();
        place(&mut bytes, 0, 0, 3300, 4);
        place(&mut bytes, 7, 0, 731, 0);
        bytes.extend([8, 10, 2, 0]);
        wrapped(&mut bytes, b"Remapped kit");
        bytes.extend(3u16.to_le_bytes());
        for (first, last) in [(first, last), (3900, 4499), (4500, 5199)] {
            bytes.extend(first.to_le_bytes());
            bytes.extend(last.to_le_bytes());
            wrapped(&mut bytes, b"");
        }
        bytes.extend([0, 0, 8, 0]);
        bytes.extend([0; 16]);
        bytes
    };
    let project = convert(
        &build(3300, 3899),
        Path::new("remapped.mix"),
        &Library::empty(),
        resolve,
    )
    .unwrap();
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.clips[0].start_tick, 64);
    assert_eq!(project.clips[0].length_ticks, 7 * TICKS_PER_BAR - 64);
    let missing = &project.missing_samples[0];
    assert_eq!(missing.metadata["source_id"], "3300");
    assert_eq!(missing.metadata["kit_first_id"], "3300");
    assert_eq!(missing.metadata["kit_last_id"], "3899");
    assert!(missing.length_inferred);
    assert!(matches!(&missing.reference,
        MissingReference::Generated { kind, .. } if kind == "unresolved-kit"));
    for invalid in [build(3300, 3900), build(3300, 3299), build(700, 3899)] {
        assert!(
            convert(
                &invalid,
                Path::new("invalid.mix"),
                &Library::empty(),
                resolve
            )
            .unwrap_err()
            .contains("invalid or overlap")
        );
    }
}

#[test]
fn missing_catalog_audio_keeps_duration_metadata_and_every_placement() {
    let sample = crate::catalog::WaveMetadata {
        native_id: u32::from(FACTORY_FIRST),
        code: "fixture.pxd".into(),
        label_key: crate::catalog::label_key("Fixture"),
        group_name: "Bass".into(),
        bars: 2,
        source_bpm: 140.0,
        frame_count: 44_100,
        pcm_digest: None,
    };
    let mut bytes = mix();
    place(&mut bytes, 0, 0, sample.native_id as u16, 0);
    place(&mut bytes, 12, 2, sample.native_id as u16, 4);
    let project = convert_with_catalog(
        &bytes,
        Path::new("missing.mix"),
        &Library::empty(),
        |_| Err("Audio unavailable.".into()),
        |id| (id == sample.native_id).then_some(&sample),
    )
    .unwrap();
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.missing_samples.len(), 1);
    let missing = &project.missing_samples[0];
    assert!(!missing.length_inferred);
    assert_eq!(missing.nominal_ticks, sample.bars * TICKS_PER_BAR);
    assert_eq!(missing.name, sample.display_name(Product::D1));
    assert_eq!(missing.metadata["code"], sample.code);
    assert_eq!(missing.metadata["category"], sample.group_name);
    assert_eq!(project.clips[0].length_ticks, sample.bars * TICKS_PER_BAR);
    assert_eq!(project.clips[1].start_tick, 12 * TICKS_PER_BAR + 64);
}

#[test]
fn missing_external_audio_retains_labels_path_category_and_inferred_extent() {
    let mut bytes = mix();
    place(&mut bytes, 0, 0, IMPORT_FIRST, 0);
    place(&mut bytes, 5, 0, 731, 0);
    place(&mut bytes, 10, 2, IMPORT_FIRST, 8);
    bytes.extend([8, 10, 1, 0]);
    wrapped(&mut bytes, b"Missing take");
    bytes.extend(1u16.to_le_bytes());
    let fields: [&[u8]; 4] = [b"caf\xe9", b"take", b"absent/take.wav", b"Voice"];
    bytes
        .extend(((8 + fields.iter().map(|field| field.len()).sum::<usize>()) as u16).to_le_bytes());
    for field in fields {
        wrapped(&mut bytes, field);
    }
    bytes.extend(8u16.to_le_bytes());
    bytes.extend([0; 16]);
    let project = convert(&bytes, Path::new("missing.mix"), &Library::empty(), resolve).unwrap();
    assert_eq!(project.clips.len(), 3);
    assert_eq!(project.missing_samples.len(), 1);
    assert!(project.imported_samples.is_empty());
    let missing = &project.missing_samples[0];
    assert_eq!(missing.name, "café take");
    assert_eq!(missing.metadata["label_1"], "café");
    assert_eq!(missing.metadata["label_2"], "take");
    assert_eq!(missing.metadata["saved_path"], "absent/take.wav");
    assert_eq!(missing.metadata["category"], "Voice");
    assert_eq!(missing.metadata["source_id"], IMPORT_FIRST.to_string());
    assert_eq!(missing.nominal_ticks, 5 * TICKS_PER_BAR);
    assert!(missing.length_inferred);
    assert_eq!(project.clips[0].length_ticks, 5 * TICKS_PER_BAR);
    assert_eq!(project.clips[2].start_tick, 10 * TICKS_PER_BAR + 128);
    let encoded = serde_json::to_string(&project).unwrap();
    let reopened: Project = serde_json::from_str(&encoded).unwrap();
    reopened.validate().unwrap();
    assert_eq!(
        serde_json::to_value(reopened).unwrap(),
        serde_json::to_value(project).unwrap()
    );
}

#[test]
fn external_wave_conflict_selects_first_for_all_repeated_placements() {
    let root = std::env::temp_dir().join(format!(
        "d1-conflict-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for (folder, value) in [("A", 0.1), ("B", 0.2)] {
        fs::create_dir_all(root.join(folder)).unwrap();
        crate::audio::Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![value; 4410],
        }
        .write_wav(&root.join(folder).join("take.wav"))
        .unwrap();
    }
    let mut library = Library::empty();
    library.import_wav_folder(&root, 140.0).unwrap();
    let mut bytes = mix();
    place(&mut bytes, 0, 0, IMPORT_FIRST, 0);
    place(&mut bytes, 4, 1, IMPORT_FIRST, 0);
    bytes.extend([8, 10, 1, 0]);
    wrapped(&mut bytes, b"Conflicting take");
    bytes.extend(1u16.to_le_bytes());
    let fields: [&[u8]; 4] = [b"Recorded", b"take", b"old/take.wav", b"Voice"];
    bytes
        .extend(((8 + fields.iter().map(|field| field.len()).sum::<usize>()) as u16).to_le_bytes());
    for field in fields {
        wrapped(&mut bytes, field);
    }
    bytes.extend(8u16.to_le_bytes());
    bytes.extend([0; 16]);
    let project = convert(&bytes, &root.join("song.mix"), &library, resolve).unwrap();
    assert!(project.missing_samples.is_empty());
    assert_eq!(project.sample_conflicts.len(), 1);
    let conflict = &project.sample_conflicts[0];
    assert_eq!(conflict.candidates.len(), 2);
    assert_eq!(conflict.selected, 0);
    assert_eq!(conflict.sample_ids, vec![project.imported_samples[0].id]);
    assert_eq!(
        project.imported_samples[0].path,
        fs::canonicalize(root.join("A/take.wav")).unwrap()
    );
    assert_eq!(project.clips.len(), 2);
    assert!(
        project
            .clips
            .iter()
            .all(|clip| clip.sample_id == conflict.sample_ids[0])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "Set MIXJAM_TECHNO_COMPAT_INSTALLATIONS to the extracted five-product media directory."]
fn all_sixteen_shipped_d1_mixes_import_with_native_ids_and_complete_footers() {
    let base = PathBuf::from(
        std::env::var_os("MIXJAM_TECHNO_COMPAT_INSTALLATIONS")
            .expect("Set MIXJAM_TECHNO_COMPAT_INSTALLATIONS"),
    );
    let root = base.join("\x64ance1");
    let mut library = Library::empty();
    library
        .register(Product::D1, &root, Path::new("unused"))
        .unwrap();
    let mut count = 0;
    for entry in fs::read_dir(root.join("MIX")).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        let expected = bytes[2..5618]
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|cell| **cell != [0, 0])
            .count();
        let project = load(&path, &library).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(project.clips.len(), expected);
        assert_eq!(project.bpm, 140.0);
        assert!(
            project
                .clips
                .iter()
                .all(|clip| library.sample_by_id(clip.sample_id).is_some())
        );
        count += 1;
    }
    assert_eq!(count, 16);
}

#[test]
fn d1_schedule_matches_legacy_format_across_offsets_and_boundaries() {
    let mut b = mix();
    // Place clips across multiple tracks, offsets, multi-bar cuts, and the row-349 guard edge.
    place(&mut b, 0, 2, 731, 4);
    place(&mut b, 1, 2, 732, 12);
    place(&mut b, 0, 0, 733, 0);
    place(&mut b, 2, 0, 734, 8);
    place(&mut b, 349, 7, 735, 15);
    let bars_for = |id: u16| match id {
        731..=734 => Some(3),
        735 => Some(4),
        _ => None,
    };
    let resolve_var = |id: u16| {
        bars_for(id)
            .map(|bars| {
                (
                    Product::D1.global_sample_id(u32::from(id)).unwrap(),
                    bars,
                    "base.\x70xd".into(),
                )
            })
            .ok_or_else(|| "Unknown".to_string())
    };
    let project = convert(&b, Path::new("parity.mix"), &Library::empty(), resolve_var).unwrap();
    let legacy_mix_events = [
        (0usize, 2usize, 731u16, 4u16),
        (1, 2, 732, 12),
        (0, 0, 733, 0),
        (2, 0, 734, 8),
        (349, 7, 735, 15),
    ];
    let mut legacy = legacy_format::mix::Mix::default();
    for (row, track, id, offset) in legacy_mix_events {
        legacy.set_cell(row, track, id, offset).unwrap();
    }
    let scheduled = legacy_format::timeline::schedule(&legacy, bars_for).unwrap();
    let step = u64::from(legacy_format::timeline::FRAMES_PER_SIXTEENTH);
    let expected: Vec<_> = scheduled
        .into_iter()
        .map(|s| {
            (
                s.track,
                (s.start_frame / step * 16) as u32,
                (s.limit_frames / step * 16) as u32,
            )
        })
        .collect();
    let actual: Vec<_> = project
        .clips
        .iter()
        .map(|c| (c.track, c.start_tick, c.length_ticks))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn a_long_external_wave_is_cropped_to_the_editable_grid() {
    let root = std::env::temp_dir().join(format!(
        "d1-long-wave-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let wave_path = root.join("long.wav");
    // A low sample rate exceeds 32,767 source bars in a small valid fixture.
    fs::write(
        &wave_path,
        legacy_format::wav::PcmWave {
            sample_rate: 1,
            channels: 1,
            samples: vec![1000; 60_000],
        }
        .encode()
        .unwrap(),
    )
    .unwrap();

    let mut bytes = mix();
    place(&mut bytes, 349, 0, IMPORT_FIRST, 15);
    bytes.extend([8, 10, 1, 0]);
    wrapped(&mut bytes, b"Long take");
    bytes.extend(1u16.to_le_bytes());
    let fields: [&[u8]; 4] = [b"Recorded", b"take", b"long.wav", b"Voice"];
    let payload = 8 + fields.iter().map(|field| field.len()).sum::<usize>();
    bytes.extend((payload as u16).to_le_bytes());
    for field in fields {
        wrapped(&mut bytes, field);
    }
    bytes.extend(8u16.to_le_bytes());
    bytes.extend([0; 16]);
    let project = convert(&bytes, &root.join("song.mix"), &Library::empty(), resolve).unwrap();
    assert_eq!(
        project.imported_samples[0].path,
        fs::canonicalize(&wave_path).unwrap()
    );
    assert_eq!(project.clips[0].start_tick, 350 * TICKS_PER_BAR - 16);
    assert_eq!(project.clips[0].length_ticks, 16);
    fs::remove_file(wave_path).unwrap();
    fs::remove_dir(root).unwrap();
}
