use super::*;

struct Scratch {
    root: PathBuf,
    created_assets: Vec<PathBuf>,
}

impl Scratch {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "mixjam-d2-import-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ASSET_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root,
            created_assets: Vec::new(),
        }
    }

    fn track_asset(&mut self, bytes: &[u8]) -> PathBuf {
        let path = crate::project_io::data_root()
            .join("imported-audio")
            .join(format!("{:x}.wav", Sha256::digest(bytes)));
        if !path.exists() {
            self.created_assets.push(path.clone());
        }
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        for path in &self.created_assets {
            let _ = fs::remove_file(path);
        }
        // This directory was created exclusively by this test.
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn block(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value);
}

fn wrapped(bytes: &mut Vec<u8>, value: &str) {
    block(bytes, &[value.as_bytes(), &[0, 1]].concat());
}

fn source_event(path: &str, position: i32, bars: u32, right_trim: u32) -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend_from_slice(&position.to_le_bytes());
    bytes.extend_from_slice(&10193u16.to_le_bytes());
    if position >= 0 {
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&right_trim.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
    }
    wrapped(&mut bytes, path);
    wrapped(&mut bytes, "");
    wrapped(&mut bytes, "Source label");
    bytes.extend_from_slice(&12u16.to_le_bytes());
    bytes.extend_from_slice(&bars.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes
}

fn song(events: &[Vec<u8>], footer: Option<&[(&str, &[u8])]>) -> (Vec<u8>, usize) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xa19u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&140u16.to_le_bytes());
    bytes.extend_from_slice(&140u32.to_le_bytes());
    block(&mut bytes, b"Composer\0#SKKENNUNG#:0000000\0\x01");
    block(&mut bytes, b"D2 source regression\0Mixer text");
    for _ in 0..16 {
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&16384u16.to_le_bytes());
    }
    let timeline_start = bytes.len();
    let mut chunk = vec![1];
    chunk.extend_from_slice(&(events.len() as u32).to_le_bytes());
    for event in events {
        chunk.extend_from_slice(event);
    }
    chunk.push(1);
    bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
    bytes.extend_from_slice(&chunk);
    bytes.extend_from_slice(&SENTINEL);
    if let Some(assets) = footer {
        bytes.extend_from_slice(&(assets.len() as u32).to_le_bytes());
        for (path, payload) in assets {
            wrapped(&mut bytes, path);
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
        }
    }
    (bytes, timeline_start)
}

fn packed(predictor: u16) -> Vec<u8> {
    let mut bytes = b"tPxD\0T".to_vec();
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(&predictor.to_le_bytes());
    bytes.push(0); // The initial dictionary entry emits five unchanged values.
    bytes
}

fn wave(value: f32) -> Vec<u8> {
    encode_float_wave(&crate::audio::Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: vec![value, -value, value, 0.0],
    })
    .unwrap()
}

#[test]
fn counted_external_wave_conflicts_keep_distinct_references_and_repeated_ids() {
    let mut scratch = Scratch::new();
    let wav = wave(0.2873);
    scratch.track_asset(&wav);
    for folder in ["A", "B"] {
        fs::create_dir(scratch.root.join(folder)).unwrap();
        fs::write(scratch.root.join(folder).join("take.wav"), &wav).unwrap();
    }
    let mut library = Library::empty();
    library.import_wav_folder(&scratch.root, 140.0).unwrap();
    let old_reference = scratch
        .root
        .join("old/take.wav")
        .to_string_lossy()
        .into_owned();
    let other_reference = scratch
        .root
        .join("other/take.wav")
        .to_string_lossy()
        .into_owned();
    let (bytes, _) = song(
        &[
            source_event(&old_reference, 0, 1, 0),
            source_event(&old_reference, 128, 1, 0),
            source_event(&other_reference, 256, 1, 0),
        ],
        None,
    );
    let path = scratch.root.join("song.mix");
    fs::write(&path, bytes).unwrap();
    let project = load(&path, Product::D2, &library).unwrap();
    assert!(project.missing_samples.is_empty());
    assert_eq!(project.sample_conflicts.len(), 2);
    assert_eq!(project.clips.len(), 3);
    assert_eq!(project.clips[0].sample_id, project.clips[1].sample_id);
    assert_ne!(project.clips[0].sample_id, project.clips[2].sample_id);
    for conflict in &project.sample_conflicts {
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.selected, 0);
        assert!(!conflict.confirmed);
        assert_eq!(conflict.sample_ids.len(), 1);
        assert!(conflict.source_spans.is_empty());
    }
    let (embedded, _) = song(
        &[source_event(&old_reference, 0, 1, 0)],
        Some(&[(&old_reference, &wav)]),
    );
    fs::write(&path, embedded).unwrap();
    let project = load(&path, Product::D2, &library).unwrap();
    assert!(project.sample_conflicts.is_empty());
}

