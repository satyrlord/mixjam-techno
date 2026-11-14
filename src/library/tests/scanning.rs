use super::*;

#[test]
fn a_folder_scan_uses_one_catalog_snapshot_for_identity_and_tempo() {
    let folder = Folder::new();
    let catalogs_dir = folder.0.join("catalogs");
    fs::create_dir(&catalogs_dir).unwrap();
    let mut entries = Vec::new();
    for index in 0..3 {
        let audio = waveform(index + 10, 300);
        for root in ["First", "Second"] {
            folder.mono(&format!("{root}/sound-{index}.wav"), &audio);
        }
        let mut entry = identity(731 + index as u32, "Sound", "Loop", &audio);
        entry.source_bpm = 90.0;
        entries.push(entry);
    }
    let own =
        ["First", "Second"].map(|root| folder.mono(&format!("{root}/own.wav"), &waveform(99, 300)));
    let path = catalogs_dir.join("d1.json");
    fs::write(
        &path,
        serde_json::to_vec(&catalog_file("a", entries.clone())).unwrap(),
    )
    .unwrap();
    let earlier = crate::catalog::Catalogs::open(Some(catalogs_dir.clone()));
    // A replacement catalog is already available before the old scan
    // imports its listings. Each scan must keep its own reading.
    for entry in &mut entries {
        entry.source_bpm = 180.0;
    }
    fs::write(
        &path,
        serde_json::to_vec(&catalog_file("b", entries)).unwrap(),
    )
    .unwrap();
    let later = crate::catalog::Catalogs::open(Some(catalogs_dir));
    let roots = [folder.0.join("First"), folder.0.join("Second")];
    for (catalogs, expected) in [(&earlier, 90.0), (&later, 180.0)] {
        let evidence = crate::source_tempo::SourceTempos::from_catalogs(catalogs);
        let library = Library::open_wav_folders_identified(
            &roots,
            140.0,
            None,
            None,
            |product| catalogs.identities(product),
            |product, id| catalogs.aliases(product, id),
            &evidence,
        )
        .unwrap();
        for native_id in 731..734 {
            let id = Product::D1.global_sample_id(native_id).unwrap();
            assert_eq!(library.sample_by_id(id).unwrap().source_bpm, expected);
        }
        for path in &own {
            let sample = library
                .samples
                .iter()
                .find(|sample| sample.wav_path.as_ref() == Some(path))
                .unwrap();
            assert_eq!(sample.source_bpm, expected);
        }
        assert_eq!(library.folder_tempos.len(), 2);
        assert!(
            library
                .folder_tempos
                .iter()
                .all(|(_, tempo, _)| *tempo == expected)
        );
    }
}

