use crate::products::{IMPORTED_ID_START, LibraryBinding, Product};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Sample {
    pub id: u32,
    pub code: String,
    pub name: String,
    pub bars: u32,
    pub group: u8,
    pub group_name: String,
    pub offset: u64,
    pub length: u64,
    pub frame_count: u32,
    pub sample_rate: u32,
    pub source_bpm: f64,
    pub channels: u16,
    pub wav_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Library {
    pub samples: Vec<Sample>,
    pub archive_path: PathBuf,
    pub unresolved_samples: Vec<u32>,
    by_id: HashMap<u32, usize>,
    sources: HashMap<u32, AudioSource>,
    bindings: Vec<LibraryBinding>,
    wav_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
enum AudioSource {
    Archive {
        path: PathBuf,
    },
    T1Packed {
        path: PathBuf,
    },
    Wave {
        path: PathBuf,
        origin: Option<Product>,
    },
}

impl Library {
    #[cfg(test)]
    pub(crate) fn fixture(samples: Vec<Sample>) -> Self {
        let by_id = samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        let unresolved_samples = samples
            .iter()
            .filter(|sample| sample.wav_path.is_none())
            .map(|sample| sample.id)
            .collect();
        let sources = samples_sources(&samples);
        Self {
            samples,
            archive_path: PathBuf::new(),
            unresolved_samples,
            by_id,
            sources,
            bindings: Vec::new(),
            wav_root: None,
        }
    }

    pub fn open(root: &Path) -> Result<Self, String> {
        Self::open_with_samples(
            root,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples"),
        )
    }

    pub fn open_with_samples(root: &Path, samples_root: &Path) -> Result<Self, String> {
        Self::open_archive(Product::T2, root, samples_root, "\x72ave20", 140.0)
    }

    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            archive_path: PathBuf::new(),
            unresolved_samples: Vec::new(),
            by_id: HashMap::new(),
            sources: HashMap::new(),
            bindings: Vec::new(),
            wav_root: None,
        }
    }

    pub fn bindings(&self) -> &[LibraryBinding] {
        &self.bindings
    }

    pub fn wav_root(&self) -> Option<&Path> {
        self.wav_root.as_deref()
    }

    pub fn is_wav_product(&self, product: Product) -> bool {
        self.sources.iter().any(|(id, source)| {
            Product::sample_key(*id).is_some_and(|(owner, _)| owner == product)
                && matches!(source, AudioSource::Wave { .. })
        })
    }

    pub fn imported_samples(&self) -> Vec<crate::engine::ImportedSample> {
        self.samples
            .iter()
            .filter_map(|sample| {
                if sample.id < IMPORTED_ID_START {
                    return None;
                }
                let AudioSource::Wave { path, origin } = self.sources.get(&sample.id)? else {
                    return None;
                };
                Some(crate::engine::ImportedSample {
                    id: sample.id,
                    name: sample.name.clone(),
                    group_name: sample.group_name.clone(),
                    path: path.clone(),
                    source_bpm: sample.source_bpm,
                    origin: *origin,
                })
            })
            .collect()
    }

    /// Restore unused, already validated palette WAVs after a project is opened.
    /// Project identities take precedence; palette ID conflicts receive fresh IDs.
    pub fn restore_wav_palette(
        &mut self,
        palette: &Library,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        let mut paths: HashSet<_> = self
            .samples
            .iter()
            .filter_map(|sample| sample.wav_path.clone())
            .collect();
        let mut next = self
            .samples
            .iter()
            .map(|sample| sample.id)
            .filter(|id| *id >= IMPORTED_ID_START)
            .max()
            .unwrap_or(IMPORTED_ID_START - 1);
        let mut additions = Vec::new();
        for sample in &palette.samples {
            if sample.id < IMPORTED_ID_START {
                continue;
            }
            let Some(AudioSource::Wave { path, origin }) = palette.sources.get(&sample.id) else {
                continue;
            };
            if !paths.insert(path.clone()) {
                continue;
            }
            next = next
                .checked_add(1)
                .ok_or("Imported sample ID limit reached.")?;
            let mut restored = sample.clone();
            restored.id = next;
            restored.code = format!("IMPORT{next}");
            let metadata = crate::engine::ImportedSample {
                id: next,
                name: restored.name.clone(),
                group_name: restored.group_name.clone(),
                path: path.clone(),
                source_bpm: restored.source_bpm,
                origin: *origin,
            };
            additions.push((restored, metadata));
        }
        let mut restored_metadata = Vec::with_capacity(additions.len());
        for (sample, metadata) in additions {
            self.sources.insert(
                sample.id,
                AudioSource::Wave {
                    path: metadata.path.clone(),
                    origin: metadata.origin,
                },
            );
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample);
            restored_metadata.push(metadata);
        }
        Ok(restored_metadata)
    }

    pub fn open_wav_folder(root: &Path, source_bpm: f64) -> Result<Self, String> {
        let mut library = Self::empty();
        library.import_wav_folder(root, source_bpm)?;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        library.wav_root = Some(root.clone());
        for product in Product::ALL {
            library.register_wav_product(
                product,
                &root,
                &crate::catalog::wave_metadata(product)?,
            )?;
        }
        // A display label and duration cannot distinguish different native IDs.
        // Keep ambiguous files as ordinary WAVs until their path identifies them.
        let mut owners: HashMap<PathBuf, Vec<u32>> = HashMap::new();
        for sample in &library.samples {
            if sample.id < IMPORTED_ID_START {
                owners
                    .entry(sample.wav_path.clone().unwrap())
                    .or_default()
                    .push(sample.id);
            }
        }
        let ambiguous: HashSet<_> = owners
            .values()
            .filter(|ids| ids.len() > 1)
            .flatten()
            .copied()
            .collect();
        library
            .samples
            .retain(|sample| !ambiguous.contains(&sample.id));
        library.bindings.retain(|binding| {
            library.samples.iter().any(|sample| {
                Product::sample_key(sample.id)
                    .is_some_and(|(product, _)| product == binding.product)
            })
        });
        let native_paths: HashSet<_> = library
            .samples
            .iter()
            .filter(|sample| sample.id < IMPORTED_ID_START)
            .filter_map(|sample| sample.wav_path.clone())
            .collect();
        library.samples.retain(|sample| {
            sample.id < IMPORTED_ID_START
                || sample
                    .wav_path
                    .as_ref()
                    .is_none_or(|path| !native_paths.contains(path))
        });
        library.by_id = library
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        library
            .sources
            .retain(|id, _| library.by_id.contains_key(id));
        Ok(library)
    }

    /// Attach factual native identities to already validated WAVs in this root.
    /// A partial folder is valid. A song reports the identities that it lacks.
    pub fn register_wav_product(
        &mut self,
        product: Product,
        root: &Path,
        metadata: &[crate::catalog::WaveMetadata],
    ) -> Result<usize, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        if self.source_root(product).is_some() {
            return Err(format!(
                "{} already has a sample provider.",
                product.label()
            ));
        }
        let mut by_name: HashMap<String, Vec<&Sample>> = HashMap::new();
        let mut seen_paths = HashSet::new();
        for sample in &self.samples {
            let Some(path) = &sample.wav_path else {
                continue;
            };
            if !path.starts_with(&root) || !seen_paths.insert(path) {
                continue;
            }
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            by_name.entry(compact_name(&name)).or_default().push(sample);
        }
        let mut resolved = Vec::new();
        let mut seen_ids = HashSet::new();
        for entry in metadata {
            let id = product.global_sample_id(entry.native_id)?;
            if !seen_ids.insert(id)
                || entry.bars == 0
                || entry.frame_count == 0
                || !entry.source_bpm.is_finite()
                || entry.source_bpm <= 0.0
            {
                return Err(
                    "The bundled sample identity has invalid or duplicate metadata.".into(),
                );
            }
            let code = entry.code.replace('\\', "/");
            let code_name = Path::new(&code)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let mut candidates: HashMap<&PathBuf, (&Sample, u8)> = HashMap::new();
            for (name, score) in [
                (compact_name(&entry.name), 1),
                (compact_name(&code_name), 2),
            ] {
                for sample in by_name.get(&name).into_iter().flatten() {
                    let path = sample.wav_path.as_ref().unwrap();
                    if score < 2
                        && path
                            .ancestors()
                            .skip(1)
                            .filter_map(Path::file_name)
                            .find_map(|name| product_folder_hint(&name.to_string_lossy()))
                            .is_some_and(|hint| hint != product)
                    {
                        continue;
                    }
                    let category_matches = score == 2
                        || canonical_group(&sample.group_name)
                            == canonical_group(&entry.group_name)
                        || sample.group_name == "Wave"
                        || product_folder_hint(&sample.group_name).is_some();
                    let measured_frames =
                        f64::from(sample.frame_count) * 44_100.0 / f64::from(sample.sample_rate);
                    if category_matches
                        && (measured_frames - f64::from(entry.frame_count)).abs() <= 4.0
                    {
                        candidates.insert(sample.wav_path.as_ref().unwrap(), (sample, score));
                    }
                }
            }
            let best = candidates
                .values()
                .map(|(_, score)| *score)
                .max()
                .unwrap_or(0);
            candidates.retain(|_, (_, score)| *score == best);
            if candidates.len() > 1 {
                continue;
            }
            let Some((path, (candidate, score))) = candidates.into_iter().next() else {
                continue;
            };
            let mut sample = candidate.clone();
            sample.id = id;
            sample.code = entry.code.clone();
            sample.name = entry.name.clone();
            sample.group_name = entry.group_name.clone();
            sample.bars = entry.bars;
            sample.source_bpm = entry.source_bpm;
            resolved.push((sample, path.clone(), score));
        }
        let mut path_scores: HashMap<PathBuf, (u8, usize)> = HashMap::new();
        for (_, path, score) in &resolved {
            let (best, count) = path_scores.entry(path.clone()).or_default();
            if score > best {
                *best = *score;
                *count = 1;
            } else if score == best {
                *count += 1;
            }
        }
        resolved.retain(|(_, path, score)| path_scores.get(path) == Some(&(*score, 1)));
        if resolved
            .iter()
            .any(|(sample, _, _)| self.by_id.contains_key(&sample.id))
        {
            return Err("The WAV folder contains a conflicting sample identity.".into());
        }
        let count = resolved.len();
        for (sample, path, _) in resolved {
            self.sources
                .insert(sample.id, AudioSource::Wave { path, origin: None });
            self.by_id.insert(sample.id, self.samples.len());
            self.samples.push(sample);
        }
        if count > 0 {
            self.bindings.push(LibraryBinding { product, root });
        }
        Ok(count)
    }

    pub fn source_root(&self, product: Product) -> Option<&Path> {
        self.bindings
            .iter()
            .find(|binding| binding.product == product)
            .map(|binding| binding.root.as_path())
    }

    pub fn remove_product(&mut self, product: Product) -> Result<(), String> {
        self.samples
            .retain(|sample| Product::sample_key(sample.id).is_none_or(|(p, _)| p != product));
        self.sources
            .retain(|id, _| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.bindings.retain(|binding| binding.product != product);
        self.unresolved_samples
            .retain(|id| Product::sample_key(*id).is_none_or(|(p, _)| p != product));
        self.by_id = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        Ok(())
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
            wav.import_wav_folder(&root, 140.0)?;
            if wav.register_wav_product(product, &root, &crate::catalog::wave_metadata(product)?)?
                == 0
            {
                return Err(format!(
                    "The WAV folder has no identified {} samples.",
                    product.label()
                ));
            }
            wav.samples.retain(|sample| sample.id < IMPORTED_ID_START);
            wav.sources.retain(|id, _| *id < IMPORTED_ID_START);
            wav.by_id = wav
                .samples
                .iter()
                .enumerate()
                .map(|(index, sample)| (sample.id, index))
                .collect();
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
        self.unresolved_samples.extend(other.unresolved_samples);
        if self.archive_path.as_os_str().is_empty() {
            self.archive_path = other.archive_path;
        }
        Ok(())
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
                group: 9,
                group_name: native.group_name.trim().to_owned(),
                offset,
                length,
                frame_count: native.frame_count,
                sample_rate: 44100,
                source_bpm: native.source_bpm,
                channels: 1,
                wav_path: None,
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
            let mut prefix =
                vec![0; file.metadata().map_err(|e| e.to_string())?.len().min(512) as usize];
            file.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            let header = legacy_format::packed::parse_header(&prefix).map_err(|e| e.to_string())?;
            let id = Product::T1.global_sample_id(u32::from(metadata.id))?;
            let sample = Sample {
                id,
                code: metadata.path.clone(),
                name: metadata.labels.join(" ").trim().into(),
                bars: metadata.nominal_bars.unwrap_or(1).max(1),
                group: 9,
                group_name: canonical_group(&metadata.category),
                offset: 0,
                length: file.metadata().map_err(|e| e.to_string())?.len(),
                frame_count: u32::try_from(header.decoded_samples)
                    .map_err(|_| "The native sample is too long.")?,
                sample_rate: 44100,
                source_bpm: 180.0,
                channels: 1,
                wav_path: None,
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
        let packed = root.join("\x65Jay").join("\x50XD");
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
            let mut prefix = [0; 5];
            archive.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            if &prefix[..4] != b"t\x50xD" || prefix[4] < 2 {
                return Err(format!("Sample {id} has an invalid packed header."));
            }
            let boundary = prefix[4] as usize + 3;
            if (boundary + 9) as u64 > length {
                return Err(format!("Sample {id} has a truncated packed header."));
            }
            let mut header = vec![0; boundary + 9 - 5];
            archive.read_exact(&mut header).map_err(|e| e.to_string())?;
            let text_length = boundary - 5;
            if header[text_length..text_length + 3] != [0, 0, b'T'] {
                return Err(format!("Sample {id} has an unsupported packed payload."));
            }
            let embedded = decode_1252(&header[..text_length]);
            let fields: Vec<_> = embedded.split("\r\n").collect();
            if fields.len() != 8 {
                return Err(format!("Sample {id} has invalid packed metadata."));
            }
            let frame_count =
                u32::from_le_bytes(header[text_length + 3..text_length + 7].try_into().unwrap());
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
                group: u8::try_from(number(8)?).map_err(|_| "The sample group is too large.")?,
                group_name: fields[4].to_string(),
                offset,
                length,
                frame_count,
                sample_rate: 44100,
                source_bpm,
                channels: 1,
                wav_path: None,
            };
            if sample.group == 0 || sample.bars == 0 || by_id.insert(id, samples.len()).is_some() {
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
        let unresolved_samples = samples
            .iter()
            .filter(|s| s.wav_path.is_none())
            .map(|s| s.id)
            .collect();
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
            unresolved_samples,
            by_id,
            sources,
            bindings: vec![LibraryBinding { product, root }],
            wav_root: None,
        })
    }

    pub fn sample_by_id(&self, id: u32) -> Option<&Sample> {
        self.by_id
            .get(&id)
            .and_then(|index| self.samples.get(*index))
    }

    pub fn read_pcm(&self, id: u32) -> Result<crate::audio::Pcm, String> {
        let sample = self
            .sample_by_id(id)
            .ok_or("The sample is not in the library.")?;
        let source = self
            .sources
            .get(&id)
            .ok_or("The sample has no audio source.")?;
        let archive_path = match source {
            AudioSource::Wave {
                path,
                origin: Some(Product::T1),
            } => return read_t1_wave(path),
            AudioSource::Wave { path, .. } => return crate::audio::read_wav(path),
            AudioSource::T1Packed { path } => {
                if fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
                    return Err("The compressed sample is too large.".into());
                }
                let bytes = fs::read(path).map_err(|e| e.to_string())?;
                // Shared-library audio ends at a complete physical opcode, even
                // when a native header declares a longer nominal duration.
                let decoded = legacy_format::source::SampleSource::from_packed_file_audio(&bytes)
                    .map_err(|e| e.to_string())?;
                return Ok(crate::audio::Pcm {
                    sample_rate: 44100,
                    channels: 1,
                    samples: decoded
                        .samples()
                        .iter()
                        .map(|s| *s as f32 / 32768.0)
                        .collect(),
                });
            }
            AudioSource::Archive { path } => path,
        };
        if sample.length > 64 * 1024 * 1024 {
            return Err("The compressed sample is too large.".into());
        }
        let mut archive = fs::File::open(archive_path)
            .map_err(|e| format!("Cannot read the original sample archive: {e}"))?;
        archive
            .seek(SeekFrom::Start(sample.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; sample.length as usize];
        archive.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        let decoded = crate::packed::decode_record(&bytes)?;
        if decoded.samples.len() != sample.frame_count as usize {
            return Err("The decoded sample length does not match the catalog.".into());
        }
        Ok(crate::audio::Pcm {
            sample_rate: decoded.sample_rate,
            channels: 1,
            samples: decoded
                .samples
                .into_iter()
                .map(|sample| sample as f32 / 32768.0)
                .collect(),
        })
    }

    pub fn step_synth_sources(&self) -> Result<Vec<(PathBuf, String)>, String> {
        if self.wav_root.is_some() {
            return Ok(self
                .samples
                .iter()
                .filter_map(|sample| {
                    sample
                        .wav_path
                        .as_ref()
                        .map(|path| (path.clone(), sample.name.clone()))
                })
                .collect());
        }
        let packed = self
            .archive_path
            .parent()
            .ok_or("The original archive path is invalid.")?;
        let installation = packed
            .parent()
            .ok_or("The original archive path is invalid.")?;
        let text = decode_1252(
            &fs::read(installation.join("\x65jay").join("\x48ypsound"))
                .map_err(|e| e.to_string())?,
        );
        let mut lines: Vec<_> = text.lines().collect();
        while lines.last() == Some(&"") {
            lines.pop();
        }
        if !lines.len().is_multiple_of(3) {
            return Err("The generator source index has an incomplete record.".into());
        }
        let mut sources = Vec::new();
        for row in lines.as_chunks::<3>().0 {
            let relative = Path::new(row[0]);
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err("The generator source index contains an invalid path.".into());
            }
            let path = packed.join(relative);
            if !path.is_file() {
                return Err(format!("Generator source {} is missing.", path.display()));
            }
            sources.push((path, row[1].trim().to_string()));
        }
        Ok(sources)
    }

    pub fn import_wav(
        &mut self,
        path: &Path,
        name: Option<&str>,
        group: &str,
    ) -> Result<crate::engine::ImportedSample, String> {
        self.import_wav_with_tempo(path, name, group, 140.0)
    }

    /// Load a WAV-only folder without an original installation or a sidecar index.
    /// The caller supplies the tempo: a WAV file does not establish musical BPM.
    /// Keep the returned records in the project when these samples are used.
    pub fn import_wav_folder(
        &mut self,
        root: &Path,
        source_bpm: f64,
    ) -> Result<Vec<crate::engine::ImportedSample>, String> {
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("The WAV folder needs a finite positive source tempo.".into());
        }
        let root =
            fs::canonicalize(root).map_err(|e| format!("Cannot open the WAV folder: {e}"))?;
        if !root.is_dir() {
            return Err("Select a folder that contains WAV samples.".into());
        }
        let mut pending = vec![(root.clone(), 0usize)];
        let mut visited = HashSet::new();
        let mut paths = Vec::new();
        let mut entries_seen = 0usize;
        while let Some((folder, depth)) = pending.pop() {
            if !visited.insert(folder.clone()) {
                continue;
            }
            if depth > 64 {
                return Err("The WAV folder exceeds the 64-level folder limit.".into());
            }
            for entry in fs::read_dir(&folder)
                .map_err(|e| format!("Cannot read {}: {e}", folder.display()))?
            {
                let entry = entry.map_err(|e| e.to_string())?;
                entries_seen += 1;
                if entries_seen > 100_000 {
                    return Err("The WAV folder exceeds the 100,000-item limit.".into());
                }
                let path = fs::canonicalize(entry.path()).map_err(|e| e.to_string())?;
                if !path.starts_with(&root) {
                    return Err("A WAV folder link points outside the selected folder.".into());
                }
                if path.is_dir() {
                    pending.push((path, depth + 1));
                } else if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
                {
                    paths.push(path);
                }
            }
        }
        paths.sort();
        paths.dedup();
        if paths.is_empty() {
            return Err("The selected folder contains no WAV samples.".into());
        }
        let mut candidate = self.clone();
        let mut imported = Vec::with_capacity(paths.len());
        for path in paths {
            let group = path
                .parent()
                .filter(|parent| *parent != root)
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Wave".into());
            imported.push(candidate.import_wav_with_tempo(&path, None, &group, source_bpm)?);
        }
        *self = candidate;
        Ok(imported)
    }

    /// Resolve a moved WAV by its longest matching path suffix. Basename-only
    /// references must identify exactly one registered file; do not guess.
    pub fn resolve_wav_reference(&self, reference: &str) -> Result<PathBuf, String> {
        self.find_wav_reference(reference)?
            .ok_or_else(|| format!("WAV sample {reference} is missing from the selected folder."))
    }

    /// Return no match separately from invalid or ambiguous references so callers
    /// can retain an existing external sample only when this folder lacks it.
    pub(crate) fn find_wav_reference(&self, reference: &str) -> Result<Option<PathBuf>, String> {
        let normalized = reference.replace('\\', "/");
        let parts: Vec<_> = normalized
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect();
        if parts.is_empty() || parts.contains(&"..") {
            return Err("The WAV reference is empty or contains a parent folder.".into());
        }
        let mut best_score = 0;
        let mut matches = HashSet::new();
        for sample in &self.samples {
            let Some(path) = &sample.wav_path else {
                continue;
            };
            let candidate = path.to_string_lossy().replace('\\', "/");
            let score = parts
                .iter()
                .rev()
                .zip(candidate.split('/').rev())
                .take_while(|(left, right)| left.eq_ignore_ascii_case(right))
                .count();
            if score == 0 || score < best_score {
                continue;
            }
            if score > best_score {
                best_score = score;
                matches.clear();
            }
            matches.insert(path.clone());
        }
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.into_iter().next()),
            _ => Err(format!(
                "WAV sample {reference} is ambiguous. Keep its relative folder path."
            )),
        }
    }

    pub fn import_wav_with_tempo(
        &mut self,
        path: &Path,
        name: Option<&str>,
        group: &str,
        source_bpm: f64,
    ) -> Result<crate::engine::ImportedSample, String> {
        let path =
            fs::canonicalize(path).map_err(|e| format!("Cannot open the imported sample: {e}"))?;
        let existing = self
            .samples
            .iter()
            .position(|s| s.id >= 1_000_000 && s.wav_path.as_ref() == Some(&path));
        let id = if let Some(index) = existing {
            self.samples[index].id
        } else {
            self.samples
                .iter()
                .map(|s| s.id)
                .filter(|id| *id >= 1_000_000)
                .max()
                .unwrap_or(999_999)
                .checked_add(1)
                .ok_or("Imported sample ID limit reached.")?
        };
        let default_name = path.file_stem().unwrap_or_default().to_string_lossy();
        let imported = crate::engine::ImportedSample {
            id,
            name: name.unwrap_or(&default_name).trim().to_string(),
            group_name: group.trim().to_string(),
            path,
            source_bpm,
            origin: None,
        };
        let sample = imported_sample(&imported)?;
        self.sources.insert(
            id,
            AudioSource::Wave {
                path: imported.path.clone(),
                origin: imported.origin,
            },
        );
        if let Some(index) = existing {
            self.samples[index] = sample;
        } else {
            self.by_id.insert(id, self.samples.len());
            self.samples.push(sample);
        }
        Ok(imported)
    }

    /// An import whose metadata and file path match its current catalog entry
    /// keeps that validated entry. Undo and redo of a large WAV palette must not
    /// decode every file again.
    fn unchanged_import(&self, imported: &crate::engine::ImportedSample) -> Option<Sample> {
        crate::engine::validate_import_metadata(imported).ok()?;
        let sample = self.sample_by_id(imported.id)?;
        let AudioSource::Wave { path, origin } = self.sources.get(&imported.id)? else {
            return None;
        };
        (sample.wav_path.as_deref() == Some(imported.path.as_path())
            && *path == imported.path
            && sample.name == imported.name
            && sample.group_name == imported.group_name
            && sample.source_bpm == imported.source_bpm
            && *origin == imported.origin)
            .then(|| sample.clone())
    }

    /// Resolve all files before replacing the imported part of the catalog.
    /// A missing or invalid file leaves the current library unchanged.
    pub fn apply_imports(
        &mut self,
        imports: &[crate::engine::ImportedSample],
    ) -> Result<(), String> {
        let mut resolved = Vec::with_capacity(imports.len());
        let mut ids = HashSet::new();
        let mut sources = self.sources.clone();
        sources.retain(|id, _| *id < IMPORTED_ID_START);
        for imported in imports {
            if !ids.insert(imported.id) {
                return Err("The project has duplicate imported sample IDs.".into());
            }
            let sample = match self.unchanged_import(imported) {
                Some(sample) => sample,
                None => imported_sample(imported)?,
            };
            sources.insert(
                imported.id,
                AudioSource::Wave {
                    path: sample
                        .wav_path
                        .clone()
                        .ok_or("The imported sample has no path.")?,
                    origin: imported.origin,
                },
            );
            resolved.push(sample);
        }
        self.samples.retain(|sample| sample.id < 1_000_000);
        self.samples.extend(resolved);
        self.sources = sources;
        self.by_id = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.id, index))
            .collect();
        self.unresolved_samples = self
            .samples
            .iter()
            .filter(|sample| sample.wav_path.is_none())
            .map(|sample| sample.id)
            .collect();
        Ok(())
    }
}

