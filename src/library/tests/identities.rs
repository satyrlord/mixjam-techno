use super::*;

#[test]
fn compatible_release_aliases_identify_wavs_without_comparable_digests() {
    let folder = Folder::new();
    let original = waveform(1, 300);
    let first = identity(731, "First name", "Bass", &original);
    let mut second = first.clone();
    second.code = "SECOND-CODE".into();
    second.label_key = crate::catalog::label_key("Second name");
    let catalogs_dir = folder.0.join("catalogs");
    fs::create_dir(&catalogs_dir).unwrap();
    for (name, entry) in [("a", first), ("b", second)] {
        fs::write(
            catalogs_dir.join(format!("{name}.json")),
            serde_json::to_vec(&catalog_file(name, vec![entry])).unwrap(),
        )
        .unwrap();
    }
    let catalogs = crate::catalog::Catalogs::open(Some(catalogs_dir));
    assert!(catalogs.problems().is_empty());
    let selected = folder.wave("waves/SECOND-CODE.wav", 22_050, 150);
    let library = Library::open_wav_folders_identified(
        &[folder.0.join("waves")],
        140.0,
        None,
        None,
        |product| catalogs.identities(product),
        |product, id| catalogs.aliases(product, id),
        &crate::source_tempo::SourceTempos::from_catalogs(&catalogs),
    )
    .unwrap();
    let id = Product::D1.global_sample_id(731).unwrap();
    assert_eq!(
        library.sample_by_id(id).unwrap().wav_path.as_ref(),
        Some(&selected)
    );
}

#[test]
fn later_release_digest_prevents_wrong_named_audio_from_matching() {
    let folder = Folder::new();
    let audio = waveform(1, 300);
    let strong = identity(731, "Bass", "Bass", &audio);
    let mut weak = strong.clone();
    weak.pcm_digest = None;
    let catalogs_dir = folder.0.join("catalogs");
    fs::create_dir(&catalogs_dir).unwrap();
    for (name, entry) in [("a", weak), ("b", strong)] {
        fs::write(
            catalogs_dir.join(format!("{name}.json")),
            serde_json::to_vec(&catalog_file(name, vec![entry])).unwrap(),
        )
        .unwrap();
    }
    let catalogs = crate::catalog::Catalogs::open(Some(catalogs_dir));
    assert!(catalogs.problems().is_empty());
    let wrong = folder.mono("waves/CODE731.wav", &waveform(2, 300));
    let library = Library::open_wav_folders_identified(
        &[folder.0.join("waves")],
        140.0,
        None,
        None,
        |product| catalogs.identities(product),
        |product, id| catalogs.aliases(product, id),
        &crate::source_tempo::SourceTempos::from_catalogs(&catalogs),
    )
    .unwrap();
    let id = Product::D1.global_sample_id(731).unwrap();
    assert!(library.sample_by_id(id).is_none());
    assert_eq!(
        library.unverified_waves().collect::<Vec<_>>(),
        [(id, wrong.as_path())]
    );
}

#[test]
fn wav_supplements_do_not_reclassify_or_rebind_an_original_provider() {
    let folder = Folder::new();
    let original_root = folder.0.join("original");
    fs::create_dir(&original_root).unwrap();
    let wav = folder.mono("extra/supplement.wav", &waveform(3, 300));
    let original_id = Product::D1.global_sample_id(731).unwrap();
    let wav_id = Product::D1.global_sample_id(732).unwrap();
    let sample = |id, wav_path| Sample {
        id,
        code: format!("CODE{id}"),
        name: "Sound".into(),
        bars: 1,
        group_name: "Bass".into(),
        offset: 0,
        length: 600,
        frame_count: 300,
        sample_rate: 44_100,
        source_bpm: 140.0,
        channels: 1,
        wav_path,
        digest: None,
    };
    let mut library = Library::fixture(vec![sample(original_id, None)]);
    library.sources.insert(
        original_id,
        AudioSource::Archive {
            path: original_root.join("archive"),
        },
    );
    let binding = LibraryBinding {
        product: Product::D1,
        root: original_root,
    };
    library.bindings.push(binding.clone());
    let mut supplement = Library::fixture(vec![sample(wav_id, Some(wav.clone()))]);
    supplement.sources.insert(
        wav_id,
        AudioSource::Wave {
            path: wav,
            origin: None,
        },
    );
    library.supplement_loaded(Product::D1, &supplement);

    assert!(library.has_original_provider(Product::D1));
    assert!(library.is_wav_product(Product::D1));
    assert!(!library.is_sparse_wav_product_with_count(Product::D1, 100));
    assert_eq!(
        library.song_binding(&binding, &HashSet::from([original_id, wav_id])),
        binding
    );
    assert_eq!(
        library.describe_missing(Product::D1, 99_999),
        "D1 native sample 99999 is missing from its library."
    );
}

