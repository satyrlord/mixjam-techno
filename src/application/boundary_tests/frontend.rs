//! The frontend source reaches the backend only through the application boundary.

#[test]
fn frontend_source_can_only_reach_the_application_boundary() {
    // Every frontend file is checked, including files that later splits add.
    // Test modules are exempt: they drive the backend to set up fixtures.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut frontend = Vec::new();
    let mut folders = vec![root.join("ui")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name.contains("tests") || name.contains("bench") {
                continue;
            }
            if path.is_dir() {
                folders.push(path);
            } else if name.ends_with(".rs") {
                frontend.push(path);
            }
        }
    }
    frontend.extend([root.join("ui.rs"), root.join("ui_icons.rs")]);
    let frontend: Vec<(String, String)> = frontend
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (name, std::fs::read_to_string(&path).unwrap())
        })
        .collect();
    assert!(frontend.len() > 60, "{} frontend files", frontend.len());
    // Screen capture writes its image file itself, so it is exempt from
    // the I/O patterns but not from the backend path check below.
    for (name, source) in frontend.iter().filter(|(name, _)| name != "ui/capture.rs") {
        for forbidden in [
            ".editor",
            ".transact(",
            "crate::audio",
            "crate::engine",
            "crate::library",
            "crate::project_io",
            "crate::recording",
            "crate::effects",
            "crate::synth",
            "crate::sample_edit",
            "std::fs::",
            "serde_json::",
            "render_project(",
            "read_pcm(",
            "Recorder::",
            "Player::",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} bypasses the application boundary through {forbidden}"
            );
        }
    }
    // The native binary names the backend as `mixjam_techno`. Every such
    // path, in an import or inline, must go through the application API.
    for (name, source) in &frontend {
        for line in source.lines() {
            assert_eq!(
                line.matches("mixjam_techno::").count(),
                line.matches("mixjam_techno::application::").count(),
                "{name} reaches a backend implementation directly: {line}"
            );
        }
    }
    let library = include_str!("../../lib.rs");
    assert!(
        !library.contains("pub mod ui;"),
        "The production library must remain headless."
    );
}
