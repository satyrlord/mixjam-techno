use super::*;

#[test]
fn counted_profiles_import_from_wav_only_native_providers() {
    let root = asset_test_directory("counted-wav");
    let wave_path = root.join("sample.wav");
    fs::write(&wave_path, wave_fixture()).unwrap();
    let path = root.join("song.mix");
    for product in [
        Product::D2,
        Product::D3,
        Product::D4,
        Product::H1,
        Product::T3,
    ] {
        let mut library = Library::empty();
        library.import_wav_folder(&root, 140.0).unwrap();
        library
            .register_wav_product(
                product,
                &root,
                &[crate::catalog::WaveMetadata {
                    native_id: 174,
                    code: "sample".into(),
                    label_key: crate::catalog::label_key("Sample"),
                    group_name: "Test".into(),
                    bars: 2,
                    source_bpm: 140.0,
                    frame_count: 4,
                    pcm_digest: None,
                }],
            )
            .unwrap();
        let mut bytes = header_fixture(product, 140, 140);
        let mut chunk = Vec::new();
        lane(&mut chunk, 1, &[(0, -129, 174)]);
        bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&SENTINEL);
        fs::write(&path, bytes).unwrap();
        // Song detection keeps its own copy of each profile's app ID.
        assert_eq!(crate::project_io::detect_product(&path).unwrap(), product);
        let project = load(&path, product, &library).unwrap();
        assert_eq!(
            project.clips[0].sample_id,
            product.global_sample_id(174).unwrap()
        );
        assert_eq!(project.clips[0].start_tick, 256);
        assert_eq!(project.clips[0].length_ticks, 512);
    }
    fs::remove_file(path).unwrap();
    fs::remove_file(wave_path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn typed_header_preserves_tempo_and_centered_pan() {
    for product in [Product::D2, Product::D3, Product::T3, Product::H1] {
        let bytes = header_fixture(product, 125, 146);
        let header = read_header(&bytes, product).unwrap();
        assert_eq!(header.title, "Fixture title");
        assert_eq!(header.author, "Composer");
        assert_eq!(header.bpm, 125.0);
        assert_eq!(header.effective_bpm, 146.0);
        assert_eq!(header.tracks.len(), TRACK_COUNT);
        assert_eq!(header.tracks[0].pan, 0.0);
        let gain = if product == Product::D2 {
            40961.0 / 65536.0
        } else {
            43582.0 / 65536.0
        };
        assert_eq!(header.tracks[0].gain, gain);
        let project = build_project(
            Path::new("test.mix"),
            &bytes,
            product,
            &Library::empty(),
            header,
            Timeline {
                start: bytes.len(),
                events: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(project.bpm, 140.0);
        assert_eq!(project.pitch_ratio, 1.0);
        assert_eq!(project.import_notices[0].source_bpm, 146.0);
        assert_eq!(project.import_notices[0].target_bpm, 140.0);
    }
    assert_eq!(decode_pan(0).unwrap(), -1.0);
    assert_eq!(decode_pan(128).unwrap(), 0.0);
    assert_eq!(decode_pan(255).unwrap(), 1.0);
    assert!(decode_pan(256).is_err());
}

#[test]
fn native_mute_solo_and_utility_lane_settings_survive_import_and_json() {
    for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
        let mut bytes = header_fixture(product, 140, 140);
        let core_count = profile(product).unwrap().1;
        let count = core_count + usize::from(product != Product::H1);
        let start = bytes.len() - count * 8;
        // A normal mute, a muted solo, and an unmuted solo on the last saved
        // lane exercise precedence and an empty utility lane's solo switch.
        let outputs_on = |tracks: &[Track]| {
            tracks
                .iter()
                .all(|track| track.left_enabled && track.right_enabled)
        };
        let audible = |tracks: &[Track]| {
            let any_solo = tracks.iter().any(|track| track.solo);
            tracks
                .iter()
                .map(|track| track.audible(any_solo))
                .collect::<Vec<_>>()
        };
        bytes[start + 4..start + 6].copy_from_slice(&1u16.to_le_bytes());
        let muted = read_header(&bytes, product).unwrap();
        assert!(outputs_on(&muted.tracks));
        assert!(muted.tracks[0].muted && !muted.tracks[0].solo);
        assert_eq!(audible(&muted.tracks)[..2], [false, true]);
        bytes[start + 6..start + 8].copy_from_slice(&1u16.to_le_bytes());
        let last = start + (count - 1) * 8;
        bytes[last + 2..last + 4].copy_from_slice(&20000u16.to_le_bytes());
        bytes[last + 6..last + 8].copy_from_slice(&1u16.to_le_bytes());
        let header = read_header(&bytes, product).unwrap();
        assert_eq!(header.after_mixer, bytes.len());
        assert!(outputs_on(&header.tracks));
        assert!(header.tracks[0].muted && header.tracks[0].solo);
        assert!(!header.tracks[count - 1].muted && header.tracks[count - 1].solo);
        let heard = audible(&header.tracks);
        assert_eq!(heard.iter().filter(|audible| **audible).count(), 1);
        assert!(heard[count - 1]);
        assert!(
            header.tracks[count..]
                .iter()
                .all(|track| track == &Track::default())
        );

        // A solo of 2 keeps its lane audible without starting solo
        // playback. No Mute or Solo state says that, so the song keeps
        // the earlier conversion to outputs.
        let mut odd = bytes.clone();
        odd[start + 14..start + 16].copy_from_slice(&2u16.to_le_bytes());
        let fallback = read_header(&odd, product).unwrap();
        assert!(
            fallback
                .tracks
                .iter()
                .all(|track| !track.muted && !track.solo)
        );
        let enabled: Vec<_> = fallback.tracks[..count]
            .iter()
            .map(|track| track.left_enabled && track.right_enabled)
            .collect();
        let mut expected = vec![false; count];
        expected[1] = true;
        expected[count - 1] = true;
        assert_eq!(enabled, expected);
        assert!(fallback.source_notices.iter().any(
            |notice| notice.contains("solo=2") && notice.contains("applied to the L/R outputs")
        ));
        let expected_gain = native_fader_gain(20000, 1.33);
        assert_eq!(header.tracks[count - 1].gain, expected_gain);
        let project = build_project(
            Path::new("switches.mix"),
            &bytes,
            product,
            &Library::empty(),
            header,
            Timeline {
                start: bytes.len(),
                events: Vec::new(),
            },
        )
        .unwrap();
        let reopened: Project =
            serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        assert_eq!(reopened.tracks, project.tracks);
        assert_eq!(reopened.source_notices, project.source_notices);
        assert!(
            reopened
                .source_notices
                .iter()
                .any(|notice| notice.contains("mute=1, solo=1"))
        );
    }
}

#[test]
fn saved_settings_after_the_title_survive_import_and_json() {
    let title = b"Song\0BoostEQ#\xb0_#0%\xb0_%BoostCompressorLED#\xb0_#1%\
                      \xb0_%BoostCompressorDrive#\xb0_#12080%\xb0_%FX_12#\xb0_#1%\xb0_%DrumEcho#\
                      \xb0_#1%\xb0_%\0\x01";
    for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
        let bytes = titled_header_fixture(product, 140, 140, title);
        let header = read_header(&bytes, product).unwrap();
        assert_eq!(header.title, "Song");
        assert_eq!(header.settings.entries.len(), 5);
        let project = build_project(
            Path::new("settings.mix"),
            &bytes,
            product,
            &Library::empty(),
            header,
            Timeline {
                start: bytes.len(),
                events: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(project.title, "Song");
        assert_eq!(project.source_settings["BoostCompressorDrive"], "12080");
        assert_eq!(project.source_settings["BoostEQ"], "0");
        // The Booster compressor plays as a master pedal. No drum pattern
        // uses the kit, so no other saved effect changes the sound.
        if product == Product::H1 {
            assert!(!project.master.is_active());
            assert_eq!(project.source_notices.len(), 1);
            assert!(project.source_notices[0].starts_with("House saved 5 settings"));
        } else {
            assert!(project.master.booster_compressor.on);
            assert!(!project.master.booster_eq.on && !project.master.booster_wide.on);
            assert_eq!(project.master.booster_compressor.drive, 369);
            assert!(project.source_notices.is_empty());
        }
        let reopened: Project =
            serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        assert_eq!(reopened.source_settings, project.source_settings);
        assert_eq!(reopened.master, project.master);
    }
    // A song without the list has no settings and no notice.
    let bytes = header_fixture(Product::D4, 140, 140);
    let header = read_header(&bytes, Product::D4).unwrap();
    assert_eq!(header.settings, settings::Saved::default());
}

#[test]
fn native_headers_use_original_pan_regions_and_signed_fader_curve() {
    for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
        let mut bytes = header_fixture(product, 140, 140);
        let count = profile(product).unwrap().1 + usize::from(product != Product::H1);
        let start = bytes.len() - count * 8;
        for (pan, expected) in [
            (0, -1.0),
            (14, -1.0),
            (15, -113.0 / 128.0),
            (116, -12.0 / 128.0),
            (117, 0.0),
            (128, 0.0),
            (139, 0.0),
            (140, 12.0 / 128.0),
            (240, 112.0 / 128.0),
            (241, 1.0),
            (255, 1.0),
        ] {
            bytes[start..start + 2].copy_from_slice(&u16::to_le_bytes(pan));
            bytes[start + 2..start + 4].copy_from_slice(&25200u16.to_le_bytes());
            let header = read_header(&bytes, product).unwrap();
            let track = &header.tracks[0];
            assert_eq!(track.pan, expected, "saved pan {pan}");
            assert_eq!(track.left_enabled, expected < 1.0);
            assert_eq!(track.right_enabled, expected > -1.0);
            assert_eq!(track.gain, native_fader_gain(25200, 1.33));
        }
        bytes[start + 2..start + 4].copy_from_slice(&u16::MAX.to_le_bytes());
        let track = &read_header(&bytes, product).unwrap().tracks[0];
        assert!(track.polarity_inverted);
        assert_eq!(track.gain, 2.0 / 65536.0);
        assert_eq!(track.effective_gain(), -2.0 / 65536.0);
        bytes[start..start + 2].copy_from_slice(&256u16.to_le_bytes());
        assert!(read_header(&bytes, product).is_err());
    }
}

#[test]
fn imported_native_mixer_settings_control_rendered_channels_and_levels() {
    for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
        let mut bytes = header_fixture(product, 140, 140);
        let count = profile(product).unwrap().1 + usize::from(product != Product::H1);
        let start = bytes.len() - count * 8;
        for (index, pan) in [0u16, 255, 128].into_iter().enumerate() {
            let offset = start + index * 8;
            bytes[offset..offset + 2].copy_from_slice(&pan.to_le_bytes());
            bytes[offset + 2..offset + 4].copy_from_slice(&25200u16.to_le_bytes());
        }
        bytes[start + 20..start + 22].copy_from_slice(&1u16.to_le_bytes());
        let samples = HashMap::from([(
            1,
            crate::audio::Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.1; 8],
            },
        )]);
        let mut project = Project {
            tracks: read_header(&bytes, product).unwrap().tracks,
            master_gain: 1.0,
            clips: (0..3)
                .map(|track| Clip {
                    id: track as u64 + 1,
                    sample_id: 1,
                    track,
                    start_tick: 0,
                    length_ticks: 1,
                    offset_frames: 0,
                    gain: if track == 1 { 2.0 } else { 1.0 },
                    fades: Default::default(),
                    pitch_cents: 0,
                    reverse: false,
                })
                .collect(),
            ..Project::default()
        };
        let output = crate::audio::render_with_samples(&project, &samples).unwrap();
        // 25200 maps to native Q16 coefficient 70133, not 25200/32767.
        assert_eq!(output.samples[0], 0.1 * (70133.0 / 65536.0));
        assert_eq!(output.samples[1], 0.2 * (70133.0 / 65536.0));
        bytes[start + 14..start + 16].copy_from_slice(&1u16.to_le_bytes());
        project.tracks = read_header(&bytes, product).unwrap().tracks;
        let solo = crate::audio::render_with_samples(&project, &samples).unwrap();
        assert_eq!(solo.samples[0], 0.0);
        assert_eq!(solo.samples[1], output.samples[1]);
        bytes[start + 12..start + 14].copy_from_slice(&1u16.to_le_bytes());
        project.tracks = read_header(&bytes, product).unwrap().tracks;
        assert!(
            crate::audio::render_with_samples(&project, &samples)
                .unwrap()
                .samples
                .iter()
                .all(|sample| *sample == 0.0)
        );
    }
}

