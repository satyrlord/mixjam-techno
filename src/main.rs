#[cfg(feature = "gui")]
mod ui;
#[cfg(feature = "gui")]
mod ui_icons;

use mixjam_techno::{
    audio,
    engine::Project,
    legacy_mix,
    library::Library,
    products::{LibraryBinding, Product},
    project_io,
};
#[cfg(feature = "gui")]
use std::sync::Arc;
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut app: Option<PathBuf> = None;
    let mut samples: Option<PathBuf> = None;
    let mut source_bpm = 140.0f64;
    let mut mix = None;
    let mut command = "gui".to_string();
    let mut output = None;
    let mut additional_libraries = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--app" => {
                app = Some(PathBuf::from(
                    args.next().ok_or("--app needs a provider path.")?,
                ))
            }
            "--library" => {
                let value = args.next().ok_or("--library needs PRODUCT=PATH.")?;
                let (product, root) = value
                    .split_once('=')
                    .ok_or("--library needs PRODUCT=PATH.")?;
                additional_libraries.push(LibraryBinding {
                    product: Product::parse(product)?,
                    root: PathBuf::from(root),
                });
            }
            "--samples" => {
                samples = Some(PathBuf::from(
                    args.next().ok_or("--samples needs a WAV folder path.")?,
                ))
            }
            "--source-bpm" => {
                source_bpm = args
                    .next()
                    .ok_or("--source-bpm needs a tempo.")?
                    .parse()
                    .map_err(|_| "The source tempo must be a number.")?;
            }
            "--mix" => {
                mix = Some(PathBuf::from(
                    args.next().ok_or("--mix needs a project path.")?,
                ))
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output needs a WAV path.")?,
                ))
            }
            "inspect" | "verify-fixtures" | "render" | "convert" | "gui" => command = arg,
            "--help" | "-h" => {
                println!(
                    "MixJam Techno\n\nUsage: mixjam-techno [gui|inspect|verify-fixtures|render|convert] [--app PATH] [--library PRODUCT=PATH] [--samples PATH] [--mix PATH] [--output PATH]\n\nProducts: d1, d2, d3, d4, h1, t1, t2, t3.\nThe editor supports up to 64 tracks and 120, 140, 160, or 180 BPM.\nSelect a WAV folder with --samples PATH or Libraries in the app. No original installation is required. --source-bpm sets the tempo for unrecognized WAVs (default 140).\nrender and convert require --mix and --output. convert saves native JSON.\nverify-fixtures reads T2 demos."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}. Use --help for usage.")),
        }
    }
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    let mut library = Library::empty();
    let mut startup_messages = Vec::new();
    if samples.is_none() && command == "gui" {
        match project_io::load_wave_config() {
            Ok(Some(config)) => {
                samples = Some(config.root);
                source_bpm = config.source_bpm;
            }
            Ok(None) => {}
            Err(error) => startup_messages.push(error),
        }
    }
    if let Some(root) = &samples {
        match Library::open_wav_folder(root, source_bpm) {
            Ok(loaded) => library = loaded,
            Err(error) if command == "gui" => startup_messages.push(error),
            Err(error) => return Err(error),
        }
    }
    if command == "gui" {
        // Libraries added in the Libraries panel persist in libraries.json. The
        // selected WAV folder already identifies its own products and wins.
        match project_io::load_library_config() {
            Ok(bindings) => {
                for binding in bindings {
                    if library.source_root(binding.product).is_some() {
                        continue;
                    }
                    if let Err(error) = library.register(
                        binding.product,
                        &binding.root,
                        &project_io::reference_samples_root(),
                    ) {
                        startup_messages.push(error);
                    }
                }
            }
            Err(error) => startup_messages.push(error),
        }
    }
    if let Some(root) = &app {
        // An explicit provider replaces a T2 binding derived from the WAV folder.
        if library.source_root(Product::T2).is_some() {
            library.remove_product(Product::T2)?;
        }
        library.register(Product::T2, root, &project_io::reference_samples_root())?;
    }
    for binding in additional_libraries {
        if library.source_root(binding.product).is_some() {
            library.remove_product(binding.product)?;
        }
        library.register(
            binding.product,
            &binding.root,
            &project_io::reference_samples_root(),
        )?;
    }
    match command.as_str() {
        "inspect" => {
            let mapped = library
                .samples
                .iter()
                .filter(|s| s.wav_path.is_some())
                .count();
            println!(
                "{}",
                serde_json::json!({"samples":library.samples.len(),"name_and_frame_matched_wavs":mapped,"unresolved_wav_mappings":library.samples.len()-mapped,"archive":library.archive_path,"samples_root":samples,
                    "tracks":mixjam_techno::engine::TRACK_COUNT,"supported_bpms":mixjam_techno::engine::SUPPORTED_BPMS,
                    "libraries":library.bindings().iter().map(|binding| serde_json::json!({"product":binding.product.slug(),
                        "samples":library.samples.iter().filter(|sample| Product::sample_key(sample.id).is_some_and(|(product,_)|product==binding.product)).count()})).collect::<Vec<_>>()})
            );
            Ok(())
        }
        "verify-fixtures" => {
            let root = app
                .as_ref()
                .ok_or("verify-fixtures requires --app PATH to the development fixtures.")?;
            let folder = root.join("\x65Jay").join("mix");
            let mut paths = std::fs::read_dir(&folder)
                .map_err(|e| e.to_string())?
                .map(|r| r.map(|x| x.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            paths.sort();
            let mut total = 0;
            for path in paths
                .into_iter()
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mix")))
            {
                let p = legacy_mix::load(&path, &library)?;
                let missing = p
                    .clips
                    .iter()
                    .filter(|c| {
                        library
                            .sample_by_id(c.sample_id)
                            .is_none_or(|s| s.wav_path.is_none())
                    })
                    .count();
                println!(
                    "{}: {} clips, {} ticks, {} clip references without name/frame WAV associations (original archive remains available)",
                    path.file_name().unwrap().to_string_lossy(),
                    p.clips.len(),
                    p.end_tick(),
                    missing
                );
                total += 1;
            }
            if total == 0 {
                return Err("No demo mixes were found.".into());
            }
            println!(
                "Parsed and validated {total} original demo mixes. Audio equivalence is not established."
            );
            Ok(())
        }
        "convert" => {
            let path = mix.ok_or("convert requires --mix PATH.")?;
            let target = output.ok_or("convert requires --output JSON.")?;
            let (project, _) = project_io::open(&path, &library)?;
            project_io::save(&project, &target)?;
            println!(
                "{}",
                serde_json::json!({"tracks":project.tracks.len(),"bpm":project.bpm,"clips":project.clips.len(),"import_notices":project.import_notices,"source_notices":project.source_notices,"output":target})
            );
            Ok(())
        }
        "render" => {
            let path = mix.ok_or("render requires --mix PATH.")?;
            let target = output.ok_or("render requires --output WAV.")?;
            let (p, resolved_library) = project_io::open(&path, &library)?;
            library = resolved_library;
            for notice in &p.import_notices {
                println!(
                    "Imported tempo: {} -> {} BPM.",
                    notice.source_bpm, notice.target_bpm
                );
            }
            let pcm = audio::render_project(&p, &library)?;
            pcm.write_wav(&target)?;
            println!(
                "Wrote {} stereo frames at {} Hz to {}",
                pcm.frames(),
                pcm.sample_rate,
                target.display()
            );
            Ok(())
        }
        _ => {
            let project = if let Some(path) = mix {
                let (project, resolved_library) = project_io::open(&path, &library)?;
                library = resolved_library;
                project
            } else {
                Project {
                    libraries: library.bindings().to_vec(),
                    imported_samples: library.imported_samples(),
                    ..Project::default()
                }
            };
            #[cfg(feature = "gui")]
            {
                let size = std::env::var("\x54ECHNOREA_WINDOW_SIZE")
                    .ok()
                    .and_then(|value| {
                        let (w, h) = value.split_once('x')?;
                        let w = w.parse::<f32>().ok()?;
                        let h = h.parse::<f32>().ok()?;
                        if (960.0..=3840.0).contains(&w) && (720.0..=2160.0).contains(&h) {
                            Some([w, h])
                        } else {
                            None
                        }
                    })
                    .unwrap_or([1440.0, 1000.0]);
                let options = eframe::NativeOptions {
                    viewport: eframe::egui::ViewportBuilder::default()
                        .with_inner_size(size)
                        .with_min_inner_size([960.0, 720.0]),
                    ..Default::default()
                };
                eframe::run_native(
                    "MixJam Techno",
                    options,
                    Box::new(move |cc| {
                        let mut app = ui::MixJamApp::new(
                            cc,
                            mixjam_techno::application::Session::new(Arc::new(library), project)?,
                        );
                        app.set_startup_messages(&startup_messages);
                        Ok(Box::new(app))
                    }),
                )
                .map_err(|e| e.to_string())
            }
            #[cfg(not(feature = "gui"))]
            {
                let _ = (project, library, startup_messages);
                Err("This build has no GUI. Rebuild with the default features.".into())
            }
        }
    }
}
