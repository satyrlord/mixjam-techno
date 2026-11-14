use super::*;

#[test]
fn a_palette_larger_than_the_former_cap_validates_as_project_imports() {
    use crate::engine::{ImportedSample, Project};
    // The former limit was 200,000 imports; these are records, not files.
    let count = 200_001;
    let project = Project {
        imported_samples: (0..count)
            .map(|index| ImportedSample {
                id: IMPORTED_ID_START + index as u32,
                name: "Sample".into(),
                group_name: "Wave".into(),
                path: PathBuf::from(format!("sample-{index}.wav")),
                source_bpm: 140.0,
                source_sample_rate: Some(44_100),
                origin: None,
            })
            .collect(),
        ..Project::default()
    };
    project.validate().unwrap();
    // Imported IDs stay above every product region.
    let last = project.imported_samples.last().unwrap().id;
    assert!(Product::sample_key(last).is_none());
}

#[test]
fn resolving_a_song_keeps_supplemental_native_samples() {
    let folder = Folder::new();
    let metadata = crate::catalog::identities(Product::D1);
    if metadata.len() < 3 {
        // With no local catalog, the product still preserves unresolved
        // native placements instead of needing the optional data files.
        let id = Product::D1.global_sample_id(731).unwrap();
        let mut editor = crate::engine::Editor::new(crate::engine::Project::default());
        editor.place(id, 0, 0, 256).unwrap();
        let (project, _) =
            crate::project_io::resolve(editor.project().clone(), &Library::empty()).unwrap();
        assert_eq!(project.clips[0].sample_id, id);
        assert_eq!(project.missing_samples.len(), 1);
        assert_eq!(project.missing_samples[0].id, id);
        return;
    }
    let write = |root: &str, entry: &crate::catalog::WaveMetadata| {
        let code = entry.code.replace('\\', "/");
        let name = Path::new(&code).file_stem().unwrap().to_string_lossy();
        folder.wave(
            &format!("{root}/{name}.wav"),
            22_050,
            entry.frame_count as usize / 2,
        );
        Product::D1.global_sample_id(entry.native_id).unwrap()
    };
    let mut ids = vec![write("First", &metadata[0]), write("Second", &metadata[1])];
    let roots = [folder.0.join("First"), folder.0.join("Second")];
    let library = Library::open_wav_folders_with(&roots, 140.0, None, None).unwrap();
    assert!(library.is_sparse_wav_product(Product::D1));
    for id in &ids {
        assert!(library.sample_by_id(*id).is_some());
    }
    // The saved provider can recover a sample added since the scan,
    // while retaining the sample already loaded from the other folder.
    ids.push(write("First", &metadata[2]));
    assert!(library.sample_by_id(ids[2]).is_none());
    let mut editor = crate::engine::Editor::new(crate::engine::Project::default());
    for (track, id) in ids.iter().enumerate() {
        editor.place(*id, track, 0, 256).unwrap();
    }
    let mut project = editor.project().clone();
    project.libraries = library.bindings().to_vec();
    let (resolved, _) = crate::project_io::resolve(project, &library).unwrap();
    assert!(
        resolved.missing_samples.is_empty(),
        "{:?}",
        resolved.missing_samples
    );
}

#[test]
fn relocated_mixjam_references_include_copies_omitted_from_the_palette() {
    let folder = Folder::new();
    let audio = waveform(1, 300);
    folder.mono("First/kit.wav", &audio);
    let copy = folder.mono("Second/Perc/kick.wav", &audio);
    let other = folder.mono("Second/Other/kick.wav", &audio);
    let metadata = [identity(1, "kit", "Drum", &audio)];
    let library = Library::open_wav_folders_identified(
        &[folder.0.join("First"), folder.0.join("Second")],
        140.0,
        None,
        None,
        |product| {
            if product == Product::T2 {
                &metadata
            } else {
                &[]
            }
        },
        |_, _| &[],
        &crate::source_tempo::SourceTempos::default(),
    )
    .unwrap();
    assert_eq!(library.duplicate_copies().len(), 2);
    assert!(library.samples.iter().all(|sample| {
        sample
            .wav_path
            .as_ref()
            .is_none_or(|path| path != &copy && path != &other)
    }));
    assert_eq!(
        library.resolve_wav_reference("Old/Perc/kick.wav").unwrap(),
        copy
    );
    let mut resolver = crate::mixjam::ReferenceResolver::new(&library);
    assert_eq!(
        resolver.resolve_candidates("Old/Perc/kick.wav").unwrap(),
        vec![copy.clone()]
    );
    let mut candidates = vec![copy, other];
    candidates.sort();
    assert_eq!(
        resolver.resolve_candidates("Old/kick.wav").unwrap(),
        candidates
    );
}

