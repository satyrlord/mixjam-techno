//! Album Review window through its accessible controls, as a person or an
//! agent uses it.
use super::*;
use crate::library::Library;
use mixjam_techno::application::album_review::{
    Energy, Event, Kind, Review, Track, covers::CoverCache,
};

fn folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("mixjam-album-window-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("reviews")).unwrap();
    folder
}

fn review(id: &str, artist: &str, title: &str, year: i32, genres: &[&str]) -> Review {
    let track = |num: u32, energy: Energy| Track {
        num,
        title: format!("Part {num}"),
        duration: "4:00".into(),
        energy,
        tags: vec!["Arpeggio".into(), "Pad".into()],
        role: "Album role: Carries the theme.".into(),
        events: vec![
            Event {
                timestamp: "0:00".into(),
                section: "Intro".into(),
                description: "A pad swells.".into(),
                detail: Some("Filtered noise under it.".into()),
            },
            Event {
                timestamp: "2:00".into(),
                section: "Theme".into(),
                description: "The lead enters.".into(),
                detail: None,
            },
        ],
        local_audio: None,
        song_file: None,
    };
    Review {
        id: id.into(),
        kind: Kind::Reference,
        artist: artist.into(),
        title: title.into(),
        year,
        label: "Label".into(),
        producer: "Producer".into(),
        genre: genres.join(" / "),
        genre_tags: genres.iter().map(|genre| (*genre).to_owned()).collect(),
        runtime: "8:00".into(),
        is_soundtrack: None,
        // Nothing listens on the discard port, so a download fails at once.
        cover_url: Some("http://127.0.0.1:9/cover.jpg".into()),
        cover_source: Some("https://en.wikipedia.org/wiki/Album".into()),
        youtube_url: Some("https://www.youtube.com/watch?v=1".into()),
        bandcamp_url: None,
        local_audio: None,
        song_file: None,
        overview: "First paragraph of the overview.\n\nSecond paragraph.".into(),
        tracks: vec![track(1, Energy::Low), track(2, Energy::Peak)],
    }
}

fn fixture(name: &str, reviews: &[Review]) -> (egui::Context, MixJamApp, PathBuf) {
    let root = folder(name);
    for review in reviews {
        std::fs::write(
            root.join("reviews").join(format!("{}.json", review.id)),
            review.to_file_text(),
        )
        .unwrap();
    }
    let context = egui::Context::default();
    configure_style(&context);
    context.enable_accesskit();
    let mut app = MixJamApp::from_session(
        Session::headless(Arc::new(Library::fixture(Vec::new())), Project::default()).unwrap(),
        "Album window".into(),
        false,
    );
    app.capture_path = None;
    app.album_review.use_folder(
        Some(root.join("reviews")),
        CoverCache::new(root.join("covers"), 1 << 20),
    );
    (context, app, root)
}

fn frame(ctx: &egui::Context, app: &mut MixJamApp, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
        events,
        focused: true,
        ..Default::default()
    };
    ctx.run(input, |ctx| app.draw(ctx))
}

fn nodes(output: &egui::FullOutput) -> Vec<(egui::accesskit::NodeId, egui::accesskit::Node)> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled.")
        .nodes
        .clone()
}

fn node(output: &egui::FullOutput, label: &str) -> Option<egui::accesskit::Node> {
    nodes(output)
        .into_iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(_, node)| node)
}

fn has_text(output: &egui::FullOutput, text: &str) -> bool {
    nodes(output).iter().any(|(_, node)| {
        node.value().is_some_and(|value| value.contains(text))
            || node.label().is_some_and(|label| label.contains(text))
    })
}

/// Click the control named `label` and draw the frames that show the result.
fn click(ctx: &egui::Context, app: &mut MixJamApp, label: &str) -> egui::FullOutput {
    let output = frame(ctx, app, Vec::new());
    let target = nodes(&output)
        .into_iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, _)| id)
        .unwrap_or_else(|| {
            let labels: Vec<_> = nodes(&output)
                .into_iter()
                .map(|(_, node)| format!("{:?}={:?}", node.label(), node.value()))
                .collect();
            panic!("Missing control: {label} among {labels:?}")
        });
    frame(
        ctx,
        app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
    );
    frame(ctx, app, Vec::new());
    frame(ctx, app, Vec::new())
}