#[test]
fn streaming_probes_match_normalized_audio_and_reject_invalid_samples() {
    let folder = Folder::new();
    for (bits, format) in [
        (8, hound::SampleFormat::Int),
        (16, hound::SampleFormat::Int),
        (24, hound::SampleFormat::Int),
        (32, hound::SampleFormat::Int),
        (32, hound::SampleFormat::Float),
    ] {
        let path = folder.0.join(format!("{bits}-{format:?}.wav"));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: bits,
            sample_format: format,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for value in [-1.0_f32, -0.25, 0.0, 0.5, 0.9] {
            match format {
                hound::SampleFormat::Float => writer.write_sample(value * 2.0).unwrap(),
                hound::SampleFormat::Int => writer
                    .write_sample((value * (1_u64 << (bits - 1)) as f32) as i32)
                    .unwrap(),
            }
        }
        writer.finalize().unwrap();
        let pcm = crate::audio::read_wav(&path).unwrap();
        let samples: Vec<_> = pcm
            .samples
            .iter()
            .map(|v| (v * 32768.0).round() as i16)
            .collect();
        let probe = probe_wav(&path, None).unwrap();
        assert_eq!(probe.frame_count, 5);
        assert_eq!(probe.digest, Some(crate::catalog::pcm_identity(&samples)));
        // Input that ends inside its last frame keeps its whole frames.
        // Input cut off by a frame or more must fail in both callers of the
        // streaming reader.
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        let length = file.metadata().unwrap().len();
        let width = u64::from(bits / 8);
        if width > 1 {
            file.set_len(length - 1).unwrap();
            assert_eq!(crate::audio::read_wav(&path).unwrap().samples.len(), 4);
            assert_eq!(probe_wav(&path, None).unwrap().frame_count, 4);
        }
        file.set_len(length - width - 1).unwrap();
        assert!(crate::audio::read_wav(&path).is_err());
        assert!(probe_wav(&path, None).is_err());
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let path = folder.0.join("nonfinite.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44_100,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        writer.write_sample(bad).unwrap();
        writer.finalize().unwrap();
        assert!(
            crate::audio::read_wav(&path)
                .unwrap_err()
                .contains("finite")
        );
        assert!(probe_wav(&path, None).unwrap_err().contains("finite"));
    }
}

#[test]
fn scans_replace_unversioned_decoder_probes() {
    let folder = Folder::new();
    let path = folder.0.join("extensible.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for value in [0x4000_0000_i32, -0x4000_0000, 0x2000_0000] {
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[20..22], &0xfffe_u16.to_le_bytes());
    bytes[38..40].copy_from_slice(&24_u16.to_le_bytes());
    fs::write(&path, bytes).unwrap();
    let path = fs::canonicalize(path).unwrap();
    let mut legacy = probe_wav(&path, None).unwrap();
    let expected = legacy.digest;
    // Hound reads these left-aligned samples as zero from the low 24 bits.
    let old: Vec<_> = hound::WavReader::open(&path)
        .unwrap()
        .samples::<i32>()
        .map(Result::unwrap)
        .collect();
    assert_eq!(old, [0, 0, 0]);
    legacy.digest = Some(crate::catalog::pcm_identity(&[0, 0, 0]));
    assert_ne!(legacy.digest, expected);
    let mut cache = ScanCache::default();
    cache.insert(&path, ScanCache::stamp(&path).unwrap(), legacy);
    let mut saved = serde_json::to_value(&cache).unwrap();
    for entry in saved["entries"].as_object_mut().unwrap().values_mut() {
        entry.as_object_mut().unwrap().remove("decoder_version");
    }
    let mut cache: ScanCache = serde_json::from_value(saved).unwrap();
    let probes = probe_all(std::slice::from_ref(&path), None, Some(&mut cache)).unwrap();
    assert_eq!(probes[0].as_ref().unwrap().digest, expected);
    assert!(cache.has_changes());
    cache = serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
    probe_all(&[path], None, Some(&mut cache)).unwrap();
    assert!(!cache.has_changes());
}

#[test]
fn folder_scans_report_progress_honour_cancellation_and_reuse_the_cache() {
    let folder = Folder::new();
    let loop_a = folder.mono("Loop/a.wav", &waveform(5, 300));
    folder.mono("Loop/b.wav", &waveform(6, 300));
    folder.mono("c.wav", &waveform(7, 120));
    let progress = ScanProgress::default();
    let mut cache = ScanCache::default();
    let first =
        Library::open_wav_folder_with(&folder.0, 140.0, Some(&progress), Some(&mut cache)).unwrap();
    assert_eq!(progress.counts(), (3, 3));
    assert_eq!(cache.len(), 3);
    assert!(cache.has_changes());
    // Loading persisted entries starts a clean cache.
    cache = serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
    assert!(!cache.has_changes());
    let probe = |library: &Library, path: &Path| {
        let sample = library
            .samples
            .iter()
            .find(|s| s.wav_path.as_deref() == Some(path))
            .unwrap();
        (sample.id, sample.frame_count, sample.digest)
    };
    // A second scan takes every file from the cache and builds the same catalog.
    let again = ScanProgress::default();
    let second =
        Library::open_wav_folder_with(&folder.0, 140.0, Some(&again), Some(&mut cache)).unwrap();
    assert_eq!(again.counts(), (3, 3));
    assert_eq!(probe(&second, &loop_a), probe(&first, &loop_a));
    assert_eq!(second.samples.len(), 3);
    assert!(!cache.has_changes());
    // A changed file is probed again; its stale entry does not survive.
    let replaced = waveform(8, 200);
    folder.mono("Loop/a.wav", &replaced);
    let third = Library::open_wav_folder_with(&folder.0, 140.0, None, Some(&mut cache)).unwrap();
    assert_eq!(probe(&third, &loop_a).1, 200);
    assert!(cache.has_changes());
    assert_eq!(
        probe(&third, &loop_a).2,
        Some(crate::catalog::pcm_identity(&replaced))
    );
    // Cancelling before the scan fails it and leaves the library untouched.
    let cancelled = ScanProgress::default();
    cancelled.cancel();
    let mut untouched = Library::empty();
    let error = untouched
        .import_wav_folder_with(&folder.0, 140.0, Some(&cancelled), None)
        .unwrap_err();
    assert!(error.contains("cancelled"), "{error}");
    assert!(untouched.samples.is_empty());
    // A cache file with the wrong shape does not parse.
    let text = serde_json::to_string(&cache).unwrap();
    let restored: ScanCache = serde_json::from_str(&text).unwrap();
    assert_eq!(restored.len(), cache.len());
    assert!(serde_json::from_str::<ScanCache>("{\"entries\":1}").is_err());
}

#[test]
fn a_scan_forgets_cached_files_when_a_library_folder_becomes_empty() {
    let folder = Folder::new();
    let removed = folder.mono("First/kick.wav", &[1, 2, 3]);
    folder.mono("Second/snare.wav", &[4, 5, 6]);
    let roots = [folder.0.join("First"), folder.0.join("Second")];
    let mut cache = ScanCache::default();
    Library::open_wav_folders_with(&roots, 140.0, None, Some(&mut cache)).unwrap();
    assert_eq!(cache.len(), 2);
    fs::remove_file(removed).unwrap();
    cache = serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
    let library = Library::open_wav_folders_with(&roots, 140.0, None, Some(&mut cache)).unwrap();
    assert_eq!(library.samples.len(), 1);
    assert_eq!(library.unloaded_roots().len(), 1);
    assert!(library.unloaded_roots()[0].1.contains("no WAV"));
    assert_eq!(cache.len(), 1);
    assert!(cache.has_changes());
}

#[test]
fn a_scan_keeps_listed_files_and_forgets_cached_files_that_moved() {
    let folder = Folder::new();
    for index in 0..600 {
        folder.mono(&format!("One/Pack {}/{index}.wav", index % 7), &[1, 2, 3]);
    }
    for index in 0..400 {
        folder.mono(&format!("Two/{index}.wav"), &[4, 5, 6]);
    }
    let roots = [folder.0.join("One"), folder.0.join("Two")];
    let mut cache = ScanCache::default();
    let library = Library::open_wav_folders_with(&roots, 140.0, None, Some(&mut cache)).unwrap();
    assert_eq!(library.samples.len(), 1000);
    assert_eq!(cache.len(), 1000);
    // Files moved out of a scanned folder leave the cache; the probes of
    // a folder outside this scan stay.
    let elsewhere = folder.mono("Elsewhere/kept.wav", &[7, 8, 9]);
    Library::open_wav_folder_with(elsewhere.parent().unwrap(), 140.0, None, Some(&mut cache))
        .unwrap();
    fs::rename(folder.0.join("Two"), folder.0.join("Moved")).unwrap();
    fs::create_dir_all(folder.0.join("Two")).unwrap();
    folder.mono("Two/new.wav", &[1, 1, 1]);
    cache = serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
    Library::open_wav_folders_with(&roots, 140.0, None, Some(&mut cache)).unwrap();
    assert!(cache.has_changes());
    assert_eq!(cache.len(), 600 + 1 + 1);
}

#[test]
fn standalone_folder_reads_nested_wavs_and_skips_unreadable_files() {
    let folder = Folder::new();
    folder.wave("Bass/my original.WAV", 22_050, 220);
    folder.wave("another.wav", 48_000, 480);
    let mut library = Library::open_wav_folder(&folder.0, 160.0).unwrap();
    assert!(library.archive_path.as_os_str().is_empty());
    assert!(library.bindings().is_empty());
    assert_eq!(library.wav_roots(), [fs::canonicalize(&folder.0).unwrap()]);
    assert_eq!(library.samples.len(), 2);
    let bass = library
        .samples
        .iter()
        .find(|s| s.name == "my original")
        .unwrap();
    assert_eq!(bass.group_name, "Bass");
    assert_eq!(bass.source_bpm, 160.0);
    assert_eq!(library.read_pcm(bass.id).unwrap().frames(), 220);
    assert_eq!(library.imported_samples().len(), 2);
    assert!(library.skipped_waves().is_empty());
    let before = library.imported_samples();
    fs::write(folder.0.join("Bass/._my original.WAV"), b"Mac OS metadata").unwrap();
    fs::write(folder.0.join("invalid.wav"), b"not audio").unwrap();
    // A header without frames, as some sample packs ship.
    let mut empty = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
    empty.extend_from_slice(&44_100u32.to_le_bytes());
    empty.extend_from_slice(&88_200u32.to_le_bytes());
    empty.extend_from_slice(b"\x02\0\x10\0data\0\0\0\0");
    fs::write(folder.0.join("Bass/empty.wav"), empty).unwrap();
    library.import_wav_folder(&folder.0, 160.0).unwrap();
    assert_eq!(library.imported_samples(), before);
    let skipped: Vec<_> = library
        .skipped_waves()
        .iter()
        .map(|(path, _)| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(skipped, ["empty.wav", "invalid.wav"]);
    assert!(library.import_wav_folder(&folder.0, f64::NAN).is_err());

    let unreadable = Folder::new();
    fs::write(unreadable.0.join("invalid.wav"), b"not audio").unwrap();
    let error = Library::open_wav_folder(&unreadable.0, 140.0).unwrap_err();
    assert!(error.contains("no readable WAV samples"), "{error}");
    assert!(error.contains("invalid.wav"), "{error}");
}

#[test]
fn library_folders_merge_into_one_browser_with_one_progress_total() {
    let folder = Folder::new();
    let first = folder.mono("First/Drums/kick.wav", &waveform(1, 300));
    let second = folder.mono("Second/Drums/kick.wav", &waveform(2, 300));
    let top = folder.mono("Second/top.wav", &waveform(3, 120));
    let roots = [folder.0.join("First"), folder.0.join("Second")];
    let progress = ScanProgress::default();
    let mut cache = ScanCache::default();
    let library =
        Library::open_wav_folders_with(&roots, 140.0, Some(&progress), Some(&mut cache)).unwrap();
    assert_eq!(progress.counts(), (3, 3));
    assert_eq!(cache.len(), 3);
    assert_eq!(
        library.wav_roots(),
        [
            fs::canonicalize(&roots[0]).unwrap(),
            fs::canonicalize(&roots[1]).unwrap()
        ]
    );
    assert!(library.unloaded_roots().is_empty());
    let mut paths: Vec<_> = library
        .samples
        .iter()
        .filter_map(|sample| sample.wav_path.clone())
        .collect();
    paths.sort();
    let mut expected = vec![first.clone(), second.clone(), top.clone()];
    expected.sort();
    assert_eq!(paths, expected);
    let ids: HashSet<_> = library.samples.iter().map(|sample| sample.id).collect();
    assert_eq!(ids.len(), 3, "Each folder's sounds keep their own IDs.");
    let group = |path: &Path| {
        library
            .samples
            .iter()
            .find(|sample| sample.wav_path.as_deref() == Some(path))
            .unwrap()
            .group_name
            .clone()
    };
    assert_eq!(group(&first), "Drums");
    assert_eq!(
        group(&top),
        "Wave",
        "A file at a library's top is a plain WAV."
    );
    assert_eq!(
        library.wav_root_of(&second),
        Some(library.wav_roots()[1].as_path())
    );
}

#[test]
fn an_unreadable_entry_inside_a_folder_is_skipped_and_the_rest_loads() {
    let folder = Folder::new();
    folder.wave("Kept/kick.wav", 44_100, 300);
    // A link whose target is gone cannot be read, as a protected system
    // folder cannot. Windows allows links only with Developer Mode.
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(folder.0.join("Gone"), folder.0.join("Lost"));
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(folder.0.join("Gone"), folder.0.join("Lost"));
    if linked.is_err() {
        eprintln!("skipped: this system does not allow links");
        return;
    }
    let library = Library::open_wav_folder(&folder.0, 140.0).unwrap();
    assert_eq!(library.samples.len(), 1);
    let skipped = library.skipped_waves();
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].0.ends_with("Lost"), "{:?}", skipped[0]);
}