#[test]
fn selected_loaded_supplements_keep_current_identities_and_binding() {
    let folder = Folder::new();
    let current_path = folder.mono("current.wav", &waveform(1, 300));
    let old_path = folder.mono("old.wav", &waveform(2, 300));
    let kept_path = folder.mono("kept.wav", &waveform(3, 300));
    let skipped_path = folder.mono("skipped.wav", &waveform(4, 300));
    let foreign_path = folder.mono("foreign.wav", &waveform(5, 300));
    let current_id = Product::D1.global_sample_id(731).unwrap();
    let kept_id = Product::D1.global_sample_id(732).unwrap();
    let skipped_id = Product::D1.global_sample_id(733).unwrap();
    let foreign_id = Product::D4.global_sample_id(1).unwrap();
    let sample = |id, path: PathBuf| Sample {
        id,
        code: format!("CODE{id}"),
        name: "Sound".into(),
        bars: 1,
        group_name: "Bass".into(),
        offset: 0,
        length: 600,
        frame_count: 300,
        sample_rate: 44_100,
        source_bpm: 140.0,
        channels: 1,
        wav_path: Some(path),
        digest: None,
    };
    let mut library = Library::fixture(vec![sample(current_id, current_path.clone())]);
    let binding = LibraryBinding {
        product: Product::D1,
        root: folder.0.join("provider"),
    };
    library.bindings.push(binding.clone());
    library.unverified.insert(kept_id, old_path.clone());
    let mut loaded = Library::fixture(vec![
        sample(current_id, old_path),
        sample(kept_id, kept_path.clone()),
        sample(skipped_id, skipped_path),
        sample(foreign_id, foreign_path),
    ]);
    loaded.verified.insert(kept_id);
    library.supplement_loaded_ids(
        Product::D1,
        &loaded,
        &HashSet::from([current_id, kept_id, foreign_id]),
    );
    assert_eq!(
        library.sample_by_id(current_id).unwrap().wav_path.as_ref(),
        Some(&current_path)
    );
    assert_eq!(
        library.sample_by_id(kept_id).unwrap().wav_path.as_ref(),
        Some(&kept_path)
    );
    assert!(library.sample_by_id(skipped_id).is_none());
    assert!(library.sample_by_id(foreign_id).is_none());
    assert!(library.verified.contains(&kept_id));
    assert!(!library.unverified.contains_key(&kept_id));
    assert_eq!(library.bindings(), &[binding]);
}

