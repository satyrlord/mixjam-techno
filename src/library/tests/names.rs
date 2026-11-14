use super::*;

#[test]
fn product_folder_hints_come_from_the_selected_folder_and_below() {
    let hinted = |path: &str, root: &str| {
        in_foreign_product_folder(Path::new(path), Path::new(root), Product::D1)
    };
    // A product name above the selected folder says nothing about its files.
    assert!(!hinted(r"C:\Techno2\packs\kick.wav", r"C:\Techno2\packs"));
    assert!(hinted(r"C:\packs\Techno2\kick.wav", r"C:\packs"));
    assert!(hinted(r"C:\packs\Techno2\kick.wav", r"C:\packs\Techno2"));
    // The nearest product folder decides.
    assert!(!hinted(r"C:\packs\Techno2\Dance1\kick.wav", r"C:\packs"));
}

#[test]
fn folder_tempo_comes_from_agreeing_bar_lengths_or_stays_with_the_fallback() {
    let bar = |bpm: f64, bars: f64| ((44_100.0 * 240.0 / bpm * bars).round() as u64, 44_100);
    let dance = [
        bar(140.0, 1.0),
        bar(140.0, 2.0),
        bar(140.0, 4.0),
        (75_640, 44_100),
    ];
    assert_eq!(infer_folder_tempo(&dance, 120.0), Some(140.0));
    let house = [
        bar(125.0, 1.0),
        bar(125.0, 2.0),
        bar(125.0, 2.0),
        bar(125.0, 4.0),
    ];
    assert_eq!(infer_folder_tempo(&house, 140.0), Some(125.0));
    let rave = [bar(180.0, 1.0), bar(180.0, 2.0), bar(180.0, 2.0)];
    assert_eq!(infer_folder_tempo(&rave, 140.0), Some(180.0));
    // Two-bar loops fit 140 and 70 equally; the fallback decides.
    let long = [bar(140.0, 2.0), bar(140.0, 2.0), bar(140.0, 2.0)];
    assert_eq!(infer_folder_tempo(&long, 140.0), Some(140.0));
    assert_eq!(infer_folder_tempo(&long, 70.0), Some(70.0));
    // Long loops are not halved, and three or five bars are not preferred.
    let phrases = [bar(128.0, 4.0), bar(128.0, 4.0), bar(128.0, 8.0)];
    assert_eq!(infer_folder_tempo(&phrases, 128.0), Some(128.0));
    assert_eq!(infer_folder_tempo(&phrases, 140.0), Some(128.0));
    let sequences = [bar(140.0, 2.0), bar(140.0, 4.0), bar(140.0, 8.0)];
    assert_eq!(infer_folder_tempo(&sequences, 140.0), Some(140.0));
    assert_eq!(infer_folder_tempo(&sequences, 120.0), Some(140.0));
    let four = [bar(120.0, 4.0), bar(120.0, 4.0), bar(120.0, 4.0)];
    assert_eq!(infer_folder_tempo(&four, 140.0), Some(120.0));
    // One half-bar file among hundreds does not double a slow pack.
    let mut slow = vec![bar(90.0, 1.0); 149];
    slow.extend(vec![bar(90.0, 2.0); 391]);
    slow.push(bar(90.0, 3.0));
    slow.push(bar(90.0, 0.5));
    assert_eq!(infer_folder_tempo(&slow, 140.0), Some(90.0));
    assert_eq!(infer_folder_tempo(&slow, 180.0), Some(180.0));
    // The fallback names the octave outright when it fits.
    assert_eq!(infer_folder_tempo(&rave, 180.0), Some(180.0));
    // Long recordings do not agree on a tempo by accident.
    let recordings = [
        (8_401_050, 44_100),
        (9_801_225, 44_100),
        (13_274_100, 44_100),
    ];
    assert_eq!(infer_folder_tempo(&recordings, 140.0), None);
    let takes = [
        (7_938_000, 44_100),
        (8_820_000, 44_100),
        (10_584_000, 44_100),
    ];
    assert_eq!(infer_folder_tempo(&takes, 140.0), None);
    // Half-rate WAVs count by duration, not frames.
    let resampled = [(37_800, 22_050), (75_600, 22_050), (151_200, 22_050)];
    assert_eq!(infer_folder_tempo(&resampled, 140.0), Some(140.0));
    // One-shots, mixed packs, and tiny folders keep the fallback.
    let hits = [
        (9_001, 44_100),
        (12_345, 44_100),
        (23_456, 44_100),
        (31_111, 44_100),
    ];
    assert_eq!(infer_folder_tempo(&hits, 140.0), None);
    let mixed = [
        bar(140.0, 1.0),
        bar(125.0, 1.0),
        bar(180.0, 1.0),
        bar(160.0, 1.0),
    ];
    assert_eq!(infer_folder_tempo(&mixed, 140.0), None);
    assert_eq!(infer_folder_tempo(&dance[..2], 140.0), None);
    assert_eq!(infer_folder_tempo(&dance, f64::NAN), None);
}

