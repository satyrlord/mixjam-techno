//! Embed locally generated sample-identity catalogs when they are present.
//! A checkout without them still builds; it embeds empty catalogs instead.
use std::{env, fs, path::PathBuf};

const CATALOGS: [&str; 8] = ["d1", "d2", "d3", "d4", "h1", "t1", "t2", "t3"];

fn main() {
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