#[test]
fn audio_digests_identify_wavs_regardless_of_name_folder_offset_or_final_frame() {
    let folder = Folder::new();
    let original = waveform(1, 100);
    let mut shifted: Vec<i16> = original.iter().map(|s| s + 5).collect();
    shifted[99] = 0;
    let other = waveform(2, 100);
    let first = folder.mono("Misc/whatever-07.wav", &original);
    let second = folder.mono("Other/other.wav", &shifted);
    let named = folder.mono("Bass/hihat-hard.wav", &other);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    let own_name = |path: &PathBuf| {
        library
            .samples
            .iter()
            .find(|sample| sample.wav_path.as_ref() == Some(path))
            .unwrap()
            .name
            .clone()
    };
    let first_name = own_name(&first);
    let metadata = [
        identity(1, "hihat hard", "Bass", &original),
        identity(2, "something else", "Loop", &original),
        identity(3, "hihat hard", "Bass", &waveform(3, 100)),
    ];
    assert_eq!(
        library
            .register_wav_product(Product::T2, &folder.0, &metadata)
            .unwrap(),
        2
    );
    // Identical audio under two identities takes one file each, in order.
    assert_eq!(
        library.sample_by_id(1).unwrap().wav_path.as_ref(),
        Some(&first)
    );
    assert_eq!(
        library.sample_by_id(2).unwrap().wav_path.as_ref(),
        Some(&second)
    );
    // Catalogs hold no names: the sound keeps its own file's name.
    assert_eq!(library.sample_by_id(1).unwrap().name, first_name);
    assert_eq!(library.verified_count(), 2);
    // The name, category, and duration agree, but the audio does not.
    assert!(library.sample_by_id(3).is_none());
    assert_eq!(
        library.unverified_waves().collect::<Vec<_>>(),
        vec![(3, named.as_path())]
    );
    let message = library.describe_missing(Product::T2, 3);
    assert!(message.contains("hihat-hard.wav") && message.contains("differs"));
    assert!(!library.describe_missing(Product::T2, 4).contains("differs"));
    assert_eq!(library.read_pcm(2).unwrap().frames(), 100);
}

#[test]
fn digest_matching_requires_comparable_rate_and_enough_frames() {
    let original = waveform(3, 75_600);
    for (name, rate, audio, expected_count) in [
        ("wrong-rate", 22_050, original.clone(), 0),
        (
            "resampled",
            22_050,
            original.iter().step_by(2).copied().collect(),
            1,
        ),
    ] {
        let folder = Folder::new();
        crate::audio::Pcm {
            sample_rate: rate,
            channels: 1,
            samples: audio.iter().map(|s| f32::from(*s) / 32768.0).collect(),
        }
        .write_wav(&folder.0.join("original.wav"))
        .unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        // Older caches can contain a digest for a non-native sample rate.
        library.samples[0].digest = Some(crate::catalog::pcm_identity(&audio));
        assert_eq!(
            library
                .register_wav_product(
                    Product::T2,
                    &folder.0,
                    &[identity(1, "original", "Wave", &original)],
                )
                .unwrap(),
            expected_count,
            "{name}",
        );
        assert_eq!(library.verified_count(), 0, "{name}");
    }
    let folder = Folder::new();
    folder.mono("unrelated.wav", &[10, 20]);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    library.samples[0].digest = Some(crate::catalog::pcm_identity(&[10, 20]));
    assert_eq!(
        library
            .register_wav_product(
                Product::T2,
                &folder.0,
                &[identity(1, "original", "Wave", &[100])],
            )
            .unwrap(),
        0,
    );
}

#[test]
fn identities_that_outnumber_identical_files_share_the_last_file() {
    let folder = Folder::new();
    let original = waveform(4, 80);
    let first = folder.mono("a.wav", &original);
    let second = folder.mono("b.wav", &original);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    let metadata = [
        identity(10, "one", "Loop", &original),
        identity(11, "two", "Loop", &original),
        identity(12, "three", "Loop", &original),
    ];
    assert_eq!(
        library
            .register_wav_product(Product::D3, &folder.0, &metadata)
            .unwrap(),
        3
    );
    let path = |native: u32| {
        library
            .sample_by_id(Product::D3.global_sample_id(native).unwrap())
            .unwrap()
            .wav_path
            .clone()
            .unwrap()
    };
    assert_eq!(path(10), first);
    assert_eq!(path(11), second);
    assert_eq!(path(12), second);
    assert_eq!(library.verified_count(), 3);
    assert!(library.unverified_waves().next().is_none());
}

