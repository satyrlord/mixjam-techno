mod identities;
mod importing;
mod names;
mod reading;
mod scanning;

use super::*;
use super::{
    naming::{
        compact_name, in_foreign_product_folder, infer_folder_tempo, named_tempo, octave_apart,
        slug, stated_pack_tempo,
    },
    scan::{ScanCache, ScanProgress, check_library_folders, probe_all, probe_wav},
};
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FOLDER: AtomicU64 = AtomicU64::new(0);

struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "standalone-wavs-{}-{}",
            std::process::id(),
            NEXT_FOLDER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn wave(&self, relative: &str, rate: u32, frames: usize) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        crate::audio::Pcm {
            sample_rate: rate,
            channels: 2,
            samples: vec![0.25; frames * 2],
        }
        .write_wav(&path)
        .unwrap();
        fs::canonicalize(path).unwrap()
    }
    fn mono(&self, relative: &str, samples: &[i16]) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        crate::audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: samples.iter().map(|s| f32::from(*s) / 32768.0).collect(),
        }
        .write_wav(&path)
        .unwrap();
        fs::canonicalize(path).unwrap()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn waveform(seed: i32, frames: usize) -> Vec<i16> {
    (0..frames as i32)
        .map(|i| ((i * 37 + seed * 11) % 200 - 100) as i16)
        .collect()
}

fn identity(
    native_id: u32,
    name: &str,
    group: &str,
    audio: &[i16],
) -> crate::catalog::WaveMetadata {
    crate::catalog::WaveMetadata {
        native_id,
        code: format!("CODE{native_id}"),
        label_key: crate::catalog::label_key(name),
        group_name: group.into(),
        bars: 1,
        source_bpm: 140.0,
        frame_count: audio.len() as u32,
        pcm_digest: Some(crate::catalog::pcm_identity_hex(audio)),
    }
}

fn catalog_file(
    name: &str,
    entries: Vec<crate::catalog::WaveMetadata>,
) -> crate::catalog::CatalogFile {
    // Each release's own source content gives it its own fingerprint.
    let sources = vec![crate::catalog::SourceFile {
        bytes: 1,
        sha256: crate::catalog::label_key(name).repeat(4),
    }];
    crate::catalog::CatalogFile {
        schema: crate::catalog::SCHEMA,
        product: Product::D1,
        release: name.into(),
        fingerprint: crate::catalog::fingerprint(&sources),
        sources,
        samples: entries,
    }
}