#[test]
#[ignore = "Requires the local installed D3, D4, and House song fixtures."]
fn installed_native_mixer_headers_preserve_artist_settings() {
    let root = crate::project_io::default_root(Product::D3).unwrap();
    let header = read_header(
        &read_song_file(&root.join("MIX/start.mix")).unwrap(),
        Product::D3,
    )
    .unwrap();
    assert_eq!(header.after_mixer, 1319 + 33 * 8);
    assert_eq!(header.tracks[2].gain, 70133.0 / 65536.0);
    assert_eq!(
        (
            header.tracks[2].left_enabled,
            header.tracks[2].right_enabled
        ),
        (true, false)
    );
    assert_eq!(
        (
            header.tracks[3].left_enabled,
            header.tracks[3].right_enabled
        ),
        (false, true)
    );
    assert_eq!(header.tracks[32].gain, 53201.0 / 65536.0);
    assert_eq!(header.tracks[32].pan, 0.0);
    for (product, name, muted) in [
        (Product::H1, "amsterdam.mix", vec![13]),
        (Product::D4, "Deep house monotonic.mix", vec![15, 16]),
    ] {
        let root = crate::project_io::default_root(product).unwrap();
        let header = read_header(
            &read_song_file(&root.join("MIX").join(name)).unwrap(),
            product,
        )
        .unwrap();
        for index in muted {
            assert!(header.tracks[index].muted);
            assert!(header.tracks[index].left_enabled || header.tracks[index].right_enabled);
            assert!(header.tracks[index].gain > 0.0);
        }
    }
}

