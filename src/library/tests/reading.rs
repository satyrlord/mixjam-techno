use super::*;

#[test]
fn original_pcm_keeps_source_identity_when_playback_hides_a_stray_end() {
    let folder = Folder::new();
    let mut audio = vec![0; 600];
    audio.extend([8192, 8192]);
    let path = folder.mono("source.wav", &audio);
    let stored = fs::read(&path).unwrap();
    let mut library = Library::empty();
    let imports = library.import_wav_folder(&folder.0, 140.0).unwrap();
    let id = imports[0].id;
    let original = library.read_original_pcm(id).unwrap();
    let playback = library.read_pcm(id).unwrap();
    let integer = |pcm: &crate::audio::Pcm| -> Vec<i16> {
        pcm.samples
            .iter()
            .map(|sample| (sample * 32768.0).round() as i16)
            .collect()
    };
    assert_eq!(integer(&original), audio);
    assert_eq!(integer(&playback), vec![0; audio.len()]);
    assert_eq!(
        crate::catalog::pcm_identity(&integer(&original)),
        crate::catalog::pcm_identity(&audio)
    );
    assert_ne!(
        crate::catalog::pcm_identity(&integer(&playback)),
        crate::catalog::pcm_identity(&audio)
    );
    assert_eq!(fs::read(path).unwrap(), stored);
}

#[test]
fn native_packed_reads_unicode_host_paths_without_native_ansi_limits() {
    let folder = Folder::new();
    let path = folder.0.join("音源.\x70xd");
    let mut bytes = b"t\x50xD\0T".to_vec();
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(37u16.to_le_bytes());
    bytes.extend([255, 128, 255, 128]);
    fs::write(&path, bytes).unwrap();
    let mut library = Library::empty();
    library.samples.push(Sample {
        id: 100_731,
        code: "source.\x70xd".into(),
        name: "Native source".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 16,
        frame_count: 2,
        sample_rate: 44100,
        source_bpm: 180.0,
        channels: 1,
        wav_path: None,
        digest: None,
    });
    library.by_id.insert(100_731, 0);
    library
        .sources
        .insert(100_731, AudioSource::T1Packed { path: path.clone() });
    let pcm = library.read_pcm(100_731).unwrap();
    assert_eq!(pcm.samples, vec![74.0 / 32768.0; 3]);
    // A Step Synth source file decodes as the library does: from the header
    // word, without silence padded to the stated length.
    assert_eq!(read_packed_file(&path).unwrap().samples, pcm.samples);
}

