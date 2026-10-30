use super::*;

#[test]
fn clearing_waits_for_cover_results_and_cannot_restore_a_cleared_cover() {
    let folder =
        std::env::temp_dir().join(format!("mixjam-cover-clear-pending-{}", std::process::id()));
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