#[test]
#[ignore = "Requires the local installed D3, D4, T3, and House song fixtures."]
fn installed_songs_save_mute_and_solo_as_zero_or_one() {
    // Every installed song maps exactly to Mute and Solo; none needs the
    // conversion to outputs that another saved value would require.
    for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
        let folder = crate::project_io::default_root(product)
            .unwrap()
            .join("MIX");
        let (mut songs, mut muted, mut soloed) = (0, 0, 0);
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
            {
                continue;
            }
            let header = read_header(&read_song_file(&path).unwrap(), product).unwrap();
            assert!(
                !header
                    .source_notices
                    .iter()
                    .any(|notice| notice.contains("no Mute or Solo equivalent")),
                "{}",
                path.display()
            );
            songs += 1;
            muted += header.tracks.iter().filter(|track| track.muted).count();
            soloed += header.tracks.iter().filter(|track| track.solo).count();
        }
        assert!(songs > 0, "{}", product.label());
        println!(
            "{}: {songs} songs, {muted} muted and {soloed} soloed tracks",
            product.label()
        );
    }
}

#[test]
#[ignore = "Requires the local installed D3, D4, T3, and House song fixtures."]
fn installed_songs_keep_their_saved_settings_lists() {
    // Songs, songs with a list, and songs with the EQ, compressor, and
    // stereo wide switch of the Booster on.
    for (product, expected) in [
        (Product::D3, (17, 14, [0, 0, 0])),
        (Product::D4, (12, 12, [3, 6, 6])),
        (Product::T3, (20, 20, [0, 1, 1])),
        (Product::H1, (14, 14, [0, 0, 0])),
    ] {
        let folder = crate::project_io::default_root(product)
            .unwrap()
            .join("MIX");
        let (mut songs, mut lists, mut switches, mut pedals) = (0, 0, [0; 3], [0; 3]);
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
            {
                continue;
            }
            let saved = read_header(&read_song_file(&path).unwrap(), product)
                .unwrap()
                .settings;
            assert_eq!(saved.unread, 0, "{}", path.display());
            songs += 1;
            lists += usize::from(!saved.entries.is_empty());
            for (count, key) in
                switches
                    .iter_mut()
                    .zip(["BoostEQ", "BoostCompressorLED", "BoostStereoLED"])
            {
                *count += usize::from(saved.entries.get(key).is_some_and(|v| v == "1"));
            }
            // Every saved Booster value is usable, so each switch that is
            // on becomes a pedal that is on.
            let (master, unusable) = settings::booster(product, &saved);
            assert!(unusable.is_empty(), "{}", path.display());
            pedals[0] += usize::from(master.booster_eq.on);
            pedals[1] += usize::from(master.booster_compressor.on);
            pedals[2] += usize::from(master.booster_wide.on);
        }
        assert_eq!((songs, lists, switches), expected, "{}", product.label());
        assert_eq!(pedals, expected.2, "{}", product.label());
    }
    let root = crate::project_io::default_root(Product::D4).unwrap();
    let saved = read_header(
        &read_song_file(&root.join("MIX/welmix.mix")).unwrap(),
        Product::D4,
    )
    .unwrap()
    .settings;
    assert_eq!(saved.entries.len(), 93);
    assert_eq!(saved.entries["BoostCompressorDrive"], "12080");
    assert_eq!(saved.entries["BoostStereoWide"], "21348");
    assert_eq!(saved.entries["DrumK_SFX_ECHOTIME"], "5529");
    assert_eq!(saved.entries["FX_12"], "1");
}