#[test]
fn songs_record_a_sample_folder_product_by_the_folder_that_holds_it() {
    let folder = Folder::new();
    folder.wave("_original/Rave/Bass/soft-dust-06.wav", 22_050, 100);
    folder.wave("_original/Rave/Drum/kick-01.wav", 22_050, 100);
    folder.wave("Hardcore/Bass/HARDCORE_BASS001_180_C_SL3.WAV", 44_100, 100);
    let mut library = Library::open_wav_folder(&folder.0, 140.0).unwrap();
    let entry = |native_id: u32, name: &str, group: &str| crate::catalog::WaveMetadata {
        native_id,
        code: format!("code-{native_id}"),
        label_key: crate::catalog::label_key(name),
        group_name: group.into(),
        bars: 1,
        source_bpm: 180.0,
        frame_count: 200,
        pcm_digest: None,
    };
    library.remove_product(Product::T1);
    let root = library.wav_roots()[0].clone();
    library
        .register_wav_product(
            Product::T1,
            &root,
            &[
                entry(731, "soft dust 6", "bass"),
                entry(851, "kick 1", "drum"),
            ],
        )
        .unwrap();
    let binding = LibraryBinding {
        product: Product::T1,
        root: root.clone(),
    };
    let bass = Product::T1.global_sample_id(731).unwrap();
    let kick = Product::T1.global_sample_id(851).unwrap();
    let rave = root.join("_original").join("Rave");
    let used = |ids: &[u32]| ids.iter().copied().collect::<HashSet<_>>();
    assert_eq!(
        library.song_binding(&binding, &used(&[bass, kick])).root,
        rave
    );
    // Only the files that the song uses decide the folder.
    assert_eq!(
        library.song_binding(&binding, &used(&[bass])).root,
        rave.join("Bass")
    );
    assert_eq!(library.song_binding(&binding, &used(&[])), binding);
    // A provider that is not this library's WAV provider is recorded as it is.
    let elsewhere = LibraryBinding {
        product: Product::T2,
        root: root.join("elsewhere"),
    };
    assert_eq!(
        library.song_binding(&elsewhere, &used(&[bass, kick])),
        elsewhere
    );
}

#[test]
fn moved_wave_resolution_preserves_relative_paths_and_returns_stable_candidates() {
    let folder = Folder::new();
    let bass = folder.wave("Bass/sound.wav", 44_100, 10);
    let drum = folder.wave("Drum/sound.wav", 44_100, 10);
    let only = folder.wave("only.wav", 44_100, 10);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    assert_eq!(
        library
            .resolve_wav_reference(r"C:\old\Bass\SOUND.WAV")
            .unwrap(),
        bass
    );
    assert_eq!(
        library.resolve_wav_reference("old/place/only.wav").unwrap(),
        only
    );
    assert_eq!(
        library.find_wav_candidates("sound.wav").unwrap(),
        vec![bass.clone(), drum]
    );
    assert_eq!(library.resolve_wav_reference("sound.wav").unwrap(), bass);
    assert!(library.resolve_wav_reference("../only.wav").is_err());
    assert!(library.resolve_wav_reference("missing.wav").is_err());
}

#[test]
fn palette_restore_keeps_project_ids_and_does_not_decode_unused_files_again() {
    let folder = Folder::new();
    let first = folder.wave("first.wav", 44_100, 10);
    let second = folder.wave("second.wav", 44_100, 10);
    let mut palette = Library::empty();
    palette.import_wav_folder(&folder.0, 160.0).unwrap();
    let mut project = Library::empty();
    let used = project
        .import_wav_with_tempo(&second, Some("Project name"), "Wave", 140.0)
        .unwrap();
    fs::remove_file(first).unwrap();
    let added = project.restore_wav_palette(&palette).unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].id, used.id + 1);
    assert_eq!(added[0].source_bpm, 160.0);
    assert_eq!(project.sample_by_id(used.id).unwrap().name, "Project name");
    assert!(project.restore_wav_palette(&palette).unwrap().is_empty());
    assert!(project.read_pcm(added[0].id).is_err());
}

