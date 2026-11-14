use super::*;

#[test]
fn last_of_sixty_four_tracks_supports_edit_undo_and_redo() {
    let mut editor = Editor::new(Project::default());
    assert_eq!(editor.project.tracks.len(), 64);
    editor.project.validate().unwrap();
    let id = editor.place(1, 63, 17, 31).unwrap();
    let placed = editor.project.clone();
    editor.move_clips(&[id], -1, 31, false).unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, placed);
    assert!(editor.redo());
    assert_eq!(editor.project.clips[0].track, 62);
    let before = editor.project.clone();
    assert!(editor.place(1, 64, 0, 256).is_err());
    assert!(
        editor
            .transact(|project| project.tracks.push(Track::default()))
            .is_err()
    );
    assert_eq!(editor.project, before);
}

#[test]
fn move_is_atomic_and_adjacent_clips_fit() {
    let mut e = Editor::new(Project::default());
    let a = e.place(1, 0, 0, 256).unwrap();
    e.place(2, 0, 256, 256).unwrap();
    let before = e.project.clone();
    assert!(e.move_clips(&[a], 0, 128, false).is_err());
    assert_eq!(e.project, before);
    e.move_clips(&[a], 1, 128, false).unwrap();
    assert!(e.undo());
    assert_eq!(e.project, before);
    assert!(e.redo());
    // A move appends the clip again, so find it by identity.
    let moved = e.project.clips.iter().find(|c| c.id == a).unwrap();
    assert_eq!(moved.track, 1);
}

#[test]
fn invalid_edit_rolls_back_and_redo_branch_is_cleared() {
    let mut e = Editor::new(Project::default());
    assert!(e.transact(|p| p.bpm = f64::NAN).is_err());
    assert_eq!(e.project.bpm, 140.0);
    e.place(1, 0, 0, 256).unwrap();
    e.undo();
    e.place(2, 1, 0, 256).unwrap();
    assert!(!e.redo());
}

#[test]
fn history_entries_share_the_import_list_until_an_edit_changes_it() {
    let sample = |id| ImportedSample {
        id,
        name: "Palette sound".into(),
        group_name: "Wave".into(),
        path: PathBuf::from("palette.wav"),
        source_bpm: 140.0,
        source_sample_rate: None,
        origin: None,
    };
    let mut e = Editor::new(Project {
        imported_samples: vec![sample(1_000_000)].into(),
        ..Project::default()
    });
    e.place(1, 0, 0, 256).unwrap();
    e.transact(|p| p.master_gain = 1.0).unwrap();
    // Project copies hold one allocation until an edit changes the imports.
    assert!(e.undo.iter().all(|snapshot| {
        std::sync::Arc::ptr_eq(&snapshot.imported_samples.0, &e.project.imported_samples.0)
    }));
    e.transact(|p| p.imported_samples.push(sample(1_000_001)))
        .unwrap();
    let before = e.undo.last().unwrap();
    assert!(!std::sync::Arc::ptr_eq(
        &before.imported_samples.0,
        &e.project.imported_samples.0
    ));
    assert_eq!(before.imported_samples.len(), 1);
    assert_eq!(e.project.imported_samples.len(), 2);
}

#[test]
fn zero_move_preserves_clip_order_history_and_redo_and_checks_ids() {
    let mut editor = Editor::new(Project::default());
    let first = editor.place(1, 0, 0, 256).unwrap();
    let second = editor.place(2, 0, 256, 256).unwrap();
    editor
        .transact(|project| project.master_gain = 1.0)
        .unwrap();
    assert!(editor.undo());
    let before = editor.project.clone();
    let generation = editor.generation();
    let undo_count = editor.undo.len();
    let redo = editor.redo.clone();
    assert_eq!(editor.move_clips(&[first], 0, 0, false).unwrap(), [first]);
    assert_eq!(
        editor.move_clips(&[second, first], 0, 0, false).unwrap(),
        [second, first]
    );
    assert!(editor.move_clips(&[first, u64::MAX], 0, 0, false).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.generation(), generation);
    assert_eq!(editor.undo.len(), undo_count);
    assert_eq!(editor.redo, redo);
    assert!(editor.redo());
    assert_eq!(editor.project.master_gain, 1.0);
}

#[test]
fn group_moves_validate_final_positions_and_undo_once() {
    let mut e = Editor::new(Project::default());
    let a = e.place(1, 0, 0, 256).unwrap();
    let b = e.place(2, 0, 256, 256).unwrap();
    let before = e.project.clone();
    e.move_clips(&[a, b], 0, 256, false).unwrap();
    assert_eq!(e.project.clips[0].start_tick, 256);
    assert!(e.undo());
    assert_eq!(e.project, before);
    assert!(e.move_clips(&[a, b], -1, 0, false).is_err());
    assert_eq!(e.project, before);
    let copied = e.move_clips(&[a, b], 1, 0, true).unwrap();
    assert_eq!(copied.len(), 2);
    assert_eq!(e.project.clips.len(), 4);
    assert!(e.undo());
    assert_eq!(e.project, before);
}

#[test]
fn new_sources_keep_history_only_while_its_clips_name_the_same_sounds() {
    let sample = |id, path: &str| ImportedSample {
        id,
        name: "Palette sound".into(),
        group_name: "Wave".into(),
        path: PathBuf::from(path),
        source_bpm: 140.0,
        source_sample_rate: None,
        origin: None,
    };
    let start = Project {
        imported_samples: vec![sample(1_000_000, "a.wav")].into(),
        ..Project::default()
    };
    let mut e = Editor::new(start.clone());
    e.place(1_000_000, 0, 0, 256).unwrap();
    e.transact(|p| p.clips.clear()).unwrap();
    // A larger palette that keeps the used sound keeps both undo steps.
    let mut grown = e.project.clone();
    grown.imported_samples = vec![sample(1_000_000, "a.wav"), sample(1_000_001, "b.wav")].into();
    e.rebind_sources(grown.clone()).unwrap();
    assert_eq!(e.project, grown);
    assert!(
        e.undo
            .iter()
            .all(|step| step.imported_samples == grown.imported_samples)
    );
    // A palette that gives the placed clip's ID another sound refuses.
    let mut moved = e.project.clone();
    moved.imported_samples = vec![sample(1_000_000, "c.wav")].into();
    assert_eq!(*e.rebind_sources(moved.clone()).unwrap_err(), moved);
    assert_eq!(e.project, grown);
    // So does a project that differs in more than its sources.
    let mut edited = grown.clone();
    edited.master_gain = 0.25;
    assert!(e.rebind_sources(edited).is_err());
    assert!(e.undo());
    assert_eq!(e.project.clips.len(), 1);
}