#[test]
fn the_rail_button_opens_the_grid_with_its_count_filters_and_sort() {
    let (ctx, mut app, root) = fixture(
        "grid",
        &[
            review(
                "jarre-oxygene",
                "Jean-Michel Jarre",
                "Oxygène",
                1976,
                &["Electronic"],
            ),
            review(
                "eno-apollo",
                "Brian Eno",
                "Apollo",
                1983,
                &["Ambient", "Electronic"],
            ),
            review("eno-on-land", "Brian Eno", "On Land", 1982, &["Ambient"]),
        ],
    );
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(
        node(&output, "Search albums").is_none(),
        "The window starts closed."
    );
    let output = click(&ctx, &mut app, "Album Review");
    assert!(app.show_album_review);
    assert!(has_text(&output, "3 / 3 albums"));
    for control in [
        "Search albums",
        "Sort albums",
        "Artist filter: Brian Eno",
        "Genre filter: Ambient",
        "Open review: Jean-Michel Jarre — Oxygène",
        "Clear cover cache",
        "Credits and rights",
    ] {
        assert!(node(&output, control).is_some(), "Missing {control}");
    }
    // Artists combine by union; their chips remove them one at a time.
    click(&ctx, &mut app, "Artist filter: Brian Eno");
    let output = click(&ctx, &mut app, "Artist filter: Jean-Michel Jarre");
    assert!(has_text(&output, "3 / 3 albums"));
    let output = click(&ctx, &mut app, "Remove filter: Jean-Michel Jarre");
    assert!(has_text(&output, "2 / 3 albums"));
    assert!(node(&output, "Open review: Jean-Michel Jarre — Oxygène").is_none());
    // Genres combine by intersection, and the Electronic tag still has a match.
    let output = click(&ctx, &mut app, "Genre filter: Ambient");
    assert!(has_text(&output, "2 / 3 albums"));
    let electronic = node(&output, "Genre filter: Electronic").unwrap();
    assert!(!electronic.is_disabled());
    let output = click(&ctx, &mut app, "Genre filter: Electronic");
    assert!(has_text(&output, "1 / 3 albums"));
    // A search for an accented title without its accent finds nothing here,
    // and the empty state offers Clear filters.
    app.album_review.filter.search = "oxygene".into();
    let output = frame(&ctx, &mut app, Vec::new());
    let output = if has_text(&output, "No results") {
        output
    } else {
        frame(&ctx, &mut app, Vec::new())
    };
    assert!(has_text(
        &output,
        "No results for the current search and filters."
    ));
    let output = click(&ctx, &mut app, "Clear filters");
    assert!(app.album_review.filter.is_empty());
    assert!(has_text(&output, "3 / 3 albums"));
    app.album_review.filter.search = "OXYGENE".into();
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(has_text(&output, "1 / 3 albums"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_genre_that_would_leave_no_album_is_disabled_and_extra_genres_fold_away() {
    let genres = [
        "Ambient",
        "Drone",
        "Electronic",
        "Krautrock",
        "New Age",
        "Minimal",
        "Techno",
    ];
    let mut reviews = vec![review("a-a", "A", "A", 1990, &genres)];
    reviews.push(review("b-b", "B", "B", 1991, &["Ambient"]));
    let (ctx, mut app, root) = fixture("genres", &reviews);
    app.show_album_review = true;
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(
        node(&output, "Genre filter: Techno").is_none(),
        "Only the top six show."
    );
    let output = click(&ctx, &mut app, "+1 more");
    assert!(node(&output, "Genre filter: Techno").is_some());
    let output = click(&ctx, &mut app, "Show fewer");
    assert!(node(&output, "Genre filter: Techno").is_none());
    let output = click(&ctx, &mut app, "Artist filter: B");
    assert!(node(&output, "Genre filter: Drone").unwrap().is_disabled());
    assert!(
        !node(&output, "Genre filter: Ambient")
            .unwrap()
            .is_disabled()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_album_page_shows_metadata_links_overview_and_each_timeline() {
    let (ctx, mut app, root) = fixture(
        "page",
        &[review(
            "jarre-oxygene",
            "Jean-Michel Jarre",
            "Oxygène",
            1976,
            &["Electronic"],
        )],
    );
    app.show_album_review = true;
    frame(&ctx, &mut app, Vec::new());
    let output = click(&ctx, &mut app, "Open review: Jean-Michel Jarre — Oxygène");
    assert_eq!(app.album_review.page.as_deref(), Some("jarre-oxygene"));
    for text in [
        "Released",
        "1976",
        "Label",
        "Producer",
        "Reference album",
        "First paragraph of the overview.",
        "Second paragraph.",
        "Energy: peak",
        "The lead enters.",
        "Filtered noise under it.",
        "Album role: Carries the theme.",
    ] {
        assert!(has_text(&output, text), "Missing {text}");
    }
    for control in [
        "Listen on YouTube",
        "Cover source",
        "Display cover art",
        "Timeline of track 1: Part 1",
        "Timeline of track 2: Part 2",
    ] {
        assert!(node(&output, control).is_some(), "Missing {control}");
    }
    // A link that fails gives its reason and keeps the placeholder.
    click(&ctx, &mut app, "Display cover art");
    let mut output = frame(&ctx, &mut app, Vec::new());
    for _ in 0..2000 {
        if has_text(&output, "No cover:") {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
        output = frame(&ctx, &mut app, Vec::new());
    }
    assert!(has_text(&output, "No cover: The download failed"));
    assert!(node(&output, "Display cover art").is_some());
    let output = click(&ctx, &mut app, "← All albums");
    assert_eq!(app.album_review.page, None);
    assert!(has_text(&output, "1 / 1 album"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_folder_and_an_empty_folder_have_explicit_states() {
    let (ctx, mut app, root) = fixture("empty", &[]);
    app.show_album_review = true;
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(has_text(&output, "No albums available."));
    app.album_review
        .use_folder(None, CoverCache::new(root.join("covers"), 1 << 20));
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(has_text(&output, "No reviews folder"));
    assert!(node(&output, "Search albums").is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn genre_chips_wrap_inside_supported_and_narrower_windows() {
    let bundled = mixjam_techno::application::album_review::Shelf::open_default();
    assert!(bundled.folder().is_some());
    for (width, height) in [(920, 720), (960, 720), (1024, 768), (1440, 1000)] {
        for filtered in [false, true] {
            let (ctx, mut app, root) = fixture(&format!("genre-wrap-{width}-{filtered}"), &[]);
            ctx.style_mut(|style| {
                style.animation_time = 0.0;
                style.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.album_review.use_folder(
                bundled.folder().map(Path::to_path_buf),
                CoverCache::new(root.join("covers"), 1 << 20),
            );
            app.show_album_review = true;
            app.show_libraries = false;
            if filtered {
                app.album_review.filter.search = "eno".into();
                app.album_review.filter.genres.insert("Ambient".into());
            }
            let mut output = None;
            for _ in 0..6 {
                output = Some(ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(width as f32, height as f32),
                        )),
                        focused: true,
                        ..Default::default()
                    },
                    |ctx| app.draw(ctx),
                ));
            }
            let output = output.unwrap();
            let window = node(&output, "ALBUM REVIEW").unwrap().bounds().unwrap();
            let genres: Vec<_> = nodes(&output)
                .into_iter()
                .filter_map(|(_, node)| {
                    node.label()
                        .is_some_and(|label| label.starts_with("Genre filter:"))
                        .then_some(node)
                })
                .collect();
            assert_eq!(genres.len(), 6);
            for genre in genres {
                let rect = genre.bounds().unwrap();
                assert!(
                    rect.x0 >= window.x0 + 12.0 && rect.x1 <= window.x1 - 12.0,
                    "{} at {rect:?} exceeds window {window:?} at {width}x{height}, filtered={filtered}",
                    genre.label().unwrap()
                );
            }
            let mut checked = 0;
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "New Age (8)"
                {
                    checked += 1;
                    let right = text.pos.x + text.galley.size().x;
                    assert!(
                        right <= shape.clip_rect.right() + 1.0,
                        "Genre text ends at {right}, beyond clip {:?}, at {width}x{height}, filtered={filtered}",
                        shape.clip_rect
                    );
                }
            }
            assert_eq!(checked, 1, "The bundled New Age filter must be painted.");
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn opening_an_album_from_a_scrolled_grid_starts_at_the_top_of_its_page() {
    let reviews: Vec<_> = (0..40)
        .map(|index| {
            review(
                &format!("album-{index}"),
                "Artist",
                &format!("Album {index}"),
                1990,
                &["Ambient"],
            )
        })
        .collect();
    let (ctx, mut app, root) = fixture("page-scroll", &reviews);
    app.show_libraries = false;
    app.show_album_review = true;
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    let search_top = node(&output, "Search albums").unwrap().bounds().unwrap().y0;
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(Pos2::new(900.0, 600.0))],
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -500.0),
            modifiers: egui::Modifiers::NONE,
        }],
    );
    for _ in 0..20 {
        frame(&ctx, &mut app, Vec::new());
    }
    let output = frame(&ctx, &mut app, Vec::new());
    let scrolled_top = node(&output, "Search albums").unwrap().bounds().unwrap().y0;
    assert!(
        scrolled_top < search_top - 100.0,
        "The grid must first scroll."
    );
    let output = click(&ctx, &mut app, "Open review: Artist — Album 20");
    let back_top = node(&output, "← All albums").unwrap().bounds().unwrap().y0;
    assert!(
        (back_top - search_top).abs() < 5.0,
        "The album page must open at its top: search={search_top}, back={back_top}."
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_reviews_folder_can_be_rediscovered_without_restarting() {
    let (ctx, mut app, root) = fixture("rediscover", &[]);
    app.album_review
        .use_folder(None, CoverCache::new(root.join("covers"), 1 << 20));
    app.show_album_review = true;
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(has_text(&output, "No reviews folder"));
    let expected = mixjam_techno::application::album_review::Shelf::open_default();
    assert!(
        expected.folder().is_some(),
        "The bundled reviews are the restored folder."
    );
    let output = click(&ctx, &mut app, "Reload reviews");
    assert_eq!(app.album_review.shelf().folder(), expected.folder());
    assert!(!has_text(&output, "No reviews folder"));
    assert!(node(&output, "Search albums").is_some());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn clear_cover_cache_removes_the_downloaded_covers() {
    let (ctx, mut app, root) = fixture("cache", &[review("a-a", "A", "A", 1990, &["Ambient"])]);
    let cache = CoverCache::new(root.join("covers"), 1 << 20);
    cache
        .store("a-a", include_bytes!("../../assets/rail/album-review.png"))
        .unwrap();
    app.show_album_review = true;
    frame(&ctx, &mut app, Vec::new());
    let output = frame(&ctx, &mut app, Vec::new());
    assert!(has_text(&output, "Cover cache: 1 cover"));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let output = frame(&ctx, &mut app, Vec::new());
        if !node(&output, "Clear cover cache").unwrap().is_disabled() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Cover loading did not finish."
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let output = click(&ctx, &mut app, "Clear cover cache");
    assert!(has_text(&output, "Removed 1 cover."));
    assert!(has_text(&output, "Cover cache: 0 covers"));
    assert_eq!(cache.usage(), (0, 0));
    let output = click(&ctx, &mut app, "Credits and rights");
    assert!(has_text(&output, "fair use"));
    // A review added beside the application shows after Reload reviews.
    let added = review("b-b", "B", "B", 1991, &["Ambient"]);
    std::fs::write(root.join("reviews").join("b-b.json"), added.to_file_text()).unwrap();
    let output = click(&ctx, &mut app, "Reload reviews");
    assert!(has_text(&output, "Read 2 reviews again."));
    assert!(has_text(&output, "2 / 2 albums"));
    std::fs::remove_dir_all(root).unwrap();
}
