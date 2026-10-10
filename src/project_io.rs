//! Transactional project opening and local installation registrations.
use crate::{
    engine::Project,
    library::Library,
    products::{LibraryBinding, Product},
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn data_root() -> PathBuf {
    std::env::var_os("MIXJAM_TECHNO_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("MixJam Techno"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("mixjam-techno"))
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveFolderConfig {
    pub root: PathBuf,
    pub source_bpm: f64,
}

pub fn load_wave_config() -> Result<Option<WaveFolderConfig>, String> {
    let path = data_root().join("sample-folder.json");
    if !path.exists() {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(16_385)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16_384 {
        return Err("The sample folder settings exceed the size limit.".into());
    }
    let config: WaveFolderConfig = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read sample folder settings: {e}"))?;
    if !config.root.is_absolute() || !config.source_bpm.is_finite() || config.source_bpm <= 0.0 {
        return Err(
            "The sample folder settings need an absolute path and a positive source tempo.".into(),
        );
    }
    Ok(Some(config))
}

pub fn save_wave_config(root: &Path, source_bpm: f64) -> Result<(), String> {
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    fs::create_dir_all(data_root()).map_err(|e| e.to_string())?;
    let path = data_root().join("sample-folder.json");
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(
            &serde_json::to_vec_pretty(&WaveFolderConfig { root, source_bpm })
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn reference_samples_root() -> PathBuf {
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples");
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("samples"))),
        Some(local.clone()),
        std::env::var_os("MIXJAM_TECHNO_REFERENCE_SAMPLES").map(PathBuf::from),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.join("metadata.json").is_file())
        .unwrap_or(local)
}

pub fn load_library_config() -> Result<Vec<LibraryBinding>, String> {
    let path = data_root().join("libraries.json");
    match fs::read(&path) {
        Ok(bytes) if bytes.len() <= 1024 * 1024 => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot read the library settings: {e}")),
        Ok(_) => Err("The library settings exceed the 1 MiB limit.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("Cannot read the library settings: {error}")),
    }
}

pub fn save_library_config(bindings: &[LibraryBinding]) -> Result<(), String> {
    let mut products = std::collections::HashSet::new();
    if bindings.len() > Product::ALL.len()
        || bindings
            .iter()
            .any(|b| !products.insert(b.product) || !b.root.is_absolute())
    {
        return Err("Library settings require distinct products and absolute folder paths.".into());
    }
    fs::create_dir_all(data_root()).map_err(|e| e.to_string())?;
    let path = data_root().join("libraries.json");
    let temp = data_root().join(format!(".libraries.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(bindings).map_err(|e| e.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    let result = (|| {
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn default_root(product: Product) -> Option<PathBuf> {
    // Development fixtures are explicit optional providers, never startup requirements.
    let installations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("local-data/installations");
    let media = std::env::var_os("MIXJAM_TECHNO_MEDIA_ROOT").map(PathBuf::from);
    let (local, setup) = match product {
        Product::D1 => ("\x64ance1", "\x65jay_\x64ance1_setup"),
        Product::D2 => ("\x64ance2", "\x65jay_\x64ance2_setup"),
        Product::D3 => ("\x64ance3", "\x65jay_\x64ance3_setup"),
        Product::D4 => ("\x64ance4", "\x65jay_\x64ance4_setup"),
        Product::H1 => ("\x68ouse", "\x65jay_\x68ouse_setup"),
        Product::T1 => ("\x72ave", "\x65jay_\x74echno1_setup"),
        Product::T2 => ("\x74echno2", "\x65jay_\x54echno2_setup"),
        Product::T3 => ("\x74echno3", "\x65jay_\x74echno3_setup"),
    };
    [
        Some(installations.join(local)),
        media.map(|media| media.join(setup)),
    ]
    .into_iter()
    .flatten()
    .find(|root| root.is_dir())
}

pub fn open(path: &Path, library: &Library) -> Result<(Project, Library), String> {
    let candidate = library.clone();
    let project = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mix"))
    {
        let product = detect_product(path)?;
        let mut project = match product {
            Product::T2 => crate::legacy_mix::load(path, &candidate)?,
            Product::T1 => crate::compat::t1::load(path, &candidate)?,
            Product::D1 => crate::compat::d1::load(path, &candidate)?,
            _ => crate::compat::chunks::load(path, product, &candidate)?,
        };
        project.libraries = candidate.bindings().to_vec();
        project
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mixjam"))
    {
        crate::mixjam::load(path, &candidate)?
    } else {
        Project::load(path)?
    };
    resolve(project, &candidate)
}

pub fn resolve(mut project: Project, library: &Library) -> Result<(Project, Library), String> {
    let mut candidate = library.clone();
    project
        .imported_samples
        .retain(|sample| project.clips.iter().any(|clip| clip.sample_id == sample.id));
    for binding in project.libraries.clone() {
        // A selected WAV provider takes precedence over paths saved on another machine.
        if candidate.source_root(binding.product).is_some() || candidate.wav_root().is_some() {
            continue;
        }
        if !project.clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id)
                .is_some_and(|(product, _)| product == binding.product)
        }) {
            continue;
        }
        if !binding.root.is_dir() {
            return Err(format!(
                "The {} library folder is unavailable. Select its current folder in Libraries.",
                binding.product.label()
            ));
        }
        let root = fs::canonicalize(&binding.root).map_err(|e| e.to_string())?;
        candidate.register(binding.product, &root, &reference_samples_root())?;
    }
    rebase_legacy_assets(&mut project, &candidate)?;
    // Keep the selected folder's palette. Saved imports retain their IDs; unused
    // palette entries receive fresh IDs if the saved project uses those numbers.
    candidate.apply_imports(&project.imported_samples)?;
    project
        .imported_samples
        .extend(candidate.restore_wav_palette(library)?);
    for clip in &project.clips {
        if candidate.sample_by_id(clip.sample_id).is_none() {
            return Err(format!(
                "Sample {} is missing. Add its product library before you open this song.",
                clip.sample_id
            ));
        }
    }
    project.libraries = candidate.bindings().to_vec();
    project.validate()?;
    Ok((project, candidate))
}

pub fn detect_product(path: &Path) -> Result<Product, String> {
    let mut header = [0; 4];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|e| format!("Cannot read the song header: {e}"))?;
    let signature = u32::from_le_bytes(header);
    match signature {
        0x889 => Ok(Product::T2),
        0xa19 => Ok(Product::D2),
        0x2571 => Ok(Product::D3),
        0x2572 => Ok(Product::T3),
        0x2d41 => Ok(Product::D4),
        0x11d6 => Ok(Product::H1),
        _ => match u16::from_le_bytes(header[..2].try_into().unwrap()) {
            0xa06 => Ok(Product::D1),
            0xa07 => Ok(Product::T1),
            _ => Err("This song format is not supported.".into()),
        },
    }
}

/// Resolve imported files before the old source project folders are removed.
/// A selected WAV folder relinks imports by path suffix. An import that the
/// folder lacks keeps its own file when that file exists; a missing file is
/// restored from the preservation manifest, then looked up in the library.
fn rebase_legacy_assets(project: &mut Project, library: &Library) -> Result<(), String> {
    let data_root = data_root();
    for imported in &mut project.imported_samples {
        let reference = imported.path.to_string_lossy().into_owned();
        if library.wav_root().is_some()
            && !imported.path.starts_with(&data_root)
            && let Some(path) = library.find_wav_reference(&reference)?
        {
            imported.path = path;
            continue;
        }
        let path = crate::preservation::resolve_asset(&imported.path)?;
        if path != imported.path {
            let notice = format!(
                "Restored {} from the preserved project files.",
                imported.name
            );
            if !project.source_notices.contains(&notice) {
                project.source_notices.push(notice);
            }
            imported.path = path;
            continue;
        }
        if !imported.path.is_file() && !library.samples.is_empty() {
            imported.path = library.resolve_wav_reference(&reference)?;
        }
    }
    Ok(())
}

/// A saved song references its used audio, not every sound in the browser palette.
pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    let mut saved = project.clone();
    saved
        .imported_samples
        .retain(|sample| saved.clips.iter().any(|clip| clip.sample_id == sample.id));
    saved.libraries.retain(|binding| {
        saved.clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id)
                .is_some_and(|(product, _)| product == binding.product)
        })
    });
    saved.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wave(path: &Path) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..128 {
            writer.write_sample(500i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn wav_only_project_relinks_and_renders_without_saved_installations() {
        let root = std::env::temp_dir().join(format!(
            "mixjam-techno-standalone-open-{}",
            std::process::id()
        ));
        let waves = root.join("moved/Drums");
        fs::create_dir_all(&waves).unwrap();
        fs::create_dir_all(root.join("old/Drums")).unwrap();
        write_wave(&waves.join("my-own-kick.wav"));
        write_wave(&root.join("old/Drums/my-own-kick.wav"));
        let library = Library::open_wav_folder(&root.join("moved"), 140.0).unwrap();
        let mut imported = library.imported_samples().remove(0);
        imported.path = root.join("old/Drums/my-own-kick.wav");
        let mut project = Project {
            imported_samples: vec![imported.clone()],
            libraries: vec![LibraryBinding {
                product: Product::T2,
                root: root.join("absent-installation"),
            }],
            ..Project::default()
        };
        let mut unused = imported.clone();
        unused.id += 1;
        unused.path = root.join("absent-unused.wav");
        project.imported_samples.push(unused);
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 1,
            offset_frames: 0,
            gain: 1.0,
        });
        let file = root.join("song.json");
        project.save(&file).unwrap();
        let (opened, ready) = open(&file, &library).unwrap();
        assert_eq!(
            opened.imported_samples[0].path,
            fs::canonicalize(waves.join("my-own-kick.wav")).unwrap()
        );
        assert!(opened.libraries.is_empty());
        let audio = crate::audio::render_project(&opened, &ready).unwrap();
        assert!(audio.samples.iter().any(|sample| *sample != 0.0));
        save(&opened, &file).unwrap();
        assert_eq!(Project::load(&file).unwrap().imported_samples.len(), 1);
        assert_eq!(open(&file, &library).unwrap().0, opened);
        fs::remove_file(file).unwrap();
        fs::remove_file(waves.join("my-own-kick.wav")).unwrap();
        fs::remove_file(root.join("old/Drums/my-own-kick.wav")).unwrap();
        fs::remove_dir(root.join("old/Drums")).unwrap();
        fs::remove_dir(root.join("old")).unwrap();
        fs::remove_dir(waves).unwrap();
        fs::remove_dir(root.join("moved")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn ambiguous_selected_folder_rejects_open_even_when_saved_sample_exists() {
        let root = std::env::temp_dir().join(format!(
            "mixjam-techno-ambiguous-open-{}",
            std::process::id()
        ));
        let selected = root.join("selected");
        let old = root.join("old");
        for folder in [selected.join("a"), selected.join("b"), old.clone()] {
            fs::create_dir_all(&folder).unwrap();
            write_wave(&folder.join("take.wav"));
        }
        let library = Library::open_wav_folder(&selected, 140.0).unwrap();
        let original_palette = library.imported_samples();
        let imported = crate::engine::ImportedSample {
            id: crate::products::IMPORTED_ID_START + 7,
            name: "Take".into(),
            group_name: "Wave".into(),
            path: fs::canonicalize(old.join("take.wav")).unwrap(),
            source_bpm: 140.0,
            origin: None,
        };
        let mut project = Project {
            imported_samples: vec![imported.clone()],
            ..Project::default()
        };
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 1,
            offset_frames: 0,
            gain: 1.0,
        });
        let file = root.join("song.json");
        project.save(&file).unwrap();
        let saved_bytes = fs::read(&file).unwrap();

        let error = open(&file, &library).unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");
        assert_eq!(fs::read(&file).unwrap(), saved_bytes);
        assert_eq!(Project::load(&file).unwrap(), project);
        assert_eq!(library.imported_samples(), original_palette);
        assert!(library.sample_by_id(imported.id).is_none());
        assert!(imported.path.is_file());

        fs::remove_file(file).unwrap();
        for folder in [selected.join("a"), selected.join("b"), old] {
            fs::remove_file(folder.join("take.wav")).unwrap();
            fs::remove_dir(folder).unwrap();
        }
        fs::remove_dir(selected).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn selected_folder_keeps_existing_imports_that_it_lacks() {
        let root =
            std::env::temp_dir().join(format!("mixjam-techno-keep-import-{}", std::process::id()));
        let waves = root.join("folder");
        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&waves).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        write_wave(&waves.join("kick.wav"));
        write_wave(&elsewhere.join("take.wav"));
        let library = Library::open_wav_folder(&waves, 140.0).unwrap();
        let take = fs::canonicalize(elsewhere.join("take.wav")).unwrap();
        let imported = crate::engine::ImportedSample {
            id: crate::products::IMPORTED_ID_START + 7,
            name: "Take".into(),
            group_name: "Wave".into(),
            path: take.clone(),
            source_bpm: 140.0,
            origin: None,
        };
        let mut project = Project {
            imported_samples: vec![imported.clone()],
            ..Project::default()
        };
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 1,
            offset_frames: 0,
            gain: 1.0,
        });
        let (opened, ready) = resolve(project.clone(), &library).unwrap();
        assert_eq!(opened.imported_samples[0].path, take);
        assert_eq!(
            ready.sample_by_id(imported.id).unwrap().wav_path.as_deref(),
            Some(take.as_path())
        );
        fs::remove_file(&take).unwrap();
        let error = resolve(project, &library).unwrap_err();
        assert!(
            error.contains("missing from the selected folder"),
            "{error}"
        );
        fs::remove_file(waves.join("kick.wav")).unwrap();
        fs::remove_dir(waves).unwrap();
        fs::remove_dir(elsewhere).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn typed_product_headers_reject_truncation_and_unknown_formats() {
        let root =
            std::env::temp_dir().join(format!("mixjam-techno-header-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("test.mix");
        for (signature, product) in [
            (0x889u32, Product::T2),
            (0xa19, Product::D2),
            (0x2571, Product::D3),
            (0x2572, Product::T3),
            (0x2d41, Product::D4),
            (0x11d6, Product::H1),
            (0xa06, Product::D1),
            (0xa07, Product::T1),
        ] {
            fs::write(&path, signature.to_le_bytes()).unwrap();
            assert_eq!(detect_product(&path).unwrap(), product);
        }
        fs::write(&path, [7, 10]).unwrap();
        assert!(detect_product(&path).is_err());
        fs::write(&path, [0xff; 4]).unwrap();
        assert!(detect_product(&path).is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn failed_open_preserves_the_existing_catalog_and_project_files() {
        let root = std::env::temp_dir().join(format!("mixjam-techno-open-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("song.json");
        let mut project = Project::default();
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: 999,
            track: 63,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
        });
        project.save(&path).unwrap();
        let before = fs::read(&path).unwrap();
        let library = Library::empty();
        assert!(open(&path, &library).is_err());
        assert!(library.samples.is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
