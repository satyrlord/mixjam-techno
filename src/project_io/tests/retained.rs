//! Sounds a song keeps beside the data folder: provider supplements and manual choices.
use super::*;
use crate::project_io::{
    open::catalog_notices,
    retained::{provider_wave_audio, retain_manual_wave_within, retain_native_provider},
    save::save_with_library_in,
};
use crate::{
    engine::{MissingSample, Project, TICKS_PER_BAR},
    library::Library,
    products::{LibraryBinding, Product},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn original_house_provider(root: &Path) -> Library {
    original_house_provider_with_id(root, 1)
}

fn original_house_provider_with_id(root: &Path, native_id: u32) -> Library {
    let packed = crate::library::packed_folder(root);
    fs::create_dir_all(&packed).unwrap();
    let mut record = b"tPxD\x02\0\0T".to_vec();
    record.extend(5_u32.to_le_bytes());
    record.extend([0, 0, 0]);
    fs::write(packed.join("house10"), &record).unwrap();
    fs::write(
        packed.join("house10.inf"),
        format!(
            "[SAMPLES]\n{native_id}\n6\n\"HOUSE\"\n0\n{}\n\"House sound\"\n\
                 \"1\"\n1\n18\n\"\"\n0\n\"\"\n",
            record.len()
        ),
    )
    .unwrap();
    let mut installed = Library::empty();
    installed.register(Product::H1, root, root).unwrap();
    installed
}

fn supplemental_house_sample(native_id: u32, path: PathBuf) -> crate::library::Sample {
    crate::library::Sample {
        id: Product::H1.global_sample_id(native_id).unwrap(),
        code: "SUPPLEMENT".into(),
        name: "Supplement".into(),
        bars: 1,
        group_name: "Bass".into(),
        offset: 0,
        length: 0,
        frame_count: 5,
        sample_rate: 44_100,
        source_bpm: 125.0,
        channels: 1,
        wav_path: Some(path),
        digest: None,
    }
}

#[test]
fn imports_name_the_products_whose_catalogs_lack_their_native_ids() {
    let mut project = Project::default();
    for product in [Product::D1, Product::H1] {
        project.missing_samples.push(
            MissingSample::native(
                product,
                99_999,
                "Unknown".into(),
                140.0,
                TICKS_PER_BAR,
                true,
                "Missing.".into(),
            )
            .unwrap(),
        );
    }
    let notices = catalog_notices(&project, &Library::empty());
    assert_eq!(notices.len(), 2, "{notices:?}");
    assert!(notices[0].contains("D1") && notices[0].contains("native ID"));
    assert!(notices[1].contains("H1") && notices[1].contains("native ID"));
    assert!(
        Library::empty()
            .describe_missing(Product::D1, 99_999)
            .starts_with("No loaded D1 sample catalog lists native sample 99999")
    );
    // An original installation serves H1 without a catalog, so a sample
    // that it lacks is missing from the installation, not from a catalog.
    let root = std::env::temp_dir().join(format!("mixjam-catalog-notice-{}", std::process::id()));
    let mut installed = original_house_provider(&root);
    // A replacement provider may be supplemented by WAV sources from a
    // previously loaded folder. Its original source still establishes
    // which native IDs are in the installation.
    let supplemental = Library::fixture(vec![supplemental_house_sample(
        2,
        root.join("supplement.wav"),
    )]);
    let wav_notices = catalog_notices(&project, &supplemental);
    assert_eq!(wav_notices.len(), 2, "{wav_notices:?}");
    assert!(wav_notices[1].contains("H1"));
    installed.supplement_loaded(Product::H1, &supplemental);
    assert!(installed.is_wav_product(Product::H1));
    let notices = catalog_notices(&project, &installed);
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("D1"));
    assert_eq!(
        installed.describe_missing(Product::H1, 99_999),
        "H1 native sample 99999 is missing from its library."
    );
    fs::remove_dir_all(root).unwrap();
}

fn project_using(library: &Library, ids: &[u32]) -> Project {
    let mut editor = crate::engine::Editor::new(Project::default());
    for (track, id) in ids.iter().enumerate() {
        editor.place(*id, track, 0, 256).unwrap();
    }
    let mut project = editor.project().clone();
    project.libraries = library.bindings().to_vec();
    project
}