#[test]
fn d2_embedded_packed_and_wave_import_reopen_and_render_without_original_paths() {
    let mut scratch = Scratch::new();
    let packed_path = scratch
        .root
        .join("missing.pxd")
        .to_string_lossy()
        .into_owned();
    let wave_path = scratch
        .root
        .join("missing.wav")
        .to_string_lossy()
        .into_owned();
    let pxd = packed(937);
    let wav = wave(0.1379);
    let decoded = packed_wave(&packed_path, &pxd).unwrap();
    assert_eq!(decoded.frames, 6);
    let packed_destination = scratch.track_asset(&decoded.bytes);
    let wave_destination = scratch.track_asset(&wav);
    let (bytes, _) = song(
        &[
            source_event(&packed_path, 0, 1, 96),
            source_event(&packed_path.to_uppercase(), 256, 2, 224),
            source_event(&wave_path, -129, 1, 0),
        ],
        Some(&[(&packed_path, &pxd), (&wave_path, &wav)]),
    );
    let input = scratch.root.join("embedded.mix");
    fs::write(&input, bytes).unwrap();
    let project = load(&input, Product::D2, &Library::empty()).unwrap();
    assert!(!Path::new(&packed_path).exists());
    assert!(!Path::new(&wave_path).exists());
    assert_eq!(project.imported_samples.len(), 2);
    assert_eq!(project.clips.len(), 3);
    assert_eq!(project.clips[0].sample_id, project.clips[1].sample_id);
    assert_ne!(project.clips[0].sample_id, project.clips[2].sample_id);
    assert_eq!(
        project
            .clips
            .iter()
            .map(|c| c.length_ticks)
            .collect::<Vec<_>>(),
        [64, 64, 256]
    );
    assert_eq!(
        project
            .clips
            .iter()
            .map(|c| c.start_tick)
            .collect::<Vec<_>>(),
        [0, 512, 256]
    );
    assert_eq!(
        fs::read(&packed_destination).unwrap(),
        decoded.bytes.as_ref()
    );
    assert_eq!(fs::read(&wave_destination).unwrap(), wav);
    assert!(
        project
            .imported_samples
            .iter()
            .all(|s| s.origin == Some(Product::D2))
    );
    let saved = scratch.root.join("saved.json");
    project.save(&saved).unwrap();
    assert_eq!(Project::load(&saved).unwrap(), project);
    let (mut reopened, library) = crate::project_io::open(&saved, &Library::empty()).unwrap();
    assert_eq!(reopened, project);
    reopened.clips.truncate(1);
    let audio = crate::audio::render_project(&reopened, &library).unwrap();
    assert_eq!(audio.frames(), 18900);
    assert!(audio.samples[..12].iter().any(|s| s.abs() > 0.001));
    assert!(audio.samples[12..].iter().all(|s| *s == 0.0));
}

#[test]
fn d2_footer_accepts_absent_zero_and_exact_counted_payloads() {
    let event = source_event("Z:\\missing\\source.pxd", -1, 1, 0);
    for footer in [None, Some(&[][..])] {
        let (bytes, start) = song(std::slice::from_ref(&event), footer);
        assert!(d2_embedded_sources(&bytes, start).unwrap().is_empty());
    }
    let payload = packed(18);
    let (bytes, start) = song(&[event], Some(&[("Z:\\missing\\source.pxd", &payload)]));
    let sources = d2_embedded_sources(&bytes, start).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].bytes, payload);
    assert_eq!(sources[0].path, "Z:\\missing\\source.pxd");
}

