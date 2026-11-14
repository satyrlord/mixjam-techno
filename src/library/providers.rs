//! Providers of native sounds: registering and removing a product's sounds,
//! adopting a catalog, and opening a packed archive or a first-generation
//! sample folder.

use super::{
    AudioSource, Library, Sample, decode_1252,
    import::unquote,
    naming::{canonical_group, compact_name, packed_folder, slug},
};
use crate::products::{IMPORTED_ID_START, LibraryBinding, Product};
use legacy_format::packed;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

impl Library {
    /// The provider folder that a song records for a product. A product read
    /// from WAV files is recorded as the folder that holds the song's `used`
    /// files, so the song reopens it without scanning the rest of a large
    /// sample folder.
    pub fn song_binding(&self, binding: &LibraryBinding, used: &HashSet<u32>) -> LibraryBinding {
        if self.source_root(binding.product) != Some(binding.root.as_path())
            || self.has_original_provider(binding.product)
            || !self.is_wav_product(binding.product)
        {
            return binding.clone();
        }
        let mut folders = used
            .iter()
            .filter(|id| Product::sample_key(**id).is_some_and(|(p, _)| p == binding.product))
            .filter_map(|id| self.sample_by_id(*id)?.wav_path.as_deref()?.parent());
        let Some(first) = folders.next() else {
            return binding.clone();
        };
        let mut common = first;
        for folder in folders {
            common = common
                .ancestors()
                .find(|ancestor| folder.starts_with(ancestor))
                .unwrap_or(&binding.root);
        }
        LibraryBinding {
            product: binding.product,
            root: common.to_path_buf(),
        }
    }

    pub fn source_root(&self, product: Product) -> Option<&Path> {
        self.bindings
            .iter()
            .find(|binding| binding.product == product)
            .map(|binding| binding.root.as_path())
    }