#[test]
fn tempos_stated_in_pack_file_names_override_duration_guesses() {
    let folder = Folder::new();
    // One bar at 90 is two at 180: durations alone cannot tell them apart.
    for index in 1..=3 {
        folder.wave(
            &format!("Hardcore/HARDCORE_BASS00{index}_180_C_SL3.WAV"),
            44_100,
            117_600,
        );
    }
    // One bar at 125; a pack may also mix tempos within one folder.
    folder.wave("House/HOUSE_KEYS001_125_A_SL1.wav", 44_100, 84_672);
    folder.wave("House/HOUSE_KEYS002_125_Am_SL1.wav", 44_100, 84_672);
    folder.wave("House/TECHHOUSE_SYNTH040_132_A_SL3.WAV", 44_100, 80_182);
    folder.wave("House/SPOON001_CLAPBREAK_128_X_SC4(L).wav", 44_100, 82_688);
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    let tempo = |name: &str| {
        library
            .samples
            .iter()
            .find(|s| s.name == name)
            .unwrap()
            .source_bpm
    };
    assert_eq!(tempo("HARDCORE_BASS001_180_C_SL3"), 180.0);
    assert_eq!(tempo("HOUSE_KEYS002_125_Am_SL1"), 125.0);
    assert_eq!(tempo("TECHHOUSE_SYNTH040_132_A_SL3"), 132.0);
    assert_eq!(tempo("SPOON001_CLAPBREAK_128_X_SC4(L)"), 128.0);
    let reported: Vec<_> = library
        .folder_tempos()
        .iter()
        .map(|(path, bpm, count)| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                *bpm,
                *count,
            )
        })
        .collect();
    assert_eq!(
        reported,
        vec![
            ("Hardcore".to_string(), 180.0, 3),
            ("House".to_string(), 125.0, 2),
            ("House".to_string(), 128.0, 1),
            ("House".to_string(), 132.0, 1),
        ]
    );
}

#[test]
fn a_pack_bpm_inf_corrects_duration_guesses_in_its_folders() {
    let folder = Folder::new();
    let inf = |pack: &str, name: &str, bpm: u32| {
        let text =
            format!("[Tempo]\r\nBPM={bpm}\r\n\r\n[HarmonyTable]\r\nScale=c\r\nSample1=c\r\n");
        fs::write(folder.0.join(pack).join(name), text).unwrap();
    };
    // Three bars at 120 are four at 160; the durations prefer four.
    for index in 1..=3 {
        folder.wave(&format!("Flamenco/Guitar/g{index}.wav"), 44_100, 264_600);
    }
    inf("Flamenco", "BPM.inf", 120);
    // One bar at 80 reads as two at 160, which plays the same.
    for index in 1..=3 {
        folder.wave(&format!("Ambient/Pads/p{index}.wav"), 44_100, 132_300);
    }
    for (name, frames) in [("one", 9_001), ("two", 12_345), ("three", 23_456)] {
        folder.wave(&format!("Ambient/Fx/{name}.wav"), 44_100, frames);
    }
    inf("Ambient", "BPM.INF", 80);
    for (name, frames) in [("one", 9_001), ("two", 12_345), ("three", 23_456)] {
        folder.wave(&format!("Hits/{name}.wav"), 44_100, frames);
    }
    let mut library = Library::empty();
    library.import_wav_folder(&folder.0, 140.0).unwrap();
    let tempo = |name: &str| {
        library
            .samples
            .iter()
            .find(|s| s.name == name)
            .unwrap()
            .source_bpm
    };
    assert_eq!(tempo("g1"), 120.0);
    assert_eq!(tempo("p1"), 160.0);
    assert_eq!(tempo("two"), 160.0);
    let hits = fs::canonicalize(folder.0.join("Hits/one.wav")).unwrap();
    let hit = library
        .samples
        .iter()
        .find(|s| s.wav_path.as_ref() == Some(&hits));
    assert_eq!(hit.unwrap().source_bpm, 140.0);
}

#[test]
fn bpm_inf_tempo_is_the_bpm_key_of_the_tempo_section() {
    let tempo = stated_pack_tempo;
    assert_eq!(tempo("[Tempo]\nBPM=125\n"), Some(125.0));
    assert_eq!(
        tempo("[HarmonyTable]\nBPM=99\n[ tempo ]\r\n bpm = 87.5 \r\n"),
        Some(87.5)
    );
    assert_eq!(tempo("BPM=125\n"), None);
    assert_eq!(tempo("[Tempo]\nBPM=fast\n"), None);
    assert_eq!(tempo("[Tempo]\nBPM=1250\n"), None);
    assert!(octave_apart(80.0, 160.0) && octave_apart(180.0, 90.0));
    assert!(!octave_apart(120.0, 160.0));
}

#[test]
fn named_tempo_needs_a_bpm_token_followed_by_a_key_token() {
    let tempo = |name: &str| named_tempo(Path::new(name));
    assert_eq!(tempo("HARDCORE_BASS001_180_C_SL3.WAV"), Some(180.0));
    assert_eq!(tempo("DANCE_VOCALS001_140_X_SL1.wav"), Some(140.0));
    assert_eq!(tempo("SNTH_96_F#m_SC4(L).wav"), Some(96.0));
    assert_eq!(tempo("PAD_125_Bb_SL1.wav"), Some(125.0));
    assert_eq!(tempo("T2TRK0303  7.wav"), None);
    assert_eq!(tempo("r1bs001.wav"), None);
    assert_eq!(tempo("LOOP_140_SL1.wav"), None);
    assert_eq!(tempo("LOOP_1400_A_SL1.wav"), None);
    assert_eq!(tempo("LOOP_20_A_SL1.wav"), None);
    assert_eq!(tempo("LOOP_140_H_SL1.wav"), None);
}

#[test]
fn names_preserve_exact_slug_and_normalize_numeric_padding() {
    assert_eq!(compact_name("soft dust 06"), compact_name("soft-dust-6"));
    assert_ne!(slug("tech noR"), slug("alpha R"));
    assert_eq!(decode_1252(&[0x80, 0xe9]), "€é");
}