#[test]
fn d2_footer_rejects_truncation_overcount_and_trailing_bytes() {
    let payload = packed(19);
    let (bytes, start) = song(
        &[source_event("Z:\\missing\\source.pxd", -1, 1, 0)],
        Some(&[("Z:\\missing\\source.pxd", &payload)]),
    );
    let footer_start =
        start + u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) as usize + 4;
    for end in footer_start + 1..bytes.len() {
        assert!(
            d2_embedded_sources(&bytes[..end], start).is_err(),
            "accepted truncation at {end}"
        );
    }
    let mut overcount = bytes.clone();
    overcount[footer_start..footer_start + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(d2_embedded_sources(&overcount, start).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(d2_embedded_sources(&trailing, start).is_err());
}

#[test]
fn d2_conflicting_embedded_paths_fail_before_publishing_earlier_audio() {
    let mut scratch = Scratch::new();
    let good = wave(0.218731);
    let candidate = scratch.track_asset(&good);
    let previous = fs::read(&candidate).ok();
    let pxd = packed(123);
    let other = packed(124);
    let (bytes, _) = song(
        &[
            source_event("Z:\\unavailable\\good.wav", -1, 1, 0),
            source_event("Z:\\unavailable\\bad.pxd", -129, 1, 0),
        ],
        Some(&[
            ("Z:\\unavailable\\good.wav", &good),
            ("Z:\\unavailable\\bad.pxd", &pxd),
            ("z:/UNAVAILABLE/BAD.pxd", &other),
        ]),
    );
    let input = scratch.root.join("conflict.mix");
    fs::write(&input, bytes).unwrap();
    let error = load(&input, Product::D2, &Library::empty()).unwrap_err();
    assert!(error.contains("different embedded"), "{error}");
    assert_eq!(fs::read(&candidate).ok(), previous);
}

#[test]
fn d2_malformed_packed_audio_is_rejected_before_publication() {
    let mut scratch = Scratch::new();
    let good = wave(0.271896);
    let candidate = scratch.track_asset(&good);
    let previous = fs::read(&candidate).ok();
    for malformed in [b"wrong header".to_vec(), packed(22)[..11].to_vec(), {
        let mut p = packed(22);
        *p.last_mut().unwrap() = 0xf9;
        p
    }] {
        let (bytes, _) = song(
            &[
                source_event("Z:\\unavailable\\good.wav", -1, 1, 0),
                source_event("Z:\\unavailable\\bad.pxd", -129, 1, 0),
            ],
            Some(&[
                ("Z:\\unavailable\\good.wav", &good),
                ("Z:\\unavailable\\bad.pxd", &malformed),
            ]),
        );
        let header = read_header(&bytes, Product::D2).unwrap();
        let timeline = read_timeline(&bytes, header.after_mixer, Product::D2).unwrap();
        assert!(
            build_project(
                Path::new("malformed.mix"),
                &bytes,
                Product::D2,
                &Library::empty(),
                header,
                timeline
            )
            .is_err()
        );
        assert_eq!(fs::read(&candidate).ok(), previous);
    }
}

#[test]
fn d2_external_packed_file_decodes_with_its_saved_identity() {
    let scratch = Scratch::new();
    let path = scratch.root.join("original.pxd");
    fs::write(&path, packed(387)).unwrap();
    let text = path.to_string_lossy();
    let source = d2_source_wave(&text, &[], &Library::empty()).unwrap();
    assert_eq!(source.frames, 6);
    assert_eq!(source.sample_rate, 44100);
    assert_eq!(source.source_path, text);
    let mut reader = hound::WavReader::new(Cursor::new(source.bytes)).unwrap();
    let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
    assert_eq!(samples, vec![774; 6]);
    assert!(d2_source_wave("Z:\\unavailable\\wrong\\avpo.pxd", &[], &Library::empty()).is_err());
}

#[test]
fn d2_external_pxd_catalog_resolution_uses_d1_full_code_identity() {
    let scratch = Scratch::new();
    let path = scratch.root.join("verified-source.wav");
    fs::write(&path, wave(0.3125)).unwrap();
    let id = Product::D1.global_sample_id(788).unwrap();
    let sample = crate::library::Sample {
        id,
        code: "bo\\avpo.pxd".into(),
        name: "Grp.6 Vers11".into(),
        bars: 2,
        group_name: "loop".into(),
        offset: 0,
        length: 0,
        frame_count: 4,
        sample_rate: 44100,
        source_bpm: 140.0,
        channels: 1,
        wav_path: Some(path.clone()),
        digest: None,
    };
    let library = Library::fixture(vec![sample.clone()]);
    let source = d2_source_wave("Z:\\missing\\Dance\\BO\\AVPO.PXD", &[], &library).unwrap();
    assert_eq!(source.frames, 4);
    assert_eq!(source.bytes.as_ref(), fs::read(path).unwrap());
    assert!(d2_source_wave("Z:\\missing\\wrong\\AVPO.PXD", &[], &library).is_err());
    let wrong_product = Library::fixture(vec![crate::library::Sample {
        id: Product::D2.global_sample_id(788).unwrap(),
        ..sample
    }]);
    assert!(d2_source_wave("Z:\\missing\\BO\\AVPO.PXD", &[], &wrong_product).is_err());
}
#[test]
fn d2_timeline_marker_inside_embedded_pcm_does_not_hide_the_real_timeline() {
    let payload = wave(f32::from_bits(0x018a7aa9));
    assert!(payload.windows(4).any(|window| window == SENTINEL));
    let (bytes, start) = song(
        &[source_event("Z:\\missing\\marker.wav", -1, 1, 0)],
        Some(&[("Z:\\missing\\marker.wav", &payload)]),
    );
    let header = read_header(&bytes, Product::D2).unwrap();
    let timeline = read_timeline(&bytes, header.after_mixer, Product::D2).unwrap();
    assert_eq!(timeline.start, start);
    assert_eq!(timeline.events.len(), 1);
    assert_eq!(timeline.events[0].position, 0);
    let sources = d2_embedded_sources(&bytes, timeline.start).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].bytes, payload);
}

