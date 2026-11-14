//! Settings files in the data folder, and the scan cache that opening uses.
use super::*;
use crate::{engine::Project, library::Library};
use std::fs;

#[test]
fn a_headless_open_reuses_and_extends_the_scan_cache() {
    let root = std::env::temp_dir().join(format!("mixjam-cached-open-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (samples, data) = (root.join("Samples"), root.join("data"));
    fs::create_dir_all(samples.join("Loop")).unwrap();
    for path in [
        samples.join("one.wav"),
        samples.join("Loop").join("two.wav"),
    ] {
        crate::audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 441],
        }
        .write_wav(&path)
        .unwrap();
    }
    let first =
        open_wav_folders_cached(&data, std::slice::from_ref(&samples), 140.0, None).unwrap();
    assert_eq!(first.samples.len(), 2);
    assert_eq!(load_scan_cache(&data).len(), 2);
    // Bytes that cannot be decoded, at the size and time the cache knows:
    // only the cached probe can still read the file.
    let path = samples.join("one.wav");
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let length = fs::metadata(&path).unwrap().len() as usize;
    fs::write(&path, vec![0u8; length]).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let cached =
        open_wav_folders_cached(&data, std::slice::from_ref(&samples), 140.0, None).unwrap();
    assert_eq!(cached.samples.len(), 2);
    assert!(cached.skipped_waves().is_empty());
    let fresh = Library::open_wav_folder(&samples, 140.0).unwrap();
    assert_eq!(fresh.samples.len(), 1);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn reopening_refreshes_changed_wave_metadata_at_the_same_path() {
    let root = std::env::temp_dir().join(format!("mixjam-refresh-wave-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("take.wav");
    let mut pcm = crate::audio::Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 441],
    };
    pcm.write_wav(&path).unwrap();
    let mut library = Library::empty();
    let imported = library
        .import_wav_with_tempo(&path, None, "Wave", 140.0)
        .unwrap();
    let project = Project {
        imported_samples: vec![imported.clone()].into(),
        clips: vec![crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        }],
        ..Project::default()
    };
    pcm.channels = 2;
    pcm.samples = vec![0.5; 1_764];
    pcm.write_wav(&path).unwrap();
    let (_, refreshed) = resolve(project, &library).unwrap();
    let sample = refreshed.sample_by_id(imported.id).unwrap();
    assert_eq!(sample.frame_count, 882);
    assert_eq!(sample.channels, 2);
    assert_eq!(sample.digest, None);
    assert_eq!(sample.length, fs::metadata(&path).unwrap().len());
    fs::remove_file(path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn song_folder_is_remembered_until_the_folder_disappears() {
    let root = std::env::temp_dir().join(format!("mixjam-song-folder-{}", std::process::id()));
    let settings = root.join("settings");
    let songs = root.join("songs");
    fs::create_dir_all(&songs).unwrap();
    assert_eq!(load_song_folder(&settings), None);
    save_song_folder(&settings, &songs).unwrap();
    assert_eq!(load_song_folder(&settings), Some(songs.clone()));
    fs::remove_dir(&songs).unwrap();
    assert_eq!(load_song_folder(&settings), None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_earlier_single_folder_setting_reads_as_a_list_and_saves_as_one() {
    let root = std::env::temp_dir().join(format!("mixjam-folder-config-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let settings = root.join("settings");
    let (first, second) = (root.join("First"), root.join("Second"));
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::create_dir_all(&settings).unwrap();
    let file = settings.join("sample-folder.json");
    assert_eq!(load_wave_config_in(&settings), Ok(None));
    // The file that earlier versions wrote: one root.
    fs::write(
        &file,
        serde_json::to_vec(&serde_json::json!({"root": first, "source_bpm": 160.0})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        load_wave_config_in(&settings),
        Ok(Some(WaveFolderConfig {
            roots: vec![first.clone()],
            source_bpm: 160.0,
        }))
    );
    // A save writes the list. A folder that is offline keeps its path.
    let offline = root.join("Offline");
    save_wave_config_in(
        &settings,
        &[first.clone(), offline.clone(), second.clone()],
        150.0,
    )
    .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert!(
        saved.get("root").is_none() && saved["roots"].is_array(),
        "{saved}"
    );
    assert_eq!(
        load_wave_config_in(&settings).unwrap().unwrap(),
        WaveFolderConfig {
            roots: vec![
                fs::canonicalize(&first).unwrap(),
                offline,
                fs::canonicalize(&second).unwrap()
            ],
            source_bpm: 150.0,
        }
    );
    // Ten folders are refused, in a save and in a file.
    let ten: Vec<_> = (0..10)
        .map(|index| root.join(format!("L{index}")))
        .collect();
    assert!(save_wave_config_in(&settings, &ten, 140.0).is_err());
    fs::write(
        &file,
        serde_json::to_vec(&serde_json::json!({"roots": ten, "source_bpm": 140.0})).unwrap(),
    )
    .unwrap();
    assert!(
        load_wave_config_in(&settings)
            .unwrap_err()
            .contains("up to 9")
    );
    fs::write(
        &file,
        serde_json::to_vec(
            &serde_json::json!({"root": first, "roots": [second], "source_bpm": 140.0}),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(load_wave_config_in(&settings).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn oversized_folder_settings_do_not_replace_a_readable_list() {
    let root =
        std::env::temp_dir().join(format!("mixjam-folder-config-size-{}", std::process::id()));
    let settings = root.join("settings");
    let roots = |depth| {
        (0..crate::library::MAX_LIBRARY_FOLDERS)
            .map(|index| {
                let mut path = root.join(format!("Offline{index}"));
                for _ in 0..depth {
                    path.push("a".repeat(80));
                }
                path
            })
            .collect::<Vec<_>>()
    };
    let accepted = roots(10);
    save_wave_config_in(&settings, &accepted, 140.0).unwrap();
    let expected = WaveFolderConfig {
        roots: accepted,
        source_bpm: 140.0,
    };
    assert_eq!(load_wave_config_in(&settings), Ok(Some(expected.clone())));
    let file = settings.join("sample-folder.json");
    let before = fs::read(&file).unwrap();

    let error = save_wave_config_in(&settings, &roots(30), 160.0).unwrap_err();
    assert!(error.contains("size limit"), "{error}");
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(load_wave_config_in(&settings), Ok(Some(expected)));
    fs::remove_dir_all(root).unwrap();
}