#[test]
fn mixed_packed_and_supplemental_sources_do_not_replace_the_song() {
    let root = std::env::temp_dir().join(format!("mixjam-mixed-save-{}", std::process::id()));
    let provider_root = root.join("provider");
    let original = original_house_provider(&provider_root);
    let native_id = Product::H1.global_sample_id(1).unwrap();
    let song = root.join("song.mixjam");
    fs::write(&song, b"previous song").unwrap();
    for path in [
        provider_root.join("supplement.wav"),
        root.join("supplement.wav"),
    ] {
        let mut library = original.clone();
        let supplemental = supplemental_house_sample(2, path);
        let supplemental_id = supplemental.id;
        library.supplement_loaded(Product::H1, &Library::fixture(vec![supplemental]));
        let project = project_using(&library, &[native_id, supplemental_id]);
        let error =
            save_with_library_in(&project, &library, &song, &root.join("storage")).unwrap_err();
        assert!(error.contains("both an original installation and supplemental WAVs"));
        assert!(error.contains("only one provider folder"));
        assert_eq!(fs::read(&song).unwrap(), b"previous song");
        assert!(!root.join("storage").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn wav_only_supplement_is_retained_even_inside_the_packed_provider() {
    let root = std::env::temp_dir().join(format!("mixjam-wav-only-save-{}", std::process::id()));
    let provider_root = root.join("provider");
    let original = original_house_provider(&provider_root);
    let entry = crate::catalog::identities(Product::H1)
        .iter()
        .find(|entry| entry.native_id != 1 && entry.frame_count < 1_000_000);
    for (index, inside) in [true, false].into_iter().enumerate() {
        let folder = if inside { &provider_root } else { &root };
        let name = entry.map_or_else(
            || "unknown.wav".into(),
            |entry| {
                Path::new(&entry.code.replace('\\', "/"))
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
                    + ".wav"
            },
        );
        let path = folder.join(name);
        let frames = entry.map_or(5, |entry| entry.frame_count.div_ceil(2) as usize);
        let pcm = crate::audio::Pcm {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![0.25; frames],
        };
        pcm.write_wav(&path).unwrap();
        let mut sample =
            supplemental_house_sample(entry.map_or(99_999, |entry| entry.native_id), path);
        sample.sample_rate = pcm.sample_rate;
        sample.frame_count = frames as u32;
        if let Some(entry) = entry {
            sample.code.clone_from(&entry.code);
            sample.name = entry.display_name(Product::H1);
            sample.group_name.clone_from(&entry.group_name);
            sample.source_bpm = entry.source_bpm;
            sample.bars = entry.bars;
        }
        let id = sample.id;
        let mut library = original.clone();
        library.supplement_loaded(Product::H1, &Library::fixture(vec![sample]));
        let project = project_using(&library, &[id]);
        let song = root.join(format!("song-{index}.mixjam"));
        fs::write(&song, b"previous song").unwrap();
        let result = save_with_library_in(&project, &library, &song, &root.join("storage"));
        if entry.is_some() {
            result.unwrap();
            let (reopened, ready) = open(&song, &Library::empty()).unwrap();
            assert!(reopened.missing_samples.is_empty());
            assert_eq!(ready.read_pcm(id).unwrap().samples, pcm.samples);
            let (reopened_with_original, ready_with_original) = open(&song, &original).unwrap();
            assert!(reopened_with_original.missing_samples.is_empty());
            assert_eq!(
                ready_with_original.read_pcm(id).unwrap().samples,
                pcm.samples
            );
            assert_eq!(reopened_with_original.libraries, reopened.libraries);
            assert_eq!(
                ready_with_original.source_root(Product::H1),
                original.source_root(Product::H1)
            );
        } else {
            assert!(result.unwrap_err().contains("no catalog identity"));
            assert_eq!(fs::read(&song).unwrap(), b"previous song");
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unavailable_saved_supplement_keeps_the_binding_for_a_later_open() {
    let root =
        std::env::temp_dir().join(format!("mixjam-missing-supplement-{}", std::process::id()));
    let original = original_house_provider(&root.join("provider"));
    let present = Product::H1.global_sample_id(1).unwrap();
    let missing = Product::H1.global_sample_id(99_999).unwrap();
    let mut project = project_using(&original, &[present, missing]);
    let saved = LibraryBinding {
        product: Product::H1,
        root: root.join("unavailable"),
    };
    project.libraries = vec![saved.clone()];
    let (reopened, ready) = resolve(project, &original).unwrap();
    assert_eq!(reopened.libraries, vec![saved]);
    assert!(reopened.missing_sample(missing).is_some());
    assert_eq!(ready.read_pcm(present).unwrap().samples, vec![0.0; 5]);
    assert_eq!(
        ready.source_root(Product::H1),
        original.source_root(Product::H1)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_supplement_fills_only_used_missing_ids_without_replacing_originals() {
    let root = std::env::temp_dir().join(format!(
        "mixjam-supplement-collision-{}",
        std::process::id()
    ));
    let entries: Vec<_> = crate::catalog::identities(Product::H1)
        .iter()
        .filter(|entry| entry.frame_count < 1_000_000)
        .take(3)
        .collect();
    if entries.len() < 3 {
        // No locally generated catalog is available. Exercise the same
        // unavailable binding path without claiming WAV identification.
        let original = original_house_provider(&root.join("provider"));
        let present = Product::H1.global_sample_id(1).unwrap();
        let missing = Product::H1.global_sample_id(99_999).unwrap();
        let mut project = project_using(&original, &[present, missing]);
        project.libraries[0].root = root.join("unavailable");
        let (reopened, ready) = resolve(project, &original).unwrap();
        assert!(reopened.missing_sample(missing).is_some());
        assert_eq!(ready.read_pcm(present).unwrap().samples, vec![0.0; 5]);
        fs::remove_dir_all(root).unwrap();
        return;
    }
    let original = original_house_provider_with_id(&root.join("provider"), entries[0].native_id);
    let saved_root = root.join("saved-wavs");
    fs::create_dir_all(&saved_root).unwrap();
    for (index, entry) in entries.iter().enumerate() {
        let name = Path::new(&entry.code.replace('\\', "/"))
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        crate::audio::Pcm {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![0.25 * (index + 1) as f32; entry.frame_count.div_ceil(2) as usize],
        }
        .write_wav(&saved_root.join(format!("{name}.wav")))
        .unwrap();
    }
    let ids: Vec<_> = entries
        .iter()
        .map(|entry| Product::H1.global_sample_id(entry.native_id).unwrap())
        .collect();
    let mut project = project_using(&original, &ids[..2]);
    project.libraries[0].root = saved_root.clone();
    let (reopened, ready) = resolve(project, &original).unwrap();
    assert!(reopened.missing_samples.is_empty());
    assert_eq!(ready.read_pcm(ids[0]).unwrap().samples, vec![0.0; 5]);
    assert!(
        ready
            .read_pcm(ids[1])
            .unwrap()
            .samples
            .iter()
            .all(|&sample| sample == 0.5)
    );
    assert!(ready.sample_by_id(ids[2]).is_none());
    assert_eq!(reopened.libraries[0].root, saved_root);
    assert_eq!(
        ready.source_root(Product::H1),
        original.source_root(Product::H1)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_audio_identity_ignores_riff_metadata() {
    let root = std::env::temp_dir().join(format!("mixjam-provider-wave-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let first = root.join("first.wav");
    let second = root.join("second.wav");
    crate::audio::Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 300],
    }
    .write_wav(&first)
    .unwrap();
    let bytes = fs::read(&first).unwrap();
    let mut tagged = bytes.clone();
    tagged.extend_from_slice(b"JUNK\x04\x00\x00\x00test");
    let riff_size = (tagged.len() as u32 - 8).to_le_bytes();
    tagged[4..8].copy_from_slice(&riff_size);
    fs::write(&second, &tagged).unwrap();
    assert_ne!(bytes, tagged);
    assert_eq!(
        provider_wave_audio(&first).unwrap(),
        provider_wave_audio(&second).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn retained_native_provider_requires_known_ids_and_preserves_audio_on_failure() {
    let root =
        std::env::temp_dir().join(format!("mixjam-retained-provider-{}", std::process::id()));
    let metadata = crate::catalog::identities(Product::D1);
    if metadata.len() < 2 {
        // A public checkout need not contain the optional catalog files.
        // In that case, exercise rejection of an unknown identity instead
        // of indexing an empty local catalog or inventing factory sounds.
        let native_id = (1..100_000)
            .find(|id| !metadata.iter().any(|entry| entry.native_id == *id))
            .unwrap();
        let id = Product::D1.global_sample_id(native_id).unwrap();
        let path = root.join("unmatched.wav");
        let error = retain_native_provider(Product::D1, vec![(id, &path)], &root).unwrap_err();
        assert_eq!(error, "The native sample has no catalog identity.");
        assert_eq!(
            fs::read_dir(root.join("native-providers").join(Product::D1.slug()))
                .unwrap()
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
        return;
    }
    let mut sources = Vec::new();
    let mut editor = crate::engine::Editor::new(Project::default());
    for (index, entry) in metadata.iter().take(2).enumerate() {
        let folder = root.join(format!("volume-{index}"));
        fs::create_dir_all(&folder).unwrap();
        let code = entry.code.replace('\\', "/");
        let path = folder
            .join(Path::new(&code).file_stem().unwrap())
            .with_extension("wav");
        crate::audio::Pcm {
            sample_rate: 22_050,
            channels: 2,
            samples: vec![0.25 * (index + 1) as f32; entry.frame_count as usize],
        }
        .write_wav(&path)
        .unwrap();
        let id = Product::D1.global_sample_id(entry.native_id).unwrap();
        editor.place(id, index, 0, 256).unwrap();
        sources.push((id, path));
    }
    let folders = [root.join("volume-0"), root.join("volume-1")];
    let library = Library::open_wav_folders_with(&folders, 140.0, None, None).unwrap();
    let mut project = editor.project().clone();
    // Inject a binding which cannot cover the loaded files, the same
    // condition that different Windows volume roots produce.
    project.libraries = vec![LibraryBinding {
        product: Product::D1,
        root: root.join("old-provider"),
    }];
    project.clips[0].offset_frames = 3;
    project.clips[0].gain = 0.7;
    let original = project.clone();
    let song = root.join("saved.mixjam");
    let storage = root.join("data");
    save_with_library_in(&project, &library, &song, &storage).unwrap();
    let (opened, ready) = open(&song, &Library::empty()).unwrap();
    assert!(opened.missing_samples.is_empty());
    assert_eq!(opened.clips, original.clips);
    assert_eq!(project, original);
    for (id, path) in &sources {
        let expected = crate::audio::read_wav(path).unwrap();
        let actual = ready.read_pcm(*id).unwrap();
        assert_eq!(actual.samples, expected.samples);
        assert_eq!(actual.sample_rate, expected.sample_rate);
        assert_eq!(actual.channels, expected.channels);
    }
    let retained = opened.libraries[0].root.clone();
    assert!(retained.starts_with(fs::canonicalize(&storage).unwrap()));
    save_with_library_in(&project, &library, &song, &storage).unwrap();
    let (again, _) = open(&song, &Library::empty()).unwrap();
    assert_eq!(again.libraries[0].root, retained);
    assert_eq!(fs::read_dir(retained.parent().unwrap()).unwrap().count(), 1);
    let previous_song = fs::read(&song).unwrap();
    let cached = ready
        .sample_by_id(sources[0].0)
        .unwrap()
        .wav_path
        .as_ref()
        .unwrap();
    let mut corrupt = crate::audio::read_wav(cached).unwrap();
    corrupt.samples.fill(0.0);
    corrupt.write_wav(cached).unwrap();
    assert!(save_with_library_in(&project, &library, &song, &storage).is_err());
    assert_eq!(fs::read(&song).unwrap(), previous_song);
    assert_eq!(fs::read_dir(retained.parent().unwrap()).unwrap().count(), 1);
    assert_eq!(project, original);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manual_wave_retention_rejects_oversized_files_before_creating_staging_files() {
    let root = std::env::temp_dir().join(format!("mixjam-manual-size-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let wave = root.join("oversized.wav");
    fs::write(&wave, [0u8; 1001]).unwrap();
    let cache = root.join("cache");
    assert!(
        retain_manual_wave_within(&wave, &cache, 1000)
            .unwrap_err()
            .contains("larger than 4 GiB")
    );
    assert!(!cache.exists());
    fs::remove_file(wave).unwrap();
    fs::remove_dir(root).unwrap();
}