    pub fn remove_product(&mut self, product: Product) {
        self.samples
            .retain(|sample| Product::sample_key(sample.id).is_none_or(|(p, _)| p != product));
        self.sources
            .retain(|id, _| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.bindings.retain(|binding| binding.product != product);
        self.verified
            .retain(|id| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.unverified
            .retain(|id, _| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.reindex();
    }

    pub fn register(
        &mut self,
        product: Product,
        root: &Path,
        samples_root: &Path,
    ) -> Result<(), String> {
        let root = fs::canonicalize(root)
            .map_err(|e| format!("Cannot open {} library: {e}", product.label()))?;
        if let Some(existing) = self.source_root(product) {
            if existing == root {
                return Ok(());
            }
            return Err(format!(
                "{} already has a different library folder. Remove it before you select another folder.",
                product.label()
            ));
        }
        let other = if !crate::catalog::has_native_catalog(product, &root) {
            let mut wav = Self::empty();
            let catalogs = crate::catalog::catalogs();
            wav.import_wav_folder_using(
                &root,
                140.0,
                None,
                None,
                crate::source_tempo::SourceTempos::for_catalogs(catalogs),
            )?;
            if wav.identify_wav_product(
                product,
                &root,
                catalogs.identities(product),
                false,
                |id| catalogs.aliases(product, id),
            )? == 0
            {
                return Err(format!(
                    "The WAV folder has no identified {} samples.",
                    product.label()
                ));
            }
            wav.samples.retain(|sample| sample.id < IMPORTED_ID_START);
            wav.sources.retain(|id, _| *id < IMPORTED_ID_START);
            wav.reindex();
            wav
        } else {
            match product {
                Product::T2 => {
                    Self::open_archive(product, &root, samples_root, "\x72ave20", 140.0)?
                }
                Product::T1 => Self::open_t1(&root)?,
                _ => Self::from_catalog(product, &root)?,
            }
        };
        self.add_provider(other)
    }

    /// Add one provider's samples. Conflicting identities leave this library unchanged.
    fn add_provider(&mut self, other: Self) -> Result<(), String> {
        if other
            .samples
            .iter()
            .any(|sample| self.by_id.contains_key(&sample.id))
        {
            return Err("The library contains a conflicting sample identity.".into());
        }
        for sample in other.samples {
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample);
        }
        self.sources.extend(other.sources);
        self.bindings.extend(other.bindings);
        self.verified.extend(other.verified);
        self.unverified.extend(other.unverified);
        if self.archive_path.as_os_str().is_empty() {
            self.archive_path = other.archive_path;
        }
        Ok(())
    }

    /// Take a product's provider from a library that already loaded it, in
    /// place of reading its catalog or decoding its WAV folder again.
    /// Returns false when `loaded` has no usable provider for the product.
    pub(crate) fn register_loaded(
        &mut self,
        product: Product,
        loaded: &Self,
    ) -> Result<bool, String> {
        let Some(binding) = loaded
            .bindings
            .iter()
            .find(|binding| binding.product == product && binding.root.is_dir())
        else {
            return Ok(false);
        };
        if self.source_root(product).is_some() {
            return Err(format!(
                "{} already has a sample provider.",
                product.label()
            ));
        }
        let owned = |id: u32| Product::sample_key(id).is_some_and(|(owner, _)| owner == product);
        let mut provider = Self::empty();
        provider.samples = loaded
            .samples
            .iter()
            .filter(|sample| owned(sample.id))
            .cloned()
            .collect();
        provider.sources = loaded
            .sources
            .iter()
            .filter(|(id, _)| owned(**id))
            .map(|(id, source)| (*id, source.clone()))
            .collect();
        provider.bindings = vec![binding.clone()];
        provider.verified = loaded
            .verified
            .iter()
            .copied()
            .filter(|id| owned(*id))
            .collect();
        provider.unverified = loaded
            .unverified
            .iter()
            .filter(|(id, _)| owned(**id))
            .map(|(id, path)| (*id, path.clone()))
            .collect();
        if loaded.archive_path.starts_with(&binding.root) {
            provider.archive_path = loaded.archive_path.clone();
        }
        self.add_provider(provider)?;
        Ok(true)
    }

    /// Keep any loaded identities that a replacement provider does not have.
    /// Its own identities and binding retain precedence.
    pub(crate) fn supplement_loaded(&mut self, product: Product, loaded: &Self) {
        self.supplement_loaded_matching(product, loaded, |_| true);
    }

    /// Keep only selected loaded identities that the current provider lacks.
    pub(crate) fn supplement_loaded_ids(
        &mut self,
        product: Product,
        loaded: &Self,
        ids: &HashSet<u32>,
    ) {
        self.supplement_loaded_matching(product, loaded, |id| ids.contains(&id));
    }

    fn supplement_loaded_matching(
        &mut self,
        product: Product,
        loaded: &Self,
        selected: impl Fn(u32) -> bool,
    ) {
        for sample in &loaded.samples {
            if !selected(sample.id)
                || Product::sample_key(sample.id).is_none_or(|(owner, _)| owner != product)
                || self.by_id.contains_key(&sample.id)
            {
                continue;
            }
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample.clone());
            if let Some(source) = loaded.sources.get(&sample.id) {
                self.sources.insert(sample.id, source.clone());
            }
            if loaded.verified.contains(&sample.id) {
                self.verified.insert(sample.id);
            }
            self.unverified.remove(&sample.id);
        }
    }

    fn from_catalog(product: Product, root: &Path) -> Result<Self, String> {
        let mut library = Self::empty();
        for native in crate::catalog::load(product, root)? {
            let id = product.global_sample_id(native.native_id)?;
            let (source, offset, length) = match native.audio {
                crate::catalog::NativeAudio::Archive {
                    path,
                    offset,
                    length,
                } => {
                    if library.archive_path.as_os_str().is_empty() {
                        library.archive_path = path.clone();
                    }
                    (AudioSource::Archive { path }, offset, length)
                }
                crate::catalog::NativeAudio::Packed { path } => {
                    let length = fs::metadata(&path).map_err(|e| e.to_string())?.len();
                    (AudioSource::T1Packed { path }, 0, length)
                }
            };
            let sample = Sample {
                id,
                code: native.code,
                name: native.name,
                bars: native.bars,
                group_name: native.group_name.trim().to_owned(),
                offset,
                length,
                frame_count: native.frame_count,
                sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
                source_bpm: native.source_bpm,
                channels: 1,
                wav_path: None,
                digest: None,
            };
            library.sources.insert(id, source);
            library.by_id.insert(id, library.samples.len());
            library.samples.push(sample);
        }
        library.bindings.push(LibraryBinding {
            product,
            root: root.to_path_buf(),
        });
        Ok(library)
    }

    pub fn open_t1(root: &Path) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let native = legacy_format::library::Library::load(&root).map_err(|e| e.to_string())?;
        let mut library = Self::empty();
        for metadata in native.samples() {
            let path = root.join("\x50XD").join(&metadata.path);
            let mut file = fs::File::open(&path)
                .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            let mut prefix = vec![
                0;
                file.metadata()
                    .map_err(|e| e.to_string())?
                    .len()
                    .min(packed::MAX_HEADER_BYTES as u64) as usize
            ];
            file.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            let header = packed::parse_header(&prefix).map_err(|e| e.to_string())?;
            let id = Product::T1.global_sample_id(u32::from(metadata.id))?;
            let sample = Sample {
                id,
                code: metadata.path.clone(),
                name: metadata.labels.join(" ").trim().into(),
                bars: metadata.nominal_bars.unwrap_or(1).max(1),
                group_name: canonical_group(&metadata.category),
                offset: 0,
                length: file.metadata().map_err(|e| e.to_string())?.len(),
                frame_count: u32::try_from(header.decoded_samples)
                    .map_err(|_| "The native sample is too long.")?,
                sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
                source_bpm: 180.0,
                channels: 1,
                wav_path: None,
                digest: None,
            };
            library.sources.insert(id, AudioSource::T1Packed { path });
            library.by_id.insert(id, library.samples.len());
            library.samples.push(sample);
        }
        library.bindings.push(LibraryBinding {
            product: Product::T1,
            root,
        });
        Ok(library)
    }

