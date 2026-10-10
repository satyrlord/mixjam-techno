//! Embed locally generated sample-identity catalogs when they are present.
//! A checkout without them still builds; it embeds empty catalogs instead.
use std::{env, fs, path::PathBuf};

const CATALOGS: [&str; 8] = ["d1", "d2", "d3", "d4", "h1", "t1", "t2", "t3"];

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let folder = PathBuf::from("assets/catalog");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(bundled_catalog)");
    let mut complete = true;
    for name in CATALOGS {
        let source = folder.join(format!("{name}.json"));
        println!("cargo::rerun-if-changed={}", source.display());
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
}