#[test]
fn inherited_ids_keep_the_explicit_d2_namespace_and_loader_boundaries() {
    let sample = |product: Product, native_id| crate::library::Sample {
        id: product.global_sample_id(native_id).unwrap(),
        code: "fixture".into(),
        name: "Inherited source".into(),
        bars: 2,
        group_name: "Test".into(),
        offset: 0,
        length: 0,
        frame_count: 151200,
        sample_rate: 44100,
        source_bpm: 140.0,
        channels: 1,
        wav_path: None,
        digest: None,
    };
    let import = |product, native_id, library: &Library| {
        build_project(
            Path::new("inherited.mix"),
            &[],
            product,
            library,
            Header {
                title: "Inherited".into(),
                author: String::new(),
                bpm: 140.0,
                effective_bpm: 140.0,
                tracks: vec![Track::default(); TRACK_COUNT],
                source_notices: Vec::new(),
                settings: Default::default(),
                after_mixer: 0,
            },
            Timeline {
                start: 0,
                events: vec![Event {
                    track: 0,
                    position: 0,
                    left_trim: 0,
                    right_trim: 0,
                    gain: 1.0,
                    reference: Reference::Native(native_id),
                }],
            },
        )
    };
    for (product, native_id) in [(Product::T2, 1), (Product::T2, 1957), (Product::D3, 6577)] {
        let library = Library::fixture(vec![sample(Product::D2, native_id)]);
        let project = import(product, native_id as u16, &library).unwrap();
        assert_eq!(
            project.clips[0].sample_id,
            Product::D2.global_sample_id(native_id).unwrap()
        );
        assert_eq!(project.clips[0].length_ticks, 512);
    }
    // T2 loads the base D2 catalog, not the later D2 expansion archives.
    let kits = Library::fixture(vec![sample(Product::D2, 6577)]);
    let unresolved = import(Product::T2, 6577, &kits).unwrap();
    assert_eq!(unresolved.missing_samples.len(), 1);
    assert_eq!(
        unresolved.clips[0].sample_id,
        Product::T2.global_sample_id(6577).unwrap()
    );
    // An equal integer in T1 identifies another sound; never use it as a fallback.
    let wrong = Library::fixture(vec![sample(Product::T1, 1864)]);
    let unresolved = import(Product::T2, 1864, &wrong).unwrap();
    assert_eq!(unresolved.missing_samples.len(), 1);
    assert_eq!(
        unresolved.clips[0].sample_id,
        Product::D2.global_sample_id(1864).unwrap()
    );
    let current = Library::fixture(vec![sample(Product::T2, 513), sample(Product::D2, 513)]);
    assert_eq!(
        import(Product::T2, 513, &current).unwrap().clips[0].sample_id,
        Product::T2.global_sample_id(513).unwrap()
    );
}