#[test]
fn unchanged_imports_keep_their_catalog_entries_without_decoding_again() {
    let folder = Folder::new();
    let path = folder.wave("loop.wav", 44_100, 10);
    let mut library = Library::empty();
    let imported = library
        .import_wav_with_tempo(&path, Some("Loop"), "Wave", 140.0)
        .unwrap();
    // Undo and redo re-apply identical metadata: the file is not read again.
    fs::remove_file(&path).unwrap();
    library
        .apply_imports(std::slice::from_ref(&imported))
        .unwrap();
    assert_eq!(library.sample_by_id(imported.id).unwrap().frame_count, 10);
    // Changed metadata needs the file and fails atomically without it.
    let mut retimed = imported.clone();
    retimed.source_bpm = 120.0;
    assert!(library.apply_imports(&[retimed]).is_err());
    let mut renamed = imported;
    renamed.name = "Other".into();
    assert!(library.apply_imports(&[renamed]).is_err());
    assert_eq!(library.sample_by_id(1_000_000).unwrap().name, "Loop");
}

#[test]
fn imported_wav_roundtrips_and_missing_file_is_atomic() {
    let folder = Folder::new();
    let path = folder.0.join("sample.wav");
    crate::audio::Pcm {
        sample_rate: 22050,
        channels: 2,
        samples: vec![0.25; 440],
    }
    .write_wav(&path)
    .unwrap();
    let mut library = Library::empty();
    let imported = library
        .import_wav_with_tempo(&path, Some("My sample"), "Wave", 140.0)
        .unwrap();
    assert_eq!(imported.id, 1_000_000);
    let sample = library.sample_by_id(imported.id).unwrap();
    assert_eq!(sample.frame_count, 220);
    assert_eq!(sample.sample_rate, 22050);
    assert_eq!(sample.channels, 2);
    assert_eq!(library.read_pcm(imported.id).unwrap().frames(), 220);
    assert_eq!(
        library
            .import_wav_with_tempo(&path, None, "Wave", 140.0)
            .unwrap()
            .id,
        imported.id
    );
    let mut project = crate::engine::Project::default();
    project.imported_samples.push(imported.clone());
    let saved = serde_json::to_vec(&project).unwrap();
    let restored: crate::engine::Project = serde_json::from_slice(&saved).unwrap();
    library.apply_imports(&restored.imported_samples).unwrap();
    let mut missing = imported;
    missing.path = folder.0.join("missing.wav");
    assert!(library.apply_imports(&[missing]).is_err());
    assert_eq!(library.samples.len(), 1);
    assert_eq!(library.samples[0].name, "My sample");
}

#[test]
fn folders_import_with_their_own_tempo_and_report_it() {
    let folder = Folder::new();
    for (name, frames) in [("a", 75_600), ("b", 151_200), ("c", 75_600)] {
        folder.wave(&format!("Dance/{name}.wav"), 44_100, frames);
    }
    for (name, frames) in [("a", 58_800), ("b", 117_600), ("c", 117_600)] {
        folder.wave(&format!("Rave/{name}.wav"), 44_100, frames);
    }
    folder.wave("Hits/one.wav", 44_100, 9_001);
    folder.wave("Hits/two.wav", 44_100, 12_345);
    folder.wave("Hits/three.wav", 44_100, 23_456);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 120.0).unwrap();
    let tempo = |relative: &str| {
        let path = fs::canonicalize(folder.0.join(relative)).unwrap();
        library
            .samples
            .iter()
            .find(|s| s.wav_path.as_ref() == Some(&path))
            .unwrap()
            .source_bpm
    };
    assert_eq!(tempo("Dance/a.wav"), 140.0);
    assert_eq!(tempo("Rave/b.wav"), 180.0);
    assert_eq!(tempo("Hits/one.wav"), 120.0);
    let reported: Vec<_> = library
        .folder_tempos()
        .iter()
        .map(|(path, bpm, count)| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                *bpm,
                *count,
            )
        })
        .collect();
    assert_eq!(
        reported,
        vec![
            ("Dance".to_string(), 140.0, 3),
            ("Rave".to_string(), 180.0, 3)
        ]
    );
}
