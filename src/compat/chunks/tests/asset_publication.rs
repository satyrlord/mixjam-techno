use super::*;

#[test]
fn preserved_audio_never_overwrites_an_existing_file() {
    let directory = asset_test_directory("chunk-audio");
    let path = directory.join("sample.wav");
    let bytes = wave_fixture();
    preserve_wave(&path, &bytes).unwrap();
    preserve_wave(&path, &bytes).unwrap();
    assert!(preserve_wave(&path, b"different").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn unsupported_wave_rate_is_rejected_before_any_asset_is_published() {
    let mut wave = wave_fixture_frames(44100);
    wave[24..28].copy_from_slice(&705600u32.to_le_bytes());
    wave[28..32].copy_from_slice(&1411200u32.to_le_bytes());
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_le_bytes();
    wave.extend_from_slice(b"JUNK");
    wave.extend_from_slice(&16u32.to_le_bytes());
    wave.extend_from_slice(&nonce);
    let riff_length = (wave.len() - 8) as u32;
    wave[4..8].copy_from_slice(&riff_length.to_le_bytes());
    let reader = hound::WavReader::new(Cursor::new(&wave)).unwrap();
    assert_eq!(reader.spec().sample_rate, 705600);
    assert_eq!(reader.duration(), 44100);
    let hash = format!("{:x}", Sha256::digest(&wave));
    let destination = crate::project_io::data_root()
        .join("imported-audio")
        .join(format!("{hash}.wav"));
    assert!(!destination.exists());
    let source_path = r"C:\old\MIXWAVES\high-rate.wav";
    let mut bytes = header_fixture(Product::D3, 140, 140);
    let header = read_header(&bytes, Product::D3).unwrap();
    bytes.extend_from_slice(&((source_path.len() + 2) as u16).to_le_bytes());
    bytes.extend_from_slice(source_path.as_bytes());
    bytes.extend_from_slice(&[0, 1]);
    bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&wave);
    let timeline = Timeline {
        start: bytes.len(),
        events: vec![Event {
            track: 0,
            position: 0,
            left_trim: 0,
            right_trim: 0,
            gain: 1.0,
            reference: Reference::Wave(WaveReference {
                path: source_path.into(),
                declared_ticks: None,
                metadata: BTreeMap::new(),
            }),
        }],
    };
    assert!(
        build_project(
            Path::new("high-rate.mix"),
            &bytes,
            Product::D3,
            &Library::empty(),
            header,
            timeline
        )
        .is_err()
    );
    assert!(!destination.exists());
    if let Some(parent) = destination.parent().filter(|parent| parent.is_dir()) {
        assert!(!fs::read_dir(parent).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(&format!(".{hash}.wav."))
        }));
    }
}

#[test]
fn concurrent_equal_asset_publication_is_atomic_and_idempotent() {
    use std::sync::{Arc, Barrier};
    let directory = asset_test_directory("concurrent-publication");
    let path = Arc::new(directory.join("equal.wav"));
    let bytes = Arc::new(wave_fixture_frames(44100));
    let barrier = Arc::new(Barrier::new(8));
    let writers: Vec<_> = (0..8)
        .map(|_| {
            let path = Arc::clone(&path);
            let bytes = Arc::clone(&bytes);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                preserve_wave(&path, &bytes)
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap().unwrap();
    }
    assert_eq!(fs::read(path.as_ref()).unwrap(), *bytes);
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    preserve_wave(&path, &bytes).unwrap();
    assert!(preserve_wave(&path, b"different bytes").is_err());
    assert_eq!(fs::read(path.as_ref()).unwrap(), *bytes);
    fs::remove_file(path.as_ref()).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn interrupted_or_failed_staging_leaves_final_assets_safe_and_retryable() {
    let directory = asset_test_directory("interrupted-publication");
    let path = directory.join("retry.wav");
    let bytes = wave_fixture();
    let interrupted = stage_asset(&path, &bytes).unwrap();
    assert!(!path.exists());
    fs::write(&interrupted, b"partial prepublication write").unwrap();
    preserve_wave(&path, &bytes).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        fs::read(&interrupted).unwrap(),
        b"partial prepublication write"
    );
    let collision = directory.join("collision.json");
    let staged = stage_asset(&collision, b"complete metadata").unwrap();
    fs::write(&collision, b"unrelated existing metadata").unwrap();
    assert!(publish_staged_asset(&staged, &collision, b"complete metadata").is_err());
    assert!(!staged.exists());
    assert_eq!(
        fs::read(&collision).unwrap(),
        b"unrelated existing metadata"
    );
    let absent = directory.join("missing-parent/absent.wav");
    let staged = stage_asset(&path, &bytes).unwrap();
    assert!(publish_staged_asset(&staged, &absent, &bytes).is_err());
    assert!(!absent.exists());
    assert!(!staged.exists());
    fs::remove_file(&path).unwrap();
    fs::remove_file(&interrupted).unwrap();
    fs::remove_file(&collision).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn direct_publication_never_replaces_an_existing_asset() {
    let directory = asset_test_directory("direct-publication");
    let path = directory.join("direct.wav");
    let bytes = wave_fixture();
    publish_asset_directly(&path, &bytes).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    publish_asset_directly(&path, &bytes).unwrap();
    assert!(publish_asset_directly(&path, b"different bytes").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let absent = directory.join("missing-parent/absent.wav");
    assert!(publish_asset_directly(&absent, &bytes).is_err());
    assert!(!absent.exists());
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&directory).unwrap();
}