#[test]
fn d2_complete_timeline_in_embedded_pcm_is_audio_not_an_ambiguous_arrangement() {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        let fake = [10, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0xa9, 0x7a, 0x8a, 1];
        for value in fake.as_chunks::<2>().0 {
            writer.write_sample(i16::from_le_bytes(*value)).unwrap();
        }
        writer.finalize().unwrap();
    }
    let payload = cursor.into_inner();
    let (bytes, start) = song(
        &[source_event("Z:\\missing\\collision.wav", -1, 1, 0)],
        Some(&[("Z:\\missing\\collision.wav", &payload)]),
    );
    let header = read_header(&bytes, Product::D2).unwrap();
    let timeline = read_timeline(&bytes, header.after_mixer, Product::D2).unwrap();
    assert_eq!(timeline.start, start);
    assert_eq!(timeline.events.len(), 1);
}

#[test]
fn missing_repeated_source_infers_enough_span_for_every_trim() {
    let source = || {
        Reference::Wave(WaveReference {
            path: r"Z:\unavailable\long-source.wav".into(),
            declared_ticks: None,
            metadata: BTreeMap::from([("Native label 1".into(), "Lost long source".into())]),
        })
    };
    let project = build_project(
        Path::new("unknown-length.mix"),
        &[],
        Product::D3,
        &Library::empty(),
        Header {
            title: "Unknown length".into(),
            author: String::new(),
            bpm: 140.0,
            effective_bpm: 140.0,
            tracks: vec![Track::default(); TRACK_COUNT],
            source_notices: Vec::new(),
            settings: Default::default(),
            after_mixer: 0,
        },
        Timeline {
            start: 0,
            events: vec![
                Event {
                    track: 0,
                    position: 0,
                    left_trim: 0,
                    right_trim: 0,
                    gain: 1.0,
                    reference: source(),
                },
                Event {
                    track: 0,
                    position: 256,
                    left_trim: 512,
                    right_trim: 0,
                    gain: 1.0,
                    reference: source(),
                },
            ],
        },
    )
    .unwrap();
    assert_eq!(project.missing_samples.len(), 1);
    assert!(project.missing_samples[0].length_inferred);
    assert_eq!(project.missing_samples[0].nominal_ticks, 768);
    assert_eq!(project.missing_samples[0].name, "Lost long source");
    assert_eq!(project.clips[0].length_ticks, 768);
    assert_eq!(project.clips[1].length_ticks, 256);
    assert_eq!(project.clips[0].sample_id, project.clips[1].sample_id);
    let rendered = crate::audio::render_project(&project, &Library::empty()).unwrap();
    assert!(rendered.samples.iter().all(|value| *value == 0.0));
}

#[test]
fn missing_framed_wave_keeps_exact_span_and_all_saved_metadata() {
    let scratch = Scratch::new();
    let (bytes, _) = song(
        &[
            source_event("Z:\\unavailable\\named.wav", 0, 3, 96),
            source_event("Z:\\unavailable\\named.wav", -513, 3, 0),
        ],
        None,
    );
    let input = scratch.root.join("missing.mix");
    fs::write(&input, bytes).unwrap();
    let (project, library) = crate::project_io::open(&input, &Library::empty()).unwrap();
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.clips[0].length_ticks, 576);
    assert_eq!(project.clips[1].length_ticks, 768);
    let missing = &project.missing_samples[0];
    assert_eq!(project.missing_samples.len(), 1);
    assert!(!missing.length_inferred);
    assert_eq!(missing.nominal_ticks, 768);
    assert_eq!(missing.metadata["Native label 2"], "Source label");
    assert_eq!(missing.metadata["Saved sample ID"], "10193");
    assert_eq!(missing.metadata["Declared bars"], "3");
    let saved = scratch.root.join("saved.mixjam");
    crate::project_io::save(&project, &saved).unwrap();
    let (reopened, _) = crate::project_io::open(&saved, &library).unwrap();
    assert_eq!(reopened.missing_samples, project.missing_samples);
    assert_eq!(reopened.clips, project.clips);
}