#[cfg(test)]
fn samples_sources(samples: &[Sample]) -> HashMap<u32, AudioSource> {
    samples
        .iter()
        .filter_map(|sample| {
            sample.wav_path.as_ref().map(|path| {
                (
                    sample.id,
                    AudioSource::Wave {
                        path: path.clone(),
                        origin: None,
                    },
                )
            })
        })
        .collect()
}

fn read_t1_wave(path: &Path) -> Result<crate::audio::Pcm, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("The native WAV source exceeds the 128 MiB limit.".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let wave = legacy_format::playback::FileSample::from_wave(PathBuf::from("import.wav"), &bytes)
        .map_err(|e| e.to_string())?;
    let samples = wave
        .samples()
        .iter()
        .map(|sample| *sample as f32 / 32768.0)
        .collect();
    Ok(crate::audio::Pcm {
        sample_rate: 44100,
        channels: 1,
        samples,
    })
}

pub fn read_packed_file(path: &Path) -> Result<crate::audio::Pcm, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("The compressed sample is too large.".into());
    }
    let decoded = crate::packed::decode_record(&fs::read(path).map_err(|e| e.to_string())?)?;
    Ok(crate::audio::Pcm {
        sample_rate: decoded.sample_rate,
        channels: 1,
        samples: decoded
            .samples
            .into_iter()
            .map(|sample| sample as f32 / 32768.0)
            .collect(),
    })
}