#[test]
fn an_unreadable_library_folder_leaves_the_others_loaded() {
    let folder = Folder::new();
    let kept = folder.mono("Kept/kick.wav", &waveform(1, 300));
    fs::create_dir_all(folder.0.join("Empty")).unwrap();
    let roots = [
        folder.0.join("Missing"),
        folder.0.join("Kept"),
        folder.0.join("Empty"),
    ];
    let library = Library::open_wav_folders_with(&roots, 140.0, None, None).unwrap();
    assert_eq!(library.samples.len(), 1);
    assert_eq!(library.samples[0].wav_path.as_deref(), Some(kept.as_path()));
    // The list keeps every folder in order; the two that failed say why.
    assert_eq!(library.wav_roots().len(), 3);
    assert_eq!(library.wav_roots()[0], roots[0]);
    let unloaded = library.unloaded_roots();
    assert_eq!(unloaded.len(), 2);
    assert_eq!(unloaded[0].0, roots[0]);
    assert!(unloaded[0].1.contains("Cannot open"), "{}", unloaded[0].1);
    assert_eq!(unloaded[1].0, fs::canonicalize(&roots[2]).unwrap());
    assert!(unloaded[1].1.contains("no WAV"), "{}", unloaded[1].1);
    // With no folder left, the scan fails and names each one.
    let error =
        Library::open_wav_folders_with(&[roots[0].clone(), roots[2].clone()], 140.0, None, None)
            .unwrap_err();
    assert!(
        error.starts_with("No library folder could be loaded."),
        "{error}"
    );
    assert!(
        error.contains("Missing") && error.contains("Empty"),
        "{error}"
    );
}

