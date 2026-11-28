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
fn an_oversized_or_overlong_sample_index_is_refused_before_it_is_parsed() {
    let folder = Folder::new();
    let packed = super::packed_folder(&folder.0);
    fs::create_dir_all(&packed).unwrap();
    fs::write(packed.join("archive"), [0u8; 16]).unwrap();
    let open = || {
        Library::open_archive(
            crate::products::Product::T2,
            &folder.0,
            &folder.0,
            "archive",
            140.0,
        )
        .err()
        .unwrap()
    };
    let index = packed.join("archive.inf");
    let records = crate::catalog::MAX_INDEX_RECORDS + 1;
    fs::write(
        &index,
        format!(
            "[SAMPLES]
{}",
            "0
"
            .repeat(records * 12)
        ),
    )
    .unwrap();
    assert_eq!(open(), "The sample index lists too many records.");
    let file = fs::File::create(&index).unwrap();
    file.set_len(crate::catalog::MAX_CATALOG_TEXT_BYTES + 1)
        .unwrap();
    drop(file);
    assert_eq!(open(), "The sample index exceeds its size limit.");
}