fn imported_sample(imported: &crate::engine::ImportedSample) -> Result<Sample, String> {
    crate::engine::validate_import_metadata(imported)?;
    let path = fs::canonicalize(&imported.path)
        .map_err(|e| format!("Cannot open imported sample {}: {e}", imported.name))?;
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("The WAV sample exceeds the 128 MiB limit.".into());
    }
    let pcm = if imported.origin == Some(Product::T1) {
        read_t1_wave(&path)?
    } else {
        crate::audio::read_wav(&path)?
    };
    let frame_count =
        u32::try_from(pcm.frames()).map_err(|_| "The imported sample is too long.")?;
    if frame_count == 0 {
        return Err("The imported sample contains no audio.".into());
    }
    let bars = ((frame_count as f64 * imported.source_bpm / pcm.sample_rate as f64 / 240.0).ceil()
        as u32)
        .max(1);
    let group = match imported.group_name.to_ascii_lowercase().as_str() {
        "loop" => 1,
        "drum" => 2,
        "bass" => 3,
        "sphere" | "layer" => 4,
        "sequence" | "seq" => 5,
        "voice" | "rap" => 6,
        "effect" => 7,
        "xtra" => 8,
        _ => 9,
    };
    Ok(Sample {
        id: imported.id,
        code: format!("IMPORT{}", imported.id),
        name: imported.name.clone(),
        bars,
        group,
        group_name: imported.group_name.clone(),
        offset: 0,
        length: fs::metadata(&path).map_err(|e| e.to_string())?.len(),
        frame_count,
        sample_rate: pcm.sample_rate,
        source_bpm: imported.source_bpm,
        channels: pcm.channels,
        wav_path: Some(path),
    })
}

