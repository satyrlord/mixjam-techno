#[cfg(feature = "gui")]
mod ui;
#[cfg(feature = "gui")]
mod ui_icons;

use mixjam_techno::{
    agent, audio,
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
            // A failed agent request has already printed its reply.
            if !e.is_empty() {
                eprintln!("{e}");
            }
            ExitCode::FAILURE
        }
    }
}

fn replace_provider(library: &mut Library, binding: &LibraryBinding) -> Result<(), String> {
    let mut candidate = library.clone();
    if candidate.source_root(binding.product).is_some() {
        candidate.remove_product(binding.product)?;
    }
    candidate.register(
        binding.product,
        &binding.root,
        &project_io::reference_samples_root(),
    )?;
    *library = candidate;
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut app: Option<PathBuf> = None;
    let mut samples: Option<PathBuf> = None;
    let mut source_bpm: Option<f64> = None;
    let mut mix = None;
    let mut command = "gui".to_string();
    let mut output = None;
    let mut additional_libraries = Vec::new();
    let mut agent_port: Option<u16> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // The rest of the line is a request for a running application.
            "agent" => {
                return match agent::run_client(&args.collect::<Vec<_>>())? {
                    true => Ok(()),
                    false => Err(String::new()),
                };
            }
            "--agent" => agent_port = agent_port.or(Some(agent::DEFAULT_PORT)),
            "--agent-port" => {
                agent_port = Some(
                    args.next()
                        .and_then(|port| port.parse().ok())
                        .ok_or("--agent-port needs a port number.")?,
                );
            }
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
                source_bpm = Some(
                    args.next()
                        .ok_or("--source-bpm needs a tempo.")?
                        .parse()
                        .map_err(|_| "The source tempo must be a number.")?,
                );
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
                    "MixJam Techno\n\nUsage: mixjam-techno [gui|inspect|verify-fixtures|render|convert] [--app PATH] [--library PRODUCT=PATH] [--samples PATH] [--source-bpm BPM] [--mix PATH] [--output PATH] [--agent] [--agent-port PORT]\n       mixjam-techno agent [--port PORT] COMMAND [name=value ...]\n\nProducts: d1, d2, d3, d4, h1, t1, t2, t3.\nThe editor supports up to 64 tracks and 120, 140, 160, or 180 BPM.\nSelect a WAV folder with --samples PATH or Libraries in the app. No original installation is required. --source-bpm sets the fallback tempo (default 140) for unrecognized WAVs in folders whose loop lengths do not agree on one.\nrender and convert require --mix and --output. convert saves a .mixjam song.\nverify-fixtures reads T2 demos.\ngui --agent lets a local agent operate the window through port 47800; --agent-port selects another port. `agent help` lists the requests."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}. Use --help for usage.")),
        }
    }
    if source_bpm.is_some_and(|bpm| !bpm.is_finite() || bpm <= 0.0) {
        return Err("The source tempo must be finite and positive.".into());
    }
    let mut library = Library::empty();
    let mut startup_messages = Vec::new();
    if samples.is_none() && command == "gui" {
        match project_io::load_wave_config() {
            Ok(Some(config)) => {
                samples = Some(config.root);
                // A tempo given on the command line overrides the saved one.
                source_bpm = source_bpm.or(Some(config.source_bpm));
            }
            Ok(None) => {}
            Err(error) => startup_messages.push(error),
        }
    }
    let source_bpm = source_bpm.unwrap_or(140.0);
    // The window opens at once and scans its sample folder in the background.
    // Interface captures keep the synchronous scan so that they are complete.
    let background_scan = command == "gui"
        && samples.is_some()
        && std::env::var_os("MIXJAM_TECHNO_CAPTURE_TO").is_none();
    if let Some(root) = &samples
        && !background_scan
    {
        match project_io::open_wav_folder_cached(&project_io::data_root(), root, source_bpm, None) {
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
                    if library.source_root(binding.product).is_some()
                        && !library.is_sparse_wav_product(binding.product)
                    {
                        continue;
                    }
                    if let Err(error) = replace_provider(&mut library, &binding) {
                        startup_messages.push(error);
                    }
                }
            }
            Err(error) => startup_messages.push(error),
        }
    }
    // Providers named on the command line replace the same products in the WAV folder.
    let mut explicit_providers: Vec<Product> = additional_libraries
        .iter()
        .map(|binding| binding.product)
        .collect();
    if app.is_some() {
        explicit_providers.push(Product::T2);
    }
    if let Some(root) = &app {
        // An explicit provider replaces a T2 binding derived from the WAV folder.
        replace_provider(
            &mut library,
            &LibraryBinding {
                product: Product::T2,
                root: root.clone(),
            },
        )?;
    }
    for binding in additional_libraries {
        replace_provider(&mut library, &binding)?;
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
                    "audio_verified_identities":library.verified_count(),
                    "folder_tempos":library.folder_tempos().iter().map(|(folder,bpm,count)| serde_json::json!({"folder":folder,"bpm":bpm,"wavs":count})).collect::<Vec<_>>(),
                    "wavs_with_different_audio":library.unverified_waves().map(|(id,path)| serde_json::json!({"id":id,"path":path})).collect::<Vec<_>>(),
                    "skipped_wavs":library.skipped_waves().iter().map(|(path,error)| serde_json::json!({"path":path,"error":error})).collect::<Vec<_>>(),
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
            let target = output.ok_or("convert requires --output FILE.mixjam.")?;
            let (project, library) = project_io::open(&path, &library)?;
            project_io::save_with_library(&project, &library, &target)?;
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
            let mix = audio::prepare_project(&p, &library)?;
            mix.write_wav(&target, &p.master)?;
            println!(
                "Wrote {} stereo frames at {} Hz to {}",
                mix.frames(),
                audio::Mix::SAMPLE_RATE,
                target.display()
            );
            Ok(())
        }
        _ => {
            let project = if let Some(path) = &mix
                && !background_scan
            {
                let (project, resolved_library) = project_io::open(path, &library)?;
                library = resolved_library;
                project
            } else {
                Project {
                    libraries: library.bindings().to_vec(),
                    imported_samples: library.imported_samples().into(),
                    ..Project::default()
                }
            };
            #[cfg(feature = "gui")]
            {
                let size = std::env::var("MIXJAM_TECHNO_WINDOW_SIZE")
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
                // Claim the port before the window opens, so that a port in
                // use is a startup error.
                let agent_server = agent_port
                    .map(|port| agent::Server::start(port, &project_io::data_root()))
                    .transpose()?;
                if let Some(server) = &agent_server {
                    println!("Agent control: 127.0.0.1:{}", server.port());
                }
                let options = eframe::NativeOptions {
                    viewport: eframe::egui::ViewportBuilder::default()
                        .with_icon(
                            eframe::icon_data::from_png_bytes(include_bytes!(
                                "../assets/app/mixjam-techno.png"
                            ))
                            .map_err(|error| {
                                format!("Cannot load the application icon: {error}")
                            })?,
                        )
                        .with_inner_size(size)
                        .with_min_inner_size([960.0, 720.0]),
                    ..Default::default()
                };
                eframe::run_native(
                    "MIXJAM TECHNO",
                    options,
                    Box::new(move |cc| {
                        let mut session =
                            mixjam_techno::application::Session::new(Arc::new(library), project)?;
                        let mut messages = startup_messages;
                        session.prefer_providers(&explicit_providers);
                        if background_scan && let Some(root) = &samples {
                            if let Err(error) = session.start_wave_scan(root, source_bpm) {
                                messages.push(error);
                            }
                            if let Some(path) = &mix
                                && let Err(error) = session.open_after_scan(path)
                            {
                                messages.push(error);
                            }
                        }
                        let mut app = ui::MixJamApp::new(cc, session);
                        if let Some(server) = agent_server {
                            app.attach_agent(server, &cc.egui_ctx);
                        }
                        app.set_fallback_bpm(source_bpm);
                        app.set_startup_messages(&messages);
                        Ok(Box::new(app))
                    }),
                )
                .map_err(|e| e.to_string())
            }
            #[cfg(not(feature = "gui"))]
            {
                let _ = (
                    agent_port,
                    project,
                    library,
                    startup_messages,
                    background_scan,
                    mix,
                    source_bpm,
                );
                Err("This build has no GUI. Rebuild with the default features.".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_provider_replacement_keeps_scanned_samples() {
        let root = std::env::temp_dir().join(format!(
            "mixjam-provider-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 32],
        }
        .write_wav(&root.join("sample.wav"))
        .unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(&root, 140.0).unwrap();
        library
            .register_wav_product(
                Product::T2,
                &root,
                &[mixjam_techno::catalog::WaveMetadata {
                    native_id: 174,
                    code: "sample".into(),
                    name: "Sample".into(),
                    group_name: "Wave".into(),
                    bars: 1,
                    source_bpm: 140.0,
                    frame_count: 32,
                    pcm_digest: None,
                }],
            )
            .unwrap();
        let before = library.bindings().to_vec();
        assert!(
            replace_provider(
                &mut library,
                &LibraryBinding {
                    product: Product::T2,
                    root: root.join("missing")
                }
            )
            .is_err()
        );
        assert_eq!(library.bindings(), before);
        assert!(library.sample_by_id(174).is_some());
        std::fs::remove_dir_all(root).unwrap();
    }
}
