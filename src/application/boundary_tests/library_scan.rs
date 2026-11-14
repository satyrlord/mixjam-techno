//! Library folder scans, providers, and deferred song opens.
use super::*;
use std::sync::mpsc;

#[test]
fn folder_adoption_keeps_optional_providers_and_respects_explicit_priority() {
    // Whether the folder is complete depends on the size of the H1 catalog.
    let complete_count = crate::catalog::identities(Product::H1).len();
    let dir = fixture_dir();
    let provider_root = dir.join("provider");
    let packed = crate::library::packed_folder(&provider_root);
    std::fs::create_dir_all(&packed).unwrap();
    // A complete one-record native House provider, with five silent frames.
    let mut record = b"tPxD\x02\0\0T".to_vec();
    record.extend(5_u32.to_le_bytes());
    record.extend([0, 0, 0]);
    std::fs::write(packed.join("house10"), &record).unwrap();
    std::fs::write(
        packed.join("house10.inf"),
        format!(
            "[SAMPLES]\n1\n6\n\"HOUSE\"\n0\n{}\n\"House sound\"\n\"1\"\n1\n18\n\"\"\n0\n\"\"\n",
            record.len()
        ),
    )
    .unwrap();
    let mut provider = Library::empty();
    provider
        .register(Product::H1, &provider_root, &dir)
        .unwrap();
    let house_id = Product::H1.global_sample_id(1).unwrap();
    assert_eq!(provider.read_pcm(house_id).unwrap().samples, vec![0.0; 5]);
    let folder = dir.join("Dance4");
    std::fs::create_dir_all(&folder).unwrap();
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.0; 5],
    }
    .write_wav(&folder.join("sound.wav"))
    .unwrap();
    let identity = |native_id| crate::catalog::WaveMetadata {
        native_id,
        code: format!("WAVE{native_id}"),
        label_key: crate::catalog::label_key("Folder sound"),
        group_name: "Wave".into(),
        bars: 1,
        source_bpm: 140.0,
        frame_count: 5,
        pcm_digest: Some(crate::catalog::pcm_identity_hex(&[0; 5])),
    };
    let mut cases = vec![
        (0, false, true), // A D4-only folder must keep H1.
        (complete_count.max(1), false, false),
        (complete_count.max(1), true, true),
    ];
    if complete_count > 2 {
        // One shared sound is sparse only when the catalog has at least
        // three identities. A public checkout can have no local catalog.
        cases.push((1, false, true));
    }
    for (house_count, preferred, expect_provider) in cases {
        let mut scanned = Library::open_wav_folder(&folder, 140.0).unwrap();
        scanned
            .register_wav_product(Product::D4, &folder, &[identity(1)])
            .unwrap();
        if house_count > 0 {
            let entries: Vec<_> = (1..=house_count as u32).map(identity).collect();
            scanned
                .register_wav_product(Product::H1, &folder, &entries)
                .unwrap();
        }
        let mut session = Session::headless(Arc::new(provider.clone()), Project::default())
            .unwrap()
            .with_settings_root(dir.join("settings"));
        if preferred {
            session.prefer_providers(&[Product::H1]);
        }
        assert!(
            session
                .adopt_wave_folder(scanned, std::slice::from_ref(&folder), 140.0)
                .unwrap()
                .is_empty()
        );
        let source = session.library().sample_by_id(house_id).unwrap();
        assert_eq!(
            source.wav_path.is_none(),
            expect_provider,
            "count={house_count}, preferred={preferred}"
        );
        assert!(
            session
                .library()
                .sample_by_id(Product::D4.global_sample_id(1).unwrap())
                .is_some()
        );
        assert!(!session.unsaved());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unavailable_optional_provider_keeps_scanned_product_and_reports_warning() {
    let dir = fixture_dir();
    let old = dir.join("provider");
    let new = dir.join("folder");
    let metadata = [crate::catalog::WaveMetadata {
        native_id: 1,
        code: "TEST".into(),
        label_key: crate::catalog::label_key("Test"),
        group_name: "Wave".into(),
        bars: 1,
        source_bpm: 140.0,
        frame_count: 1000,
        pcm_digest: Some(crate::catalog::pcm_identity_hex(&vec![0; 1000])),
    }];
    for folder in [&old, &new] {
        std::fs::create_dir_all(folder).unwrap();
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 1000],
        }
        .write_wav(&folder.join("sound.wav"))
        .unwrap();
    }
    let mut provider = Library::empty();
    provider.import_wav_folder(&old, 140.0).unwrap();
    provider
        .register_wav_product(Product::T2, &old, &metadata)
        .unwrap();
    let mut scanned = Library::open_wav_folder(&new, 140.0).unwrap();
    scanned
        .register_wav_product(Product::T2, &new, &metadata)
        .unwrap();
    let native_id = Product::T2.global_sample_id(1).unwrap();
    assert!(scanned.sample_by_id(native_id).is_some());
    let mut session = Session::headless(Arc::new(provider), Project::default())
        .unwrap()
        .with_settings_root(dir.join("settings"));
    std::fs::remove_dir_all(&old).unwrap();
    let song = dir.join("queued.json");
    Project::default().save(&song).unwrap();
    let (sender, receiver) = mpsc::channel();
    session.wave_scan = Some(WaveScan {
        roots: vec![new],
        source_bpm: 140.0,
        progress: Arc::new(crate::library::ScanProgress::default()),
        receiver,
    });
    session.open_after_scan(&song).unwrap();
    sender.send(Ok(scanned)).unwrap();
    session.poll();
    assert!(session.library().sample_by_id(native_id).is_some());
    assert_eq!(session.take_opened_after_scan(), Some(song));
    let (notice, error) = session.take_notice().unwrap();
    assert!(error && notice.contains("Cannot open"), "{notice}");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn deferred_scan_open_accepts_previously_confirmed_unsaved_changes() {
    let dir = fixture_dir();
    let song = dir.join("accepted.json");
    Project {
        title: "Replacement".into(),
        ..Project::default()
    }
    .save(&song)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(dir.join("settings"));
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    assert!(session.unsaved());
    let (sender, receiver) = mpsc::channel();
    session.wave_scan = Some(WaveScan {
        roots: vec![dir.clone()],
        source_bpm: 140.0,
        progress: Arc::new(crate::library::ScanProgress::default()),
        receiver,
    });
    // The UI has already confirmed replacement, just as for immediate open.
    session.open_after_scan(&song).unwrap();
    sender.send(Ok(Library::empty())).unwrap();
    session.poll();
    assert_eq!(session.project().title, "Replacement");
    assert_eq!(session.take_opened_after_scan(), Some(song));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancelled_catalog_rescans_remain_pending_and_use_the_active_scan_folders() {
    let dir = fixture_dir();
    let root = dir.join("samples");
    std::fs::create_dir_all(&root).unwrap();
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&root.join("sound.wav"))
    .unwrap();
    let settings = dir.join("settings");
    crate::project_io::save_wave_config_in(&settings, std::slice::from_ref(&root), 120.0).unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(settings);
    assert!(session.rescan_changed_catalogs(true));
    session.cancel_wave_scan();
    assert!(session.catalog_rescan_needed);
    assert!(session.rescan_changed_catalogs(false));
    assert_eq!(session.wave_scan.as_ref().unwrap().source_bpm, 120.0);
    // A second Open waits for the current scan instead of opening stale data.
    assert!(session.rescan_changed_catalogs(false));
    // A change during a scan restarts that scan with its current folders/tempo.
    session.wave_scan.as_mut().unwrap().source_bpm = 160.0;
    assert!(session.rescan_changed_catalogs(true));
    assert_eq!(session.wave_scan.as_ref().unwrap().source_bpm, 160.0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while session.wave_scan().is_some() {
        session.poll();
        assert!(std::time::Instant::now() < deadline, "scan timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!session.catalog_rescan_needed);
    assert!(!session.rescan_changed_catalogs(false));
    // Abandoned workers can still finish their cache writes in this test folder.
}

#[test]
fn deferred_scan_open_preserves_intervening_work() {
    let dir = fixture_dir();
    let song = dir.join("queued.json");
    Project {
        title: "Queued song".into(),
        ..Project::default()
    }
    .save(&song)
    .unwrap();
    let other = dir.join("other.json");
    Project {
        title: "Other song".into(),
        ..Project::default()
    }
    .save(&other)
    .unwrap();
    for action in [
        "edit",
        "edit_then_save",
        "master_then_save",
        "master_undo_then_save",
        "new",
        "open",
        "take",
    ] {
        let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
            .unwrap()
            .with_settings_root(dir.join(action));
        let (sender, receiver) = mpsc::channel();
        session.wave_scan = Some(WaveScan {
            roots: vec![dir.clone()],
            source_bpm: 140.0,
            progress: Arc::new(crate::library::ScanProgress::default()),
            receiver,
        });
        session.open_after_scan(&song).unwrap();
        match action {
            "edit" => session.edit(EditCommand::SetTempo(160.0)).unwrap(),
            "edit_then_save" => {
                session.edit(EditCommand::SetTempo(160.0)).unwrap();
                session.save(&dir.join("saved.mixjam")).unwrap();
            }
            "master_then_save" | "master_undo_then_save" => {
                let mut master = session.project().master;
                master.trim = -3.0;
                session.edit(EditCommand::SetMaster(master)).unwrap();
                if action == "master_undo_then_save" {
                    session.history_step(false).unwrap();
                }
                session.save(&dir.join("saved.mixjam")).unwrap();
            }
            "new" => session.new_mix().unwrap(),
            "open" => session.open(&other).unwrap(),
            "take" => {
                session.recorded_take = Some(Arc::new(Pcm {
                    sample_rate: 44_100,
                    channels: 1,
                    samples: vec![0.25; 32],
                }))
            }
            _ => unreachable!(),
        }
        let expected = session.project().clone();
        let expected_file = session.file.clone();
        sender.send(Ok(Library::empty())).unwrap();
        session.poll();
        assert_eq!(session.project(), &expected, "{action}");
        assert_eq!(session.file, expected_file, "{action}");
        assert!(session.take_opened_after_scan().is_none(), "{action}");
        assert!(
            session.take_notice().unwrap().0.contains("was not opened"),
            "{action}"
        );
        assert_eq!(session.has_recorded_take(), action == "take");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn background_folder_scans_adopt_on_poll_and_cancel_without_replacing_the_library() {
    let dir = fixture_dir();
    std::fs::create_dir_all(dir.join("Loop")).unwrap();
    for (name, frames) in [
        ("Loop/a.wav", 75_600),
        ("Loop/b.wav", 151_200),
        ("Loop/d.wav", 151_200),
        ("c.wav", 9_001),
    ] {
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; frames],
        }
        .write_wav(&dir.join(name))
        .unwrap();
    }
    // One unreadable file is reported and does not block the folder.
    std::fs::write(dir.join("broken.wav"), b"not audio").unwrap();
    let song = dir.join("song.mixjam");
    let data = dir.join("data");
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(data.clone());
    assert!(session.wave_scan().is_none());
    session
        .start_wave_scan(std::slice::from_ref(&dir), 120.0)
        .unwrap();
    let status = session.wave_scan().unwrap();
    assert_eq!(status.roots, std::slice::from_ref(&dir));
    assert!(status.files_done <= 5);
    // The song opens once the scanned library can resolve it.
    session.open_after_scan(&song).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while session.wave_scan().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "the scan did not finish"
        );
        let delay = session.poll();
        if session.wave_scan().is_some() {
            assert_eq!(delay, Some(100));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(session.library().samples.len(), 4);
    assert_eq!(session.library().folder_tempos().len(), 1);
    let (notice, error) = session.take_notice().unwrap();
    // The pending song did not exist: the folder loaded, the open failed visibly.
    assert!(error && notice.contains("song.mixjam"), "{notice}");
    assert!(notice.contains("Skipped 1 unreadable WAV"), "{notice}");
    assert!(notice.contains("broken.wav"), "{notice}");
    assert_eq!(session.library().skipped_waves().len(), 1);
    assert_eq!(crate::project_io::load_scan_cache(&data).len(), 4);
    let cache_path = data.join("wav-scan-cache.json");
    let cache_modified = std::fs::metadata(&cache_path).unwrap().modified().unwrap();
    assert!(data.join("sample-folder.json").is_file());
    // A cancelled scan cannot deliver a late result to the session.
    let before = session.library().samples.len();
    let progress = Arc::new(crate::library::ScanProgress::default());
    let (sender, receiver) = mpsc::channel();
    session.wave_scan = Some(WaveScan {
        roots: vec![dir.join("Loop")],
        source_bpm: 140.0,
        progress: Arc::clone(&progress),
        receiver,
    });
    session.open_after_scan(&song).unwrap();
    session.cancel_wave_scan();
    assert!(session.wave_scan().is_none());
    assert!(progress.is_cancelled());
    assert!(sender.send(Ok(Library::empty())).is_err());
    let (notice, error) = session.take_notice().unwrap();
    assert!(!error && notice.contains("song.mixjam"), "{notice}");
    assert_eq!(session.poll(), None);
    assert_eq!(session.library().samples.len(), before);
    assert!(
        session
            .start_wave_scan(std::slice::from_ref(&dir), f64::NAN)
            .is_err()
    );
    // The cancelled scan's song is not opened by a later scan; a queued one is.
    let finish = |session: &mut Session| {
        while session.wave_scan().is_some() {
            assert!(
                std::time::Instant::now() < deadline,
                "the scan did not finish"
            );
            session.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    std::fs::remove_file(dir.join("broken.wav")).unwrap();
    session
        .start_wave_scan(std::slice::from_ref(&dir), 120.0)
        .unwrap();
    finish(&mut session);
    assert!(session.library().skipped_waves().is_empty());
    assert_eq!(
        std::fs::metadata(&cache_path).unwrap().modified().unwrap(),
        cache_modified,
        "An unchanged cached scan must not rewrite its cache."
    );
    let (notice, error) = session.take_notice().unwrap();
    assert!(
        !error && notice.starts_with("Sample libraries loaded"),
        "{notice}"
    );
    assert_eq!(session.take_opened_after_scan(), None);
    let source = session.library().samples[0].id;
    session.place(source, 0, 0).unwrap();
    session.save(&song).unwrap();
    session
        .start_wave_scan(std::slice::from_ref(&dir), 120.0)
        .unwrap();
    session.open_after_scan(&song).unwrap();
    finish(&mut session);
    assert_eq!(session.take_opened_after_scan(), Some(song.clone()));
    assert_eq!(session.project().clips.len(), 1);
    assert!(!session.unsaved());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn a_partial_library_scan_accepts_relative_folders_and_saves_absolute_paths() {
    let dir =
        PathBuf::from("tmp").join(format!("mixjam-relative-libraries-{}", std::process::id()));
    let (kept, offline) = (dir.join("Kept"), dir.join("Offline"));
    std::fs::create_dir_all(&kept).unwrap();
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 441],
    }
    .write_wav(&kept.join("sound.wav"))
    .unwrap();
    let data = dir.join("data");
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(data.clone());
    let ten: Vec<_> = (0..10).map(|index| dir.join(format!("L{index}"))).collect();
    assert!(session.start_wave_scan(&ten, 140.0).is_err());
    assert!(session.wave_scan().is_none());
    let roots = [offline.clone(), kept.clone()];
    session.start_wave_scan(&roots, 140.0).unwrap();
    assert_eq!(session.wave_scan().unwrap().roots, roots);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while session.wave_scan().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "the scan did not finish"
        );
        session.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(session.library().samples.len(), 1);
    let (notice, error) = session.take_notice().unwrap();
    assert!(error, "{notice}");
    assert!(
        notice.starts_with("Sample libraries loaded: 1 sounds."),
        "{notice}"
    );
    assert!(
        notice.contains("was not loaded") && notice.contains("Offline"),
        "{notice}"
    );
    // The offline folder stays in the list for the next scan.
    let saved = crate::project_io::load_wave_config_in(&data)
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.roots,
        [
            std::path::absolute(offline).unwrap(),
            std::fs::canonicalize(&kept).unwrap()
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn adopting_a_folder_that_moves_no_clip_keeps_undo() {
    let dir = fixture_dir();
    let folder = dir.join("waves");
    std::fs::create_dir_all(&folder).unwrap();
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 75_600],
    }
    .write_wav(&folder.join("a.wav"))
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(dir.join("settings"));
    session
        .edit(EditCommand::SetMixer {
            tracks: session.project().tracks.clone(),
            master_gain: 0.5,
        })
        .unwrap();
    let edited = session.project().master_gain;
    let scanned = Library::open_wav_folder(&folder, 140.0).unwrap();
    session
        .adopt_wave_folder(scanned, std::slice::from_ref(&folder), 140.0)
        .unwrap();
    assert_eq!(session.project().master_gain, edited);
    assert!(session.has_history_step(false));
    assert!(session.history_step(false).unwrap());
    assert_eq!(
        session.project().master_gain,
        Project::default().master_gain
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn added_libraries_persist_in_the_session_settings_folder() {
    let dir = fixture_dir();
    let provider_root = dir.join("provider");
    let packed = crate::library::packed_folder(&provider_root);
    std::fs::create_dir_all(&packed).unwrap();
    let mut record = b"tPxD\x02\0\0T".to_vec();
    record.extend(5_u32.to_le_bytes());
    record.extend([0, 0, 0]);
    std::fs::write(packed.join("house10"), &record).unwrap();
    std::fs::write(
        packed.join("house10.inf"),
        format!(
            "[SAMPLES]\n1\n6\n\"HOUSE\"\n0\n{}\n\"House sound\"\n\"1\"\n1\n18\n\"\"\n0\n\"\"\n",
            record.len()
        ),
    )
    .unwrap();
    let settings = dir.join("settings");
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(settings.clone());
    session.add_library(Product::H1, &provider_root).unwrap();
    // The session's own settings folder holds the binding, not the
    // process-wide data folder that the user's real settings live in.
    let bindings = crate::project_io::load_library_config(&settings).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].product, Product::H1);
}