#[test]
fn one_product_takes_identities_from_every_library_folder_in_list_order() {
    let folder = Folder::new();
    let (base_audio, kit_audio, shared_audio) =
        (waveform(1, 300), waveform(2, 300), waveform(3, 300));
    let base = folder.mono("Base/loop/base.wav", &base_audio);
    let kit = folder.mono("Kits/rap/kit.wav", &kit_audio);
    let base_shared = folder.mono("Base/loop/shared.wav", &shared_audio);
    let kit_shared = folder.mono("Kits/rap/shared copy.wav", &shared_audio);
    let metadata = [
        identity(731, "base", "loop", &base_audio),
        identity(3400, "kit", "rap", &kit_audio),
        identity(3401, "shared", "rap", &shared_audio),
    ];
    let roots = [
        fs::canonicalize(folder.0.join("Base")).unwrap(),
        fs::canonicalize(folder.0.join("Kits")).unwrap(),
    ];
    let path = |library: &Library, native_id: u32| {
        library
            .sample_by_id(Product::D1.global_sample_id(native_id).unwrap())
            .and_then(|sample| sample.wav_path.clone())
    };
    for (order, provider, shared) in [
        ([0, 1], &roots[0], &base_shared),
        ([1, 0], &roots[1], &kit_shared),
    ] {
        let ordered = [roots[order[0]].clone(), roots[order[1]].clone()];
        let mut library = Library::open_wav_folders_with(&ordered, 140.0, None, None).unwrap();
        library.remove_product(Product::D1);
        library
            .register_wav_folders(Product::D1, &ordered, &metadata)
            .unwrap();
        // The second folder's identities resolve too.
        assert_eq!(path(&library, 731).as_ref(), Some(&base));
        assert_eq!(path(&library, 3400).as_ref(), Some(&kit));
        // The first folder in the list keeps priority for shared audio.
        assert_eq!(path(&library, 3401).as_ref(), Some(shared));
        // One provider, recorded at the first folder that identified any.
        let bindings: Vec<_> = library
            .bindings()
            .iter()
            .filter(|binding| binding.product == Product::D1)
            .collect();
        assert_eq!(bindings.len(), 1);
        assert_eq!(&bindings[0].root, provider);
        assert_eq!(library.source_root(Product::D1), Some(provider.as_path()));
    }
}

