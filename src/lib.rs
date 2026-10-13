pub mod agent;
pub mod application;
mod atomic_file;
pub mod audio;
pub mod catalog;
pub mod compat;
pub mod effects;
pub mod engine;
pub mod legacy_mix;
pub mod library;
pub mod mixjam;
pub mod packed;
pub mod products;
pub mod project_io;
pub mod recording;
pub mod sample_edit;
mod source_tempo;
pub mod synth;

/// Application version: manual major and build-time Git commit count.
pub const APP_VERSION: &str = env!("MIXJAM_APP_VERSION");

#[cfg(all(test, feature = "gui"))]
extern crate self as mixjam_techno;
#[cfg(all(test, feature = "gui"))]
mod ui_icons;
#[cfg(all(test, feature = "gui"))]
mod ui_test_harness {
    include!("ui.rs");
    include!("ui/tests.rs");
    #[path = "../ui/window_lifecycle_tests.rs"]
    mod window_lifecycle_tests;
}