fn unquote(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(text)
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
        let Ok(reader) = hound::WavReader::open(&path) else {
            continue;
        };
        let spec = reader.spec();
        if spec.sample_rate == 0 || spec.channels == 0 {
            continue;
        }
        let key = (canonical_group(&entry.channel), compact_name(&entry.alias));
        lookup.entry(key).or_default().push((
            path,
            slug(&entry.alias),
            spec.sample_rate,
            spec.channels,
            reader.duration(),
        ));
    }
    let mut used_paths = HashSet::new();
    for (sample, embedded) in samples.iter_mut().zip(embedded_names) {
        let mut candidates: Vec<(&WaveCandidate, u8)> = Vec::new();
        for name in [&sample.name, embedded] {
            let key = (canonical_group(&sample.group_name), compact_name(name));
            if let Some(entries) = lookup.get(&key) {
                for entry in entries {
                    if entry.2 != 44100 || entry.3 != 1 || entry.4 != sample.frame_count {
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

fn canonical_group(group: &str) -> String {
    match group.to_ascii_lowercase().as_str() {
        "sequence" => "seq".into(),
        "sphere" => "layer".into(),
        "voice" => "rap".into(),
        other => other.into(),
    }
}

fn product_folder_hint(name: &str) -> Option<Product> {
    match compact_name(name).as_str() {
        "\x64ance1" | "\x64ance\x65jay1" => Some(Product::D1),
        "\x64ance2" | "\x64ance\x65jay2" => Some(Product::D2),
        "\x64ance3" | "\x64ance\x65jay3" => Some(Product::D3),
        "\x64ance4" | "\x64ance\x65jay4" => Some(Product::D4),
        "\x68ouse\x65jay" => Some(Product::H1),
        "\x72ave\x65jay" | "\x74echno1" | "\x74echno\x65jay1" => Some(Product::T1),
        "\x74echno2" | "\x74echno\x65jay" | "\x74echno\x65jay2" => Some(Product::T2),
        "\x74echno3" | "\x74echno\x65jay3" => Some(Product::T3),
        _ => None,
    }
}

fn slug(name: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

fn compact_name(name: &str) -> String {
    let mut result = String::new();
    let mut digits = String::new();
    fn finish(result: &mut String, digits: &mut String) {
        if !digits.is_empty() {
            let trimmed = digits.trim_start_matches('0');
            result.push_str(if trimmed.is_empty() { "0" } else { trimmed });
            digits.clear();
        }
    }
    for c in name.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            finish(&mut result, &mut digits);
            if c.is_ascii_alphabetic() {
                result.push(c.to_ascii_lowercase());
            }
        }
    }
    finish(&mut result, &mut digits);
    result
}

pub(crate) fn decode_1252(bytes: &[u8]) -> String {
    const C1: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|b| {
            if (0x80..0xa0).contains(b) {
                C1[(b - 0x80) as usize]
            } else {
                *b as char
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn standalone_folder_reads_nested_wavs_and_keeps_failures_atomic() {
        let folder = Folder::new();
        folder.wave("Bass/my original.WAV", 22_050, 220);
        folder.wave("another.wav", 48_000, 480);
        let mut library = Library::open_wav_folder(&folder.0, 160.0).unwrap();
        assert!(library.archive_path.as_os_str().is_empty());
        assert!(library.bindings().is_empty());
        assert_eq!(
            library.wav_root(),
            Some(fs::canonicalize(&folder.0).unwrap().as_path())
        );
        assert_eq!(library.samples.len(), 2);
        let bass = library
            .samples
            .iter()
            .find(|s| s.name == "my original")
            .unwrap();
        assert_eq!(bass.group_name, "Bass");
        assert_eq!(bass.source_bpm, 160.0);
        assert_eq!(library.read_pcm(bass.id).unwrap().frames(), 220);
        assert_eq!(library.imported_samples().len(), 2);
        let before = library.imported_samples();
        fs::write(folder.0.join("invalid.wav"), b"not audio").unwrap();
        assert!(library.import_wav_folder(&folder.0, 140.0).is_err());
        assert_eq!(library.imported_samples(), before);
        assert!(library.import_wav_folder(&folder.0, f64::NAN).is_err());
    }

    #[test]
    fn moved_wave_resolution_preserves_relative_paths_and_rejects_ambiguity() {
        let folder = Folder::new();
        let bass = folder.wave("Bass/sound.wav", 44_100, 10);
        folder.wave("Drum/sound.wav", 44_100, 10);
        let only = folder.wave("only.wav", 44_100, 10);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        assert_eq!(
            library
                .resolve_wav_reference(r"C:\old\Bass\SOUND.WAV")
                .unwrap(),
            bass
        );
        assert_eq!(
            library.resolve_wav_reference("old/place/only.wav").unwrap(),
            only
        );
        assert!(
            library
                .resolve_wav_reference("sound.wav")
                .unwrap_err()
                .contains("ambiguous")
        );
        assert!(library.resolve_wav_reference("../only.wav").is_err());
        assert!(library.resolve_wav_reference("missing.wav").is_err());
    }

    #[test]
    fn palette_restore_keeps_project_ids_and_does_not_decode_unused_files_again() {
        let folder = Folder::new();
        let first = folder.wave("first.wav", 44_100, 10);
        let second = folder.wave("second.wav", 44_100, 10);
        let mut palette = Library::empty();
        palette.import_wav_folder(&folder.0, 160.0).unwrap();
        let mut project = Library::empty();
        let used = project
            .import_wav(&second, Some("Project name"), "Wave")
            .unwrap();
        fs::remove_file(first).unwrap();
        let added = project.restore_wav_palette(&palette).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].id, used.id + 1);
        assert_eq!(added[0].source_bpm, 160.0);
        assert_eq!(project.sample_by_id(used.id).unwrap().name, "Project name");
        assert!(project.restore_wav_palette(&palette).unwrap().is_empty());
        assert!(project.read_pcm(added[0].id).is_err());
    }

    #[test]
    fn unchanged_imports_keep_their_catalog_entries_without_decoding_again() {
        let folder = Folder::new();
        let path = folder.wave("loop.wav", 44_100, 10);
        let mut library = Library::empty();
        let imported = library.import_wav(&path, Some("Loop"), "Wave").unwrap();
        // Undo and redo re-apply identical metadata: the file is not read again.
        fs::remove_file(&path).unwrap();
        library
            .apply_imports(std::slice::from_ref(&imported))
            .unwrap();
        assert_eq!(library.sample_by_id(imported.id).unwrap().frame_count, 10);
        // Changed metadata needs the file and fails atomically without it.
        let mut retimed = imported.clone();
        retimed.source_bpm = 120.0;
        assert!(library.apply_imports(&[retimed]).is_err());
        let mut renamed = imported;
        renamed.name = "Other".into();
        assert!(library.apply_imports(&[renamed]).is_err());
        assert_eq!(library.sample_by_id(1_000_000).unwrap().name, "Loop");
    }

    #[test]
    fn wave_product_uses_catalog_bars_and_tempo_without_original_files() {
        let folder = Folder::new();
        folder.wave("Bass/soft-dust-06.wav", 22_050, 100);
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 160.0).unwrap();
        let metadata = crate::catalog::WaveMetadata {
            native_id: 731,
            code: "native-code".into(),
            name: "soft dust 6".into(),
            group_name: "bass".into(),
            bars: 4,
            source_bpm: 180.0,
            frame_count: 200,
        };
        assert_eq!(
            library
                .register_wav_product(Product::T1, &folder.0, std::slice::from_ref(&metadata))
                .unwrap(),
            1
        );
        let id = Product::T1.global_sample_id(731).unwrap();
        let sample = library.sample_by_id(id).unwrap();
        assert_eq!(sample.bars, 4);
        assert_eq!(sample.source_bpm, 180.0);
        assert!(library.is_wav_product(Product::T1));
        assert_eq!(library.read_pcm(id).unwrap().frames(), 100);
        library.apply_imports(&[]).unwrap();
        assert_eq!(library.read_pcm(id).unwrap().frames(), 100);

        let duplicate = folder.wave("extra/Bass/soft-dust-6.wav", 22_050, 100);
        library.import_wav(&duplicate, None, "bass").unwrap();
        let count = library.samples.len();
        assert_eq!(
            library
                .register_wav_product(Product::D1, &folder.0, &[metadata])
                .unwrap(),
            0
        );
        assert_eq!(library.samples.len(), count);
        assert!(library.source_root(Product::D1).is_none());
    }

    #[test]
    fn colliding_display_labels_do_not_guess_native_ids_and_codes_override_category() {
        let folder = Folder::new();
        folder.wave("Bass/hihat-hard.wav", 44_100, 100);
        folder.wave("unrelated/T1DC501.wav", 44_100, 100);
        let mut metadata = crate::catalog::WaveMetadata {
            native_id: 13776,
            code: "T1DC501".into(),
            name: "hihat hard".into(),
            group_name: "Bass".into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 100,
        };
        let mut second = metadata.clone();
        second.native_id += 1;
        second.code = "T1DC502".into();
        let mut library = Library::empty();
        library.import_wav_folder(&folder.0, 140.0).unwrap();
        assert_eq!(
            library
                .register_wav_product(Product::T2, &folder.0, &[metadata.clone(), second.clone()])
                .unwrap(),
            2
        );
        assert!(
            library
                .sample_by_id(13776)
                .unwrap()
                .wav_path
                .as_ref()
                .unwrap()
                .ends_with("T1DC501.wav")
        );
        // With no code-named source, neither colliding display label is identified.
        let ambiguous_folder = Folder::new();
        ambiguous_folder.wave("Bass/hihat-hard.wav", 44_100, 100);
        let mut ambiguous = Library::empty();
        ambiguous
            .import_wav_folder(&ambiguous_folder.0, 140.0)
            .unwrap();
        assert_eq!(
            ambiguous
                .register_wav_product(
                    Product::T2,
                    &ambiguous_folder.0,
                    &[metadata.clone(), second]
                )
                .unwrap(),
            0
        );
        assert_eq!(ambiguous.imported_samples().len(), 1);
        metadata.native_id = 1;
        assert_eq!(
            ambiguous
                .register_wav_product(Product::D1, &ambiguous_folder.0, &[metadata])
                .unwrap(),
            1
        );
    }
    #[test]
    fn names_preserve_exact_slug_and_normalize_numeric_padding() {
        assert_eq!(compact_name("soft dust 06"), compact_name("soft-dust-6"));
        assert_ne!(slug("tech noR"), slug("alpha R"));
        assert_eq!(decode_1252(&[0x80, 0xe9]), "€é");
    }

    #[test]
    fn imported_wav_roundtrips_and_missing_file_is_atomic() {
        let folder =
            std::env::temp_dir().join(format!("mixjam-techno-import-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("sample.wav");
        crate::audio::Pcm {
            sample_rate: 22050,
            channels: 2,
            samples: vec![0.25; 440],
        }
        .write_wav(&path)
        .unwrap();
        let mut library = Library::empty();
        let imported = library
            .import_wav(&path, Some("My sample"), "Wave")
            .unwrap();
        assert_eq!(imported.id, 1_000_000);
        let sample = library.sample_by_id(imported.id).unwrap();
        assert_eq!(sample.frame_count, 220);
        assert_eq!(sample.sample_rate, 22050);
        assert_eq!(sample.channels, 2);
        assert_eq!(library.read_pcm(imported.id).unwrap().frames(), 220);
        assert_eq!(
            library.import_wav(&path, None, "Wave").unwrap().id,
            imported.id
        );
        let mut project = crate::engine::Project::default();
        project.imported_samples.push(imported.clone());
        let saved = serde_json::to_vec(&project).unwrap();
        let restored: crate::engine::Project = serde_json::from_slice(&saved).unwrap();
        library.apply_imports(&restored.imported_samples).unwrap();
        let mut missing = imported;
        missing.path = folder.join("missing.wav");
        assert!(library.apply_imports(&[missing]).is_err());
        assert_eq!(library.samples.len(), 1);
        assert_eq!(library.samples[0].name, "My sample");
        fs::remove_file(path).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn native_packed_reads_unicode_host_paths_without_native_ansi_limits() {
        let folder =
            std::env::temp_dir().join(format!("mixjam-techno-packed-host-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("音源.\x70xd");
        let mut bytes = b"t\x50xD\0T".to_vec();
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(37u16.to_le_bytes());
        bytes.extend([255, 128, 255, 128]);
        fs::write(&path, bytes).unwrap();
        let mut library = Library::empty();
        library.samples.push(Sample {
            id: 100_731,
            code: "source.\x70xd".into(),
            name: "Native source".into(),
            bars: 1,
            group: 9,
            group_name: "Loop".into(),
            offset: 0,
            length: 16,
            frame_count: 2,
            sample_rate: 44100,
            source_bpm: 180.0,
            channels: 1,
            wav_path: None,
        });
        library.by_id.insert(100_731, 0);
        library
            .sources
            .insert(100_731, AudioSource::T1Packed { path: path.clone() });
        let pcm = library.read_pcm(100_731).unwrap();
        assert_eq!(pcm.samples, vec![74.0 / 32768.0; 3]);
        fs::remove_file(path).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    #[ignore = "Requires all eight local native sample libraries; decodes the complete corpus."]
    fn every_registered_native_sample_decodes_with_its_native_length_rules() {
        let mut total = 0;
        let mut failures = Vec::new();
        for (product, expected) in [
            (Product::D1, 2845),
            (Product::D2, 3795),
            (Product::D3, 5049),
            (Product::D4, 5330),
            (Product::H1, 3563),
            (Product::T1, 3017),
            (Product::T2, 4015),
            (Product::T3, 4006),
        ] {
            let root = crate::project_io::default_root(product).expect("Native library fixture");
            let mut library = Library::empty();
            library
                .register(product, &root, &crate::project_io::reference_samples_root())
                .unwrap();
            assert_eq!(library.samples.len(), expected, "{}", product.label());
            let started = std::time::Instant::now();
            let mut frames = 0u64;
            let mut decoded = 0;
            for sample in &library.samples {
                let context = format!("{} / {} / {}", product.label(), sample.id, sample.name);
                let pcm = match library.read_pcm(sample.id) {
                    Ok(pcm) => pcm,
                    Err(error) => {
                        failures.push(format!("{context}: {error}"));
                        continue;
                    }
                };
                if let Err(error) = pcm.validate() {
                    failures.push(format!("{context}: {error}"));
                }
                let declared = u64::from(sample.frame_count);
                let length_matches = if let AudioSource::T1Packed { path } =
                    &library.sources[&sample.id]
                {
                    // Complete final runs and the metadata loader's pending
                    // initial sample differ from isolated declared-length PCM.
                    let mut prefix = [0; 512];
                    let read = fs::File::open(path).unwrap().read(&mut prefix).unwrap();
                    let header = legacy_format::packed::parse_header(&prefix[..read]).unwrap();
                    let initial =
                        u64::from(header.kind == legacy_format::packed::HeaderKind::Metadata);
                    if product == Product::D1 && sample.id == 205_043 {
                        // Authenticated factory file d4sp434.packed has 109,215
                        // physical codes and a nominal 151,200-frame header.
                        use sha2::Digest;
                        let hash = format!("{:x}", sha2::Sha256::digest(fs::read(path).unwrap()));
                        if declared != 151_200
                            || hash
                                != "07c40aaf4668c213c3b9f340c1db8d266b7fa55887897d0ba0be37186e5c649f"
                        {
                            failures.push(format!(
                                "{context}: the authenticated short-file fixture changed"
                            ));
                        }
                        pcm.frames() == 109_216
                    } else {
                        (declared.saturating_sub(4) + initial..=declared + 4 + initial)
                            .contains(&pcm.frames())
                    }
                } else {
                    pcm.frames() == declared
                };
                if !length_matches {
                    failures.push(format!(
                        "{context}: {} frames for declared {declared}",
                        pcm.frames()
                    ));
                }
                if pcm.sample_rate != sample.sample_rate || pcm.channels != sample.channels {
                    failures.push(format!(
                        "{context}: decoded format differs from the catalog"
                    ));
                }
                frames += pcm.frames();
                decoded += 1;
            }
            eprintln!(
                "Native decode: {}: {decoded}/{expected} samples, {frames} frames, {:.3} seconds",
                product.slug(),
                started.elapsed().as_secs_f64()
            );
            total += library.samples.len();
        }
        assert_eq!(total, 31_620);
        assert!(
            failures.is_empty(),
            "{} native decode failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    #[ignore = "Requires the local WAV sample corpus; validates every WAV without original providers."]
    fn standalone_local_wave_corpus_has_native_aliases_without_original_providers() {
        let root = std::env::var_os("MIXJAM_TECHNO_WAV_CORPUS")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples"));
        let started = std::time::Instant::now();
        let library = Library::open_wav_folder(&root, 140.0).unwrap();
        assert!(library.archive_path.as_os_str().is_empty());
        let native = library
            .samples
            .iter()
            .filter(|s| Product::sample_key(s.id).is_some_and(|(p, _)| p == Product::T2))
            .count();
        eprintln!(
            "Standalone WAV folder: {} palette entries, {native} T2 identities, {} generic WAVs",
            library.samples.len(),
            library.imported_samples().len()
        );
        eprintln!(
            "WAV scan elapsed: {:.3} seconds",
            started.elapsed().as_secs_f64()
        );
        assert!(native > 0);
        assert!(
            library
                .samples
                .iter()
                .all(|sample| sample.wav_path.is_some())
        );
    }

    #[test]
    #[ignore = "Requires the installed original and the supplied WAV corpus."]
    fn installed_catalog_has_complete_nonoverlapping_ranges() {
        let root = crate::project_io::default_root(Product::T2).expect("T2 fixtures");
        let library =
            Library::open_with_samples(&root, &crate::project_io::reference_samples_root())
                .unwrap();
        assert_eq!(library.samples.len(), 4015);
        assert_eq!(
            library.samples.last().unwrap().offset + library.samples.last().unwrap().length,
            429261617
        );
        assert!(
            library
                .samples
                .iter()
                .filter(|s| s.wav_path.is_some())
                .count()
                > 3800
        );
        assert_eq!(library.sample_by_id(14546).unwrap().frame_count, 75600);
        assert_eq!(library.read_pcm(14546).unwrap().frames(), 75600);
        assert_eq!(library.step_synth_sources().unwrap().len(), 504);
    }
}