#[test]
fn later_folder_copies_of_served_identities_are_counted_not_listed() {
    const PADS: u32 = 481;
    let folder = Folder::new();
    // Each pad's first step makes its audio unique.
    let pad = |index: u32| {
        let mut audio = waveform(1, 300);
        audio[1] = index as i16 * 3;
        audio
    };
    let mut house_pads = Vec::new();
    let mut dance_pads = Vec::new();
    for index in 0..PADS {
        let audio = pad(index);
        house_pads.push(folder.mono(&format!("House/Drum/pad-{index:03}.wav"), &audio));
        dance_pads.push(folder.mono(&format!("Dance4/Recovered/Kick/P{index:03}.wav"), &audio));
    }
    let (house_audio, dance_audio) = (waveform(2, 300), waveform(3, 300));
    let house_only = folder.mono("House/Bass/house.wav", &house_audio);
    let dance_only = folder.mono("Dance4/Loop/dance.wav", &dance_audio);
    // A different recording under a pad's name, and the owner's own
    // identical WAVs that no catalog knows, stay ordinary sounds.
    let renamed = folder.mono("Dance4/Recovered/Kick/pad-000.wav", &waveform(4, 300));
    let own_first = folder.mono("House/Own/mine.wav", &waveform(5, 300));
    let own_second = folder.mono("Dance4/Own/mine.wav", &waveform(5, 300));
    let pads = |prefix: &str| -> Vec<crate::catalog::WaveMetadata> {
        (0..PADS)
            .map(|index| identity(index + 1, &format!("{prefix} {index}"), "Kick", &pad(index)))
            .collect()
    };
    let mut h1 = pads("pad");
    h1.push(identity(500, "house", "Bass", &house_audio));
    let mut d4 = pads("kick");
    d4.push(identity(600, "dance", "Loop", &dance_audio));
    let catalogs = |product| match product {
        Product::H1 => h1.as_slice(),
        Product::D4 => d4.as_slice(),
        _ => &[],
    };
    let house = fs::canonicalize(folder.0.join("House")).unwrap();
    let dance = fs::canonicalize(folder.0.join("Dance4")).unwrap();
    for (roots, copies, served) in [
        ([house.clone(), dance.clone()], &dance_pads, &house_pads),
        ([dance.clone(), house.clone()], &house_pads, &dance_pads),
    ] {
        let library = Library::open_wav_folders_identified(
            &roots,
            140.0,
            None,
            None,
            catalogs,
            |_, _| &[],
            &crate::source_tempo::SourceTempos::default(),
        )
        .unwrap();
        let path = |product: Product, native_id: u32| {
            library
                .sample_by_id(product.global_sample_id(native_id).unwrap())
                .and_then(|sample| sample.wav_path.clone())
        };
        // Every identity resolves, the first folder serving shared audio.
        for index in 0..PADS {
            for product in [Product::H1, Product::D4] {
                assert_eq!(
                    path(product, index + 1).as_ref(),
                    Some(&served[index as usize])
                );
            }
        }
        assert_eq!(path(Product::H1, 500), Some(house_only.clone()));
        assert_eq!(path(Product::D4, 600), Some(dance_only.clone()));
        for product in [Product::H1, Product::D4] {
            assert_eq!(library.source_root(product), Some(roots[0].as_path()));
        }
        // The later folder's copies are counted, not listed.
        let mut expected = copies.clone();
        expected.sort();
        assert_eq!(library.duplicate_copies(), expected.as_slice());
        // A song that names a copy still finds it.
        assert_eq!(
            library
                .resolve_wav_reference(&copies[0].to_string_lossy())
                .unwrap(),
            copies[0]
        );
        let mut ordinary: Vec<_> = library
            .samples
            .iter()
            .filter(|sample| sample.id >= IMPORTED_ID_START)
            .filter_map(|sample| sample.wav_path.clone())
            .collect();
        ordinary.sort();
        let mut kept = vec![renamed.clone(), own_first.clone(), own_second.clone()];
        kept.sort();
        assert_eq!(ordinary, kept);
    }
}

#[test]
fn a_supplementing_folder_keeps_the_provider_and_its_identities() {
    let folder = Folder::new();
    let (first_audio, second_audio) = (waveform(1, 300), waveform(2, 300));
    let first = folder.mono("First/first.wav", &first_audio);
    // A different recording of the first identity, named by its code.
    folder.mono("Second/CODE731.wav", &waveform(4, 300));
    let second = folder.mono("Second/second.wav", &second_audio);
    let metadata = [
        identity(731, "first", "Wave", &first_audio),
        identity(732, "second", "Wave", &second_audio),
    ];
    let roots = [
        fs::canonicalize(folder.0.join("First")).unwrap(),
        fs::canonicalize(folder.0.join("Second")).unwrap(),
    ];
    let mut library = Library::open_wav_folders_with(&roots, 140.0, None, None).unwrap();
    library.remove_product(Product::D1);
    let error = library
        .supplement_wav_product(Product::D1, &roots[1], &metadata)
        .unwrap_err();
    assert!(error.contains("no sample provider"), "{error}");
    assert_eq!(
        library
            .register_wav_product(Product::D1, &roots[0], &metadata[..1])
            .unwrap(),
        1
    );
    let before = library.bindings().to_vec();
    assert_eq!(
        library
            .supplement_wav_product(Product::D1, &roots[1], &metadata)
            .unwrap(),
        1
    );
    assert_eq!(library.bindings(), before);
    let id = |native_id| Product::D1.global_sample_id(native_id).unwrap();
    assert_eq!(
        library.sample_by_id(id(731)).unwrap().wav_path.as_ref(),
        Some(&first)
    );
    assert_eq!(
        library.sample_by_id(id(732)).unwrap().wav_path.as_ref(),
        Some(&second)
    );
    // An explicit second registration of the product is still refused.
    let error = library
        .register_wav_product(Product::D1, &roots[1], &metadata)
        .unwrap_err();
    assert!(error.contains("already has a sample provider"), "{error}");
}