#[test]
fn library_folder_lists_hold_one_to_nine_separate_folders() {
    let folder = Folder::new();
    let roots: Vec<PathBuf> = (0..=MAX_LIBRARY_FOLDERS)
        .map(|index| {
            folder.mono(&format!("Library {index}/a.wav"), &waveform(1, 100));
            folder.0.join(format!("Library {index}"))
        })
        .collect();
    check_library_folders(&roots[..MAX_LIBRARY_FOLDERS]).unwrap();
    let error = check_library_folders(&roots).unwrap_err();
    assert!(error.contains("up to 9"), "{error}");
    assert!(Library::open_wav_folders_with(&roots, 140.0, None, None).is_err());
    assert!(check_library_folders(&[]).is_err());
    let same = [roots[0].clone(), folder.0.join("Library 0/.")];
    assert!(
        check_library_folders(&same)
            .unwrap_err()
            .contains("already")
    );
    let nested = [folder.0.clone(), roots[1].clone()];
    assert!(
        check_library_folders(&nested)
            .unwrap_err()
            .contains("inside")
    );
}

#[test]
fn library_folder_lists_reject_offline_aliases() {
    let relative = PathBuf::from(format!(
        "tmp/unavailable-library-{}-{}",
        std::process::id(),
        NEXT_FOLDER.fetch_add(1, Ordering::Relaxed)
    ));
    let absolute = std::path::absolute(&relative).unwrap();
    assert!(!absolute.exists());
    assert!(
        check_library_folders(&[relative, absolute])
            .unwrap_err()
            .contains("already")
    );
}

#[test]
fn library_folder_lists_reject_missing_children() {
    let folder = Folder::new();
    let child = folder.0.join("Unavailable/Pack");
    assert!(!child.exists());
    assert!(
        check_library_folders(&[folder.0.clone(), child])
            .unwrap_err()
            .contains("inside")
    );
}