#[test]
fn unavailable_wave_with_conflicting_saved_span_opens_with_explicit_inference() {
    // inproved fiesta saves two bars, but one missing-WAV placement's native
    // trims are 16 + 472 units. Preserve the fields and infer its minimum span.
    let source = || {
        Reference::Wave(WaveReference {
            path: r"Z:\unavailable\1refren4.wav".into(),
            declared_ticks: Some(512),
            metadata: BTreeMap::from([("Declared bars".into(), "2".into())]),
        })
    };
    let project = build_project(
        Path::new("conflicting-span.mix"),
        &[],
        Product::D3,
        &Library::empty(),
        Header {
            title: "Conflicting span".into(),
            author: String::new(),
            bpm: 140.0,
            effective_bpm: 140.0,
            tracks: vec![Track::default(); TRACK_COUNT],
            source_notices: Vec::new(),
            settings: Default::default(),
            after_mixer: 0,
        },
        Timeline {
            start: 0,
            events: std::iter::once(Event {
                track: 0,
                position: 0,
                left_trim: 16,
                right_trim: 0,
                gain: 1.0,
                reference: source(),
            })
            .chain((0..1000).map(|index| Event {
                track: 0,
                position: 48 + index * 256,
                left_trim: 32,
                right_trim: 944,
                gain: 1.0,
                reference: source(),
            }))
            .collect(),
        },
    )
    .unwrap();
    assert_eq!(project.clips.len(), 1001);
    assert_eq!(project.clips[0].length_ticks, 496);
    assert_eq!(project.clips[1].length_ticks, 48);
    assert_eq!(project.missing_samples.len(), 1);
    assert!(project.missing_samples[0].length_inferred);
    assert_eq!(project.missing_samples[0].nominal_ticks, 1024);
    assert_eq!(project.missing_samples[0].metadata["Declared bars"], "2");
    assert!(
        project.missing_samples[0].metadata["Saved conflicting placements"]
            .ends_with("512, 32, 944, 1000\n")
    );
    assert!(
        project.missing_samples[0]
            .metadata
            .contains_key("Length confidence")
    );
}

#[test]
fn conflicting_placement_metadata_keeps_every_tuple_within_field_limits() {
    let conflicts = (0..100_000).map(|left| ((256, left, 512), 1)).collect();
    let metadata = conflicting_placement_metadata(&conflicts);
    assert!(metadata.len() > 1 && metadata.len() < 128);
    assert!(metadata.values().all(|value| value.len() <= 65_536));
    let rows: std::collections::BTreeSet<_> = metadata
        .values()
        .flat_map(|value| value.lines().skip(1))
        .collect();
    assert_eq!(rows.len(), 100_000);
    assert!(rows.contains("256, 99999, 512, 1"));
}

#[test]
fn an_embedded_wave_payload_cannot_hold_further_records() {
    fn riff(payload: &[u8]) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(payload.len() as u32 + 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(payload);
        bytes
    }
    fn record(path: &str, wave: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        wrapped(&mut bytes, path);
        bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
        bytes.extend_from_slice(wave);
        bytes
    }
    let nested = record(r"C:\nested.wav", &riff(&[0; 64]));
    let mut bytes = record(r"C:\outer.wav", &riff(&[nested, vec![0; 64]].concat()));
    bytes.extend(record(r"C:\next.wav", &riff(&[0; 64])));
    bytes.extend_from_slice(&[0; 16]);
    let paths: Vec<_> = embedded_wave_records(&bytes)
        .unwrap()
        .into_iter()
        .map(|source| source.path)
        .collect();
    assert_eq!(paths, [r"C:\outer.wav", r"C:\next.wav"]);
}

#[test]
fn resolved_wave_aliases_retain_one_owned_payload_within_the_asset_budget() {
    let bytes = wave(0.1749);
    let mut pending = Vec::new();
    // Only one payload fits. Aliases must neither consume that budget again
    // nor keep an uncounted audio copy in the resolution cache.
    let mut total = MAX_PENDING_ASSET_BYTES - bytes.len();
    let mut resolved = HashMap::new();
    for alias in 0..32 {
        let path = format!(r"C:\old-{alias}\take.wav");
        let retained = retain_wave(
            Wave {
                source_path: path.clone(),
                bytes: Cow::Owned(bytes.clone()),
                frames: 4,
                sample_rate: 44100,
            },
            &mut pending,
            &mut total,
        )
        .unwrap();
        resolved.insert(path, retained);
    }
    assert_eq!(total, MAX_PENDING_ASSET_BYTES);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1.as_ref(), bytes);
    for retained in resolved.values() {
        assert_eq!(retained.path, pending[0].0);
        assert_eq!((retained.frames, retained.sample_rate), (4, 44100));
    }
    assert!(
        retain_wave(
            Wave {
                source_path: "different.wav".into(),
                bytes: Cow::Owned(wave(0.1748)),
                frames: 4,
                sample_rate: 44100,
            },
            &mut pending,
            &mut total,
        )
        .unwrap_err()
        .contains("500 MB")
    );
    assert_eq!(pending.len(), 1);
}
