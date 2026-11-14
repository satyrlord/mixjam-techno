//! Transactional project opening and local installation registrations.
mod data_folder;
mod open;
mod retained;
mod save;
#[cfg(test)]
mod tests;

pub use data_folder::{
    WaveFolderConfig, data_root, load_library_config, load_scan_cache, load_section_bookmarks,
    load_song_folder, load_wave_config, load_wave_config_in, open_wav_folders_cached,
    reference_samples_root, save_library_config, save_scan_cache, save_song_folder,
    save_wave_config_in,
};
#[cfg(test)]
pub(crate) use data_folder::{default_root, save_section_bookmarks};
pub use open::{detect_product, open, open_song, resolve};
pub(crate) use retained::retain_manual_wave;
pub use save::{save, save_with_library};

fn has_song_extension(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mixjam"))
}