#[test]
fn colliding_display_labels_do_not_guess_native_ids_and_codes_override_category() {
    let folder = Folder::new();
    folder.wave("Bass/hihat-hard.wav", 44_100, 100);
    folder.wave("unrelated/T1DC501.wav", 44_100, 100);
    let mut metadata = crate::catalog::WaveMetadata {
        native_id: 13776,
        code: "T1DC501".into(),
        label_key: crate::catalog::label_key("hihat hard"),
        group_name: "Bass".into(),
        bars: 1,
        source_bpm: 140.0,
        frame_count: 100,
        pcm_digest: None,
    };
    let mut second = metadata.clone();
    second.native_id += 1;
    second.code = "T1DC502".into();
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    assert_eq!(
        library
            .register_wav_product(Product::T2, &folder.0, &[metadata.clone(), second.clone()])
            .unwrap(),
        2
    );
    assert!(
        library
            .sample_by_id(13776)
            .unwrap()
            .wav_path
            .as_ref()
            .unwrap()
            .ends_with("T1DC501.wav")
    );
    // With no code-named source, neither colliding display label is identified.
    let ambiguous_folder = Folder::new();
    ambiguous_folder.wave("Bass/hihat-hard.wav", 44_100, 100);
    let mut ambiguous = Library::empty();
    ambiguous
        .import_wav_folder(&ambiguous_folder.0, 140.0)
        .unwrap();
    assert_eq!(
        ambiguous
            .register_wav_product(
                Product::T2,
                &ambiguous_folder.0,
                &[metadata.clone(), second]
            )
            .unwrap(),
        0
    );
    assert_eq!(ambiguous.imported_samples().len(), 1);
    metadata.native_id = 1;
    assert_eq!(
        ambiguous
            .register_wav_product(Product::D1, &ambiguous_folder.0, &[metadata])
            .unwrap(),
        1
    );
}

#[test]
fn wave_product_uses_catalog_bars_and_tempo_without_original_files() {
    let folder = Folder::new();
    folder.wave("Bass/soft-dust-06.wav", 22_050, 100);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 160.0).unwrap();
    let metadata = crate::catalog::WaveMetadata {
        native_id: 731,
        code: "native-code".into(),
        label_key: crate::catalog::label_key("soft dust 6"),
        group_name: "bass".into(),
        bars: 4,
        source_bpm: 180.0,
        frame_count: 200,
        pcm_digest: None,
    };
    assert_eq!(
        library
            .register_wav_product(Product::T1, &folder.0, std::slice::from_ref(&metadata))
            .unwrap(),
        1
    );
    let id = Product::T1.global_sample_id(731).unwrap();
    let sample = library.sample_by_id(id).unwrap();
    assert_eq!(sample.bars, 4);
    assert_eq!(sample.source_bpm, 180.0);
    assert!(library.is_wav_product(Product::T1));
    assert_eq!(library.read_pcm(id).unwrap().frames(), 100);
    library.apply_imports(&[]).unwrap();
    assert_eq!(library.read_pcm(id).unwrap().frames(), 100);

    let duplicate = folder.wave("extra/Bass/soft-dust-6.wav", 22_050, 100);
    library
        .import_wav_with_tempo(&duplicate, None, "bass", 140.0)
        .unwrap();
    let count = library.samples.len();
    assert_eq!(
        library
            .register_wav_product(Product::D1, &folder.0, &[metadata])
            .unwrap(),
        0
    );
    assert_eq!(library.samples.len(), count);
    assert!(library.source_root(Product::D1).is_none());
}
