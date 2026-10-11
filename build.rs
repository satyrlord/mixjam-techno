//! Embed locally generated sample-identity catalogs when they are present.
//! A checkout without them still builds; it embeds empty catalogs instead.
use std::{env, fs, path::PathBuf, process::Command};

const CATALOGS: [&str; 8] = ["d1", "d2", "d3", "d4", "h1", "t1", "t2", "t3"];
const APP_MAJOR_VERSION: u32 = 1;

fn git(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .output()
        .expect("App version requires Git and a checkout with complete history.");
    assert!(
        output.status.success(),
        "Cannot calculate app version. Build from a Git checkout with complete history: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout)
        .expect("Git version metadata must be UTF-8.")
        .trim()
        .to_owned()
}

fn app_version() {
    assert_eq!(
        env::var("CARGO_PKG_VERSION_MAJOR").ok(),
        Some(APP_MAJOR_VERSION.to_string()),
        "The Cargo.toml package version and APP_MAJOR_VERSION need the same major."
    );
    assert_eq!(
        git(&["rev-parse", "--is-shallow-repository"]),
        "false",
        "App version requires complete Git history. Run git fetch --unshallow before building."
    );
    // Resolve paths through Git so linked worktrees also rebuild after commits,
    // branch switches, detached HEAD changes, and reference packing.
    for name in ["HEAD", "refs", "packed-refs"] {
        let path = PathBuf::from(git(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            name,
        ]));
        if path.exists() {
            println!("cargo::rerun-if-changed={}", path.display());
        }
    }
    let count: u64 = git(&["rev-list", "--count", "HEAD"])
        .parse()
        .expect("Git commit count must be a positive integer.");
    assert!(count > 0, "App version requires at least one Git commit.");
    println!("cargo::rustc-env=MIXJAM_APP_VERSION={APP_MAJOR_VERSION}.{count:03}");
}

fn main() {
    app_version();
    println!("cargo::rerun-if-changed=assets/app/icon.rc");
    println!("cargo::rerun-if-changed=assets/app/mixjam-techno.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for(
            "assets/app/icon.rc",
            ["mixjam-techno"],
            embed_resource::NONE,
        )
        .manifest_required()
        .expect("Cannot embed the tribal mask application icon.");
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let folder = PathBuf::from("assets/catalog");
    println!("cargo::rerun-if-changed=build.rs");
    // Watch the catalog folder, not each optional file. A rerun rule for a
    // missing file makes Cargo re-run this script and recompile the crate on
    // every build of a checkout without generated catalogs.
    let watched = if folder.is_dir() {
        folder.clone()
    } else {
        PathBuf::from("assets")
    };
    if watched.is_dir() {
        println!("cargo::rerun-if-changed={}", watched.display());
    }
    println!("cargo::rustc-check-cfg=cfg(bundled_catalog)");
    let mut complete = true;
    for name in CATALOGS {
        let source = folder.join(format!("{name}.json"));
        let target = out.join(format!("{name}.json"));
        if source.is_file() {
            fs::copy(&source, &target).expect("copy sample catalog");
        } else {
            fs::write(&target, "[]").expect("write empty sample catalog");
            complete = false;
        }
    }
    if complete {
        println!("cargo::rustc-cfg=bundled_catalog");
    }
    let source = folder.join("source-tempos.json");
    let target = out.join("source-tempos.json");
    if source.is_file() {
        fs::copy(source, target).expect("copy source tempo catalog");
    } else {
        fs::write(target, "[]").expect("write empty source tempo catalog");
    }
}