#[test]
#[ignore = "Requires all eight local native sample libraries; decodes the complete corpus."]
fn every_registered_native_sample_decodes_with_its_native_length_rules() {
    let mut total = 0;
    let mut failures = Vec::new();
    for (product, expected) in [
        (Product::D1, 2845),
        (Product::D2, 3795),
        (Product::D3, 5049),
        (Product::D4, 5330),
        (Product::H1, 3563),
        (Product::T1, 3017),
        (Product::T2, 4015),
        (Product::T3, 4006),
    ] {
        let root = crate::project_io::default_root(product).expect("Native library fixture");
        let mut library = Library::empty();
        library
            .register(product, &root, &crate::project_io::reference_samples_root())
            .unwrap();
        assert_eq!(library.samples.len(), expected, "{}", product.label());
        let started = std::time::Instant::now();
        let mut frames = 0u64;
        let mut decoded = 0;
        for sample in &library.samples {
            let context = format!("{} / {} / {}", product.label(), sample.id, sample.name);
            let pcm = match library.read_original_pcm(sample.id) {
                Ok(pcm) => pcm,
                Err(error) => {
                    failures.push(format!("{context}: {error}"));
                    continue;
                }
            };
            if let Err(error) = pcm.validate() {
                failures.push(format!("{context}: {error}"));
            }
            let declared = u64::from(sample.frame_count);
            let length_matches = if let AudioSource::T1Packed { path } =
                &library.sources[&sample.id]
            {
                // Complete final runs and the metadata loader's pending
                // initial sample differ from isolated declared-length PCM.
                let mut prefix = [0; 512];
                let read = fs::File::open(path).unwrap().read(&mut prefix).unwrap();
                let header = legacy_format::packed::parse_header(&prefix[..read]).unwrap();
                let initial = u64::from(header.kind == legacy_format::packed::HeaderKind::Metadata);
                if product == Product::D1 && sample.id == 205_043 {
                    // Authenticated factory file d4sp434.packed has 109,215
                    // physical codes and a nominal 151,200-frame header.
                    use sha2::Digest;
                    let hash = format!("{:x}", sha2::Sha256::digest(fs::read(path).unwrap()));
                    if declared != 151_200
                        || hash
                            != "07c40aaf4668c213c3b9f340c1db8d266b7fa55887897d0ba0be37186e5c649f"
                    {
                        failures.push(format!(
                            "{context}: the authenticated short-file fixture changed"
                        ));
                    }
                    pcm.frames() == 109_216
                } else {
                    (declared.saturating_sub(4) + initial..=declared + 4 + initial)
                        .contains(&pcm.frames())
                }
            } else {
                pcm.frames() == declared
            };
            if !length_matches {
                failures.push(format!(
                    "{context}: {} frames for declared {declared}",
                    pcm.frames()
                ));
            }
            if pcm.sample_rate != sample.sample_rate || pcm.channels != sample.channels {
                failures.push(format!(
                    "{context}: decoded format differs from the catalog"
                ));
            }
            frames += pcm.frames();
            decoded += 1;
        }
        eprintln!(
            "Native decode: {}: {decoded}/{expected} samples, {frames} frames, {:.3} seconds",
            product.slug(),
            started.elapsed().as_secs_f64()
        );
        total += library.samples.len();
    }
    assert_eq!(total, 31_620);
    assert!(
        failures.is_empty(),
        "{} native decode failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "Requires the local WAV sample corpus; validates every WAV without original providers."]
fn standalone_local_wave_corpus_has_native_aliases_without_original_providers() {
    let root = std::env::var_os("MIXJAM_TECHNO_WAV_CORPUS")
        .map(PathBuf::from)
        .expect("Set MIXJAM_TECHNO_WAV_CORPUS");
    let started = std::time::Instant::now();
    let library = Library::open_wav_folder(&root, 140.0).unwrap();
    assert!(library.archive_path.as_os_str().is_empty());
    let native = library
        .samples
        .iter()
        .filter(|s| Product::sample_key(s.id).is_some_and(|(p, _)| p == Product::T2))
        .count();
    eprintln!(
        "Standalone WAV folder: {} palette entries, {native} T2 identities, {} generic WAVs",
        library.samples.len(),
        library.imported_samples().len()
    );
    eprintln!(
        "WAV scan elapsed: {:.3} seconds",
        started.elapsed().as_secs_f64()
    );
    assert!(native > 0);
    assert!(
        library
            .samples
            .iter()
            .all(|sample| sample.wav_path.is_some())
    );
}

#[test]
#[ignore = "Requires the installed original and the supplied WAV corpus."]
fn installed_catalog_has_complete_nonoverlapping_ranges() {
    let root = crate::project_io::default_root(Product::T2).expect("T2 fixtures");
    let mut library = Library::empty();
    library
        .register(
            Product::T2,
            &root,
            &crate::project_io::reference_samples_root(),
        )
        .unwrap();
    assert_eq!(library.samples.len(), 4015);
    assert_eq!(
        library.samples.last().unwrap().offset + library.samples.last().unwrap().length,
        429261617
    );
    assert!(
        library
            .samples
            .iter()
            .filter(|s| s.wav_path.is_some())
            .count()
            > 3800
    );
    assert_eq!(library.sample_by_id(14546).unwrap().frame_count, 75600);
    assert_eq!(library.read_pcm(14546).unwrap().frames(), 75600);
    assert_eq!(library.step_synth_sources().unwrap().len(), 504);
}
