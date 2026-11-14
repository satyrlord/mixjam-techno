//! Saving a song, with each product provider recorded as the folder that holds its files.
use super::{data_root, has_song_extension, retained::retain_native_provider};
use crate::{engine::Project, library::Library, products::Product};
use std::{collections::HashSet, path::Path};

/// [`save`] with each product provider recorded as the folder that holds the
/// song's files in this library. See [`Library::song_binding`].
pub fn save_with_library(project: &Project, library: &Library, path: &Path) -> Result<(), String> {
    save_with_library_in(project, library, path, &data_root())
}

pub(super) fn save_with_library_in(
    project: &Project,
    library: &Library,
    path: &Path,
    storage: &Path,
) -> Result<(), String> {
    project.validate()?;
    if !has_song_extension(path) {
        return Err("Save the song with the .mixjam extension.".into());
    }
    let used: HashSet<_> = project.clips.iter().map(|clip| clip.sample_id).collect();
    let mut saved = project.clone();
    saved.libraries = saved
        .libraries
        .iter()
        .map(|binding| library.song_binding(binding, &used))
        .collect();
    for binding in &mut saved.libraries {
        let files: Vec<_> = used
            .iter()
            .filter(|id| {
                Product::sample_key(**id).is_some_and(|(product, _)| product == binding.product)
                    && project.missing_sample(**id).is_none()
            })
            .filter_map(|id| {
                let sample = library.sample_by_id(*id)?;
                Some((*id, sample.wav_path.as_deref()?))
            })
            .collect();
        let original_provider = library.has_original_provider(binding.product);
        let uses_original = original_provider
            && used.iter().any(|id| {
                Product::sample_key(*id).is_some_and(|(product, _)| product == binding.product)
                    && project.missing_sample(*id).is_none()
                    && library
                        .sample_by_id(*id)
                        .is_some_and(|sample| sample.wav_path.is_none())
            });
        if uses_original && !files.is_empty() {
            return Err(format!(
                "Cannot save {} sounds from both an original installation and supplemental WAVs. \
                 This song format can save only one provider folder for each product.",
                binding.product.label()
            ));
        }
        // A packed provider never scans supplemental WAVs, including WAVs
        // inside its own folder. Retain them when no packed sound is used.
        if !files.is_empty()
            && (original_provider
                || files
                    .iter()
                    .any(|(_, file)| !file.starts_with(&binding.root)))
        {
            binding.root = retain_native_provider(binding.product, files, storage)?;
        }
    }
    save(&saved, path)
}

/// A saved song references its used audio, not every sound in the browser palette.
pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    if !has_song_extension(path) {
        return Err("Save the song with the .mixjam extension.".into());
    }
    let mut saved = project.clone();
    saved.prune_sample_conflicts();
    for imported in &mut saved.imported_samples {
        if imported.source_sample_rate.is_none()
            && let Ok(ready) = Library::check_import(imported)
        {
            imported.source_sample_rate = Some(ready.sample_rate);
        }
    }
    let used: HashSet<_> = saved.clips.iter().map(|clip| clip.sample_id).collect();
    saved
        .imported_samples
        .retain(|sample| used.contains(&sample.id));
    saved
        .missing_samples
        .retain(|sample| used.contains(&sample.id));
    saved.libraries.retain(|binding| {
        saved.clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id)
                .is_some_and(|(product, _)| product == binding.product)
        })
    });
    crate::mixjam::save(&saved, path)
}