    fn open_archive(
        product: Product,
        root: &Path,
        samples_root: &Path,
        archive_name: &str,
        source_bpm: f64,
    ) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let packed = packed_folder(&root);
        let archive_path = packed.join(archive_name);
        let index = fs::read(packed.join(format!("{archive_name}.inf")))
            .map_err(|e| format!("Cannot read the original sample index: {e}"))?;
        let text = decode_1252(&index);
        let lines: Vec<_> = text.lines().collect();
        let start = lines
            .iter()
            .position(|line| *line == "[SAMPLES]")
            .ok_or("The sample index has no SAMPLES section.")?
            + 1;
        if (lines.len() - start) % 12 != 0 {
            return Err("The sample index has an incomplete record.".into());
        }
        let mut archive = fs::File::open(&archive_path)
            .map_err(|e| format!("Cannot read the original sample archive: {e}"))?;
        let archive_size = archive.metadata().map_err(|e| e.to_string())?.len();
        let mut endpoint = 0u64;
        let mut samples = Vec::new();
        let mut by_id = HashMap::new();
        let mut embedded_names = Vec::new();
        for row in lines[start..].as_chunks::<12>().0 {
            let number = |i: usize| {
                row[i]
                    .parse::<u64>()
                    .map_err(|_| "The sample index has an invalid number.".to_string())
            };
            let id = product.global_sample_id(
                u32::try_from(number(0)?).map_err(|_| "The sample ID is too large.")?,
            )?;
            let offset = number(3)?;
            let length = number(4)?;
            if !matches!(number(1)?, 3 | 6) || offset != endpoint || length < 16 {
                return Err(format!("Sample {id} has an invalid archive range or type."));
            }
            endpoint = offset
                .checked_add(length)
                .filter(|end| *end <= archive_size)
                .ok_or("A sample range exceeds the archive.")?;
            archive
                .seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            let mut prefix = vec![0; length.min(packed::MAX_HEADER_BYTES as u64) as usize];
            archive.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            let header = packed::parse_header(&prefix)
                .ok()
                .filter(|header| header.kind == packed::HeaderKind::Metadata)
                .ok_or_else(|| format!("Sample {id} has an invalid packed header."))?;
            let embedded = decode_1252(&header.metadata);
            let fields: Vec<_> = embedded.split("\r\n").collect();
            if fields.len() != 8 {
                return Err(format!("Sample {id} has invalid packed metadata."));
            }
            let frame_count = u32::try_from(header.decoded_samples)
                .map_err(|_| format!("Sample {id} has too many frames."))?;
            if frame_count == 0 {
                return Err(format!("Sample {id} has no audio frames."));
            }
            let name = format!("{} {}", unquote(row[5]), unquote(row[6]))
                .trim()
                .to_string();
            embedded_names.push(format!("{} {}", fields[0], fields[1]).trim().to_string());
            let sample = Sample {
                id,
                code: unquote(row[2]).to_string(),
                name,
                bars: u32::try_from(number(7)?).map_err(|_| "The bar count is too large.")?,
                group_name: fields[4].to_string(),
                offset,
                length,
                frame_count,
                sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
                source_bpm,
                channels: 1,
                wav_path: None,
                digest: None,
            };
            let group = u8::try_from(number(8)?).map_err(|_| "The sample group is too large.")?;
            if group == 0 || sample.bars == 0 || by_id.insert(id, samples.len()).is_some() {
                return Err(format!(
                    "Sample {id} has an invalid group, bar count, or duplicate ID."
                ));
            }
            samples.push(sample);
        }
        if endpoint != archive_size || samples.is_empty() {
            return Err("The sample index does not cover the complete archive.".into());
        }
        if samples_root.join("metadata.json").exists() {
            map_waves(&mut samples, &embedded_names, samples_root)?;
        }
        let sources = samples
            .iter()
            .map(|sample| {
                (
                    sample.id,
                    AudioSource::Archive {
                        path: archive_path.clone(),
                    },
                )
            })
            .collect();
        Ok(Self {
            samples,
            archive_path,
            by_id,
            sources,
            bindings: vec![LibraryBinding { product, root }],
            ..Self::empty()
        })
    }

    /// Rebuild the ID index after samples were removed or replaced.
    pub(super) fn reindex(&mut self) {
        self.by_id = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
    }
}

