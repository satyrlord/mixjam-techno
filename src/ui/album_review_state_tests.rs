use super::*;
use mixjam_techno::application::album_review::covers::CoverCache;

fn fixture_folder() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "mixjam-review-state-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn clearing_waits_for_cover_results_and_cannot_restore_a_cleared_cover() {
    let folder = fixture_folder();
    let cache = CoverCache::new(folder.clone(), 1 << 20);
    cache
        .store(
            "album",
            include_bytes!("../../assets/rail/album-review.png"),
        )
        .unwrap();
    let mut state = AlbumReviewState::default();
    state.use_folder(None, cache.clone());
    state.covers().request("album", None);
    // Busy includes completed results that the UI has not consumed yet.
    assert!(state.covers().busy());
    state.clear_cache();
    assert_eq!(
        state.cache_message.as_deref(),
        Some("Wait for cover loading to finish.")
    );
    assert_eq!(cache.usage().0, 1);
    let ctx = egui::Context::default();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while state.covers().busy() && std::time::Instant::now() < deadline {
        state.poll(&ctx);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!state.covers().busy());
    assert!(state.textures.contains_key("album"));
    state.clear_cache();
    state.poll(&ctx);
    assert!(state.textures.is_empty());
    assert_eq!(cache.usage(), (0, 0));
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn the_album_page_copies_its_review_once_for_each_shelf_revision() {
    let folder = fixture_folder();
    std::fs::write(
        folder.join("brian-eno-apollo.json"),
        include_str!("../../assets/album-review/brian-eno-apollo.json"),
    )
    .unwrap();
    let mut state = AlbumReviewState::default();
    state.use_folder(
        Some(folder.clone()),
        CoverCache::new(folder.join("covers"), 1 << 20),
    );
    let first = state
        .page_review("brian-eno-apollo")
        .expect("The review is on the shelf.");
    let again = state.page_review("brian-eno-apollo").unwrap();
    assert!(
        Rc::ptr_eq(&first, &again),
        "A frame must not copy the review."
    );
    let mut changed = (*first).clone();
    changed.overview.push_str(" A closing note.");
    state.shelf().update(changed).unwrap();
    let second = state.page_review("brian-eno-apollo").unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert!(second.overview.ends_with(" A closing note."));
    assert!(state.page_review("no-such-review").is_none());
    std::fs::remove_dir_all(folder).unwrap();
}
