use super::*;

mod conflicts;
mod preferences;
mod state;
mod stretch;

#[test]
fn placement_ticks_reports_the_length_that_place_uses() {
    let sample = |id, frame_count| Sample {
        id,
        code: "FIXTURE".into(),
        name: "fixture".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 0,
        frame_count,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    };
    // No product owns this ID, so its length comes from the audio.
    let wave = 900_001;
    assert!(Product::sample_key(wave).is_none());
    let library = Library::fixture(vec![sample(1, 75_600), sample(wave, 151_200)]);
    let mut session = Session::headless(Arc::new(library), Project::default()).unwrap();
    assert_eq!(session.placement_ticks(1), Some(TICKS_PER_BAR));
    assert_eq!(session.placement_ticks(wave), Some(2 * TICKS_PER_BAR));
    assert_eq!(session.placement_ticks(7), None);
    for (track, id) in [1, wave].into_iter().enumerate() {
        let clip = session.place(id, track, 0).unwrap();
        let placed = session
            .project()
            .clips
            .iter()
            .find(|c| c.id == clip)
            .unwrap();
        assert_eq!(Some(placed.length_ticks), session.placement_ticks(id));
    }
}

fn manual_fixture_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "mixjam-manual-resolution-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
fn conflict_fixture() -> (Session, PathBuf) {
    let root = manual_fixture_dir();
    let first = root.join("first.wav");
    let second = root.join("second.wav");
    for (path, rate, frames, value) in [
        (&first, 22_050, 22_050, 0.25),
        (&second, 44_100, 88_200, -0.25),
    ] {
        Pcm {
            sample_rate: rate,
            channels: 1,
            samples: vec![value; frames],
        }
        .write_wav(path)
        .unwrap();
    }
    let first = std::fs::canonicalize(first).unwrap();
    let second = std::fs::canonicalize(second).unwrap();
    let sources: Vec<_> = [256, 512]
        .into_iter()
        .enumerate()
        .map(|(index, ticks)| crate::engine::ImportedSample {
            id: 1_000_000 + index as u32,
            name: "Lead".into(),
            group_name: "MixJam".into(),
            path: first.clone(),
            source_bpm: f64::from(ticks) * 240.0 / f64::from(TICKS_PER_BAR),
            source_sample_rate: Some(22_050),
            origin: None,
        })
        .collect();
    let mut library = Library::empty();
    library.apply_imports(&sources).unwrap();
    let project = Project {
        imported_samples: sources.into(),
        sample_conflicts: vec![SampleConflict {
            reference: "Old/lead.wav".into(),
            sample_ids: vec![1_000_000, 1_000_001],
            candidates: vec![first, second],
            selected: 0,
            confirmed: false,
            source_spans: [(1_000_000, 256), (1_000_001, 512)].into(),
        }],
        clips: [1_000_000, 1_000_001, 1_000_000]
            .into_iter()
            .enumerate()
            .map(|(index, id)| Clip {
                id: index as u64 + 1,
                sample_id: id,
                track: index * 4,
                start_tick: 17,
                length_ticks: 256,
                offset_frames: 11_025,
                gain: 0.75,
                fades: Default::default(),
                pitch_cents: 0,
                reverse: false,
            })
            .collect(),
        ..Project::default()
    };
    (Session::headless(Arc::new(library), project).unwrap(), root)
}

fn fixture() -> Session {
    let sample = Sample {
        id: 1,
        code: "FIXTURE".into(),
        name: "good grasp 2".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 0,
        frame_count: 75600,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    };
    let library = Arc::new(Library::fixture(vec![sample]));
    let mut editor = Editor::new(Project::default());
    editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
    editor.place(1, 1, TICKS_PER_BAR, TICKS_PER_BAR).unwrap();
    Session::headless(library, editor.into_project()).unwrap()
}