#[derive(Deserialize)]
struct WaveIndex {
    samples: Vec<WaveEntry>,
}

#[derive(Deserialize)]
struct WaveEntry {
    filename: String,
    alias: String,
    channel: String,
}

type WaveCandidate = (PathBuf, String, u32, u16, u32);

fn map_waves(samples: &mut [Sample], embedded_names: &[String], root: &Path) -> Result<(), String> {
    let file = fs::File::open(root.join("metadata.json")).map_err(|e| e.to_string())?;
    let index: WaveIndex = serde_json::from_reader(file)
        .map_err(|e| format!("Cannot read the WAV sample index: {e}"))?;
    let mut lookup: HashMap<(String, String), Vec<WaveCandidate>> = HashMap::new();
    for entry in index.samples {
        let relative = Path::new(&entry.filename);
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("The WAV sample index contains an invalid path.".into());
        }
        let path = root.join(relative);
        let Some(layout) = fs::File::open(&path).ok().and_then(|file| {
            let length = file.metadata().ok()?.len();
            crate::wav::read_layout(&mut std::io::BufReader::new(file), length).ok()
        }) else {
            continue;
        };
        let spec = layout.spec;
        let key = (canonical_group(&entry.channel), compact_name(&entry.alias));
        lookup.entry(key).or_default().push((
            path,
            slug(&entry.alias),
            spec.sample_rate,
            spec.channels,
            layout.frames() as u32,
        ));
    }
    let mut used_paths = HashSet::new();
    for (sample, embedded) in samples.iter_mut().zip(embedded_names) {
        let mut candidates: Vec<(&WaveCandidate, u8)> = Vec::new();
        for name in [&sample.name, embedded] {
            let key = (canonical_group(&sample.group_name), compact_name(name));
            if let Some(entries) = lookup.get(&key) {
                for entry in entries {
                    if entry.2 != legacy_format::NATIVE_SAMPLE_RATE
                        || entry.3 != 1
                        || entry.4 != sample.frame_count
                    {
                        continue;
                    }
                    let score = u8::from(entry.1 == slug(name));
                    if !candidates
                        .iter()
                        .any(|(candidate, _)| candidate.0 == entry.0)
                    {
                        candidates.push((entry, score));
                    }
                }
            }
        }
        let best = candidates.iter().map(|(_, score)| *score).max();
        let mut best_candidates = candidates.iter().filter(|(_, score)| Some(*score) == best);
        let selected = best_candidates.next().map(|(entry, _)| *entry);
        if best_candidates.next().is_some() {
            continue;
        }
        if let Some(entry) = selected {
            if !used_paths.insert(entry.0.clone()) {
                continue;
            }
            sample.wav_path = Some(entry.0.clone());
            sample.sample_rate = entry.2;
            sample.channels = entry.3;
        }
    }
    Ok(())
}