#[test]
fn header_rejects_truncation_foreign_identity_and_accepts_forty_eight_tracks() {
    let bytes = header_fixture(Product::T3, 140, 140);
    for end in [0, 3, 13, 16, bytes.len() - 1] {
        assert!(read_header(&bytes[..end], Product::T3).is_err());
    }
    assert!(read_header(&bytes, Product::D3).is_err());
    let oversized = header_fixture(Product::D4, 140, 140);
    assert_eq!(
        read_header(&oversized, Product::D4).unwrap().tracks.len(),
        TRACK_COUNT
    );
    assert!(read_header(&header_fixture(Product::H1, 0, 125), Product::H1).is_err());
}

#[test]
fn native_clock_and_signed_fader_values_keep_the_recovered_contract() {
    assert_eq!(native_position(-129).unwrap(), 256);
    assert_eq!(native_position(8).unwrap(), 16);
    assert!(native_position(i32::MIN).is_err());
    assert!(native_tick_count(MAX_TICKS).is_err());
    assert_eq!(native_fader_gain(65535, 1.33), -2.0 / 65536.0);
    assert_eq!(native_fader_gain(32767, 1.33), 154182.0 / 65536.0);
    let mut pending = Vec::new();
    let mut total = MAX_PENDING_ASSET_BYTES;
    assert!(queue_asset(&mut pending, &mut total, PathBuf::from("unused"), vec![1]).is_err());
    assert!(pending.is_empty());
}
