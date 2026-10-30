use super::collection::{self, Filter, Sort, TOP_GENRES};
use super::*;

fn event(timestamp: &str, section: &str) -> Event {
    Event {
        timestamp: timestamp.into(),
        section: section.into(),
        description: format!("{section} begins."),
        detail: None,
    }
}

fn track(num: u32, duration: &str, energy: Energy, tags: &[&str]) -> Track {
    Track {
        num,
        title: format!("Track {num}"),
        duration: duration.into(),
        energy,
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        role: "Album role: Opens the record.".into(),
        events: vec![event("0:00", "Intro"), event("1:00", "Theme")],
        local_audio: None,
        song_file: None,
    }
}

fn review(id: &str, artist: &str, title: &str, year: i32, genres: &[&str]) -> Review {
    Review {
        id: id.into(),
        kind: Kind::Reference,
        artist: artist.into(),
        title: title.into(),
        year,
        label: "Label".into(),
        producer: "Producer".into(),
        genre: genres.join(" / "),
        genre_tags: genres.iter().map(|tag| (*tag).to_owned()).collect(),
        runtime: "9:00".into(),
        is_soundtrack: None,
        cover_url: None,
        cover_source: None,
        youtube_url: None,
        bandcamp_url: None,
        local_audio: None,
        song_file: None,
        overview: "An analytical overview.".into(),
        tracks: vec![
            track(1, "4:00", Energy::Low, &["Ambient", "Drone"]),
            track(2, "5:00", Energy::Peak, &["Arpeggio", "Sequencer"]),
        ],
    }
}

fn folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("mixjam-album-review-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn a_complete_review_passes_and_round_trips_through_its_file_text() {
    let review = review("artist-album", "Artist", "Album", 1999, &["Electronic"]);
    assert_eq!(review.issues(), Vec::<String>::new());
    let text = review.to_file_text();
    assert!(text.ends_with("}\n"));
    assert!(text.contains("\"genreTags\""));
    assert!(text.contains("\"kind\": \"reference\""));
    assert!(!text.contains("coverUrl"), "Absent links are left out.");
    assert_eq!(parse(&text).unwrap(), review);
}

#[test]
fn the_shape_rejects_unknown_fields_and_the_dropped_streaming_links() {
    let text = review("artist-album", "Artist", "Album", 1999, &["Electronic"]).to_file_text();
    for field in ["spotifyUrl", "audioStreamUrl", "videoStreamUrl", "rating"] {
        let changed = text.replacen('{', &format!("{{\n  \"{field}\": \"x\","), 1);
        let error = parse(&changed).unwrap_err();
        assert!(error.contains(field), "{field}: {error}");
    }
    let wrong_energy = text.replace("\"low\"", "\"loud\"");
    assert!(parse(&wrong_energy).is_err());
    let missing = text.replace("\"label\": \"Label\",", "");
    assert!(parse(&missing).unwrap_err().contains("label"));
}

#[test]
fn the_rules_name_each_problem() {
    let mut bad = review("Bad--Id", " ", "Album", 99, &[]);
    bad.label = String::new();
    bad.runtime = "1:5".into();
    bad.is_soundtrack = Some(false);
    bad.overview = format!("{} More.", review::SCAFFOLD_OVERVIEW);
    bad.cover_url = Some("covers/bad-id.jpg".into());
    bad.youtube_url = Some("http://www.youtube.com/watch?v=1".into());
    bad.bandcamp_url = Some("https://example.com/album".into());
    bad.local_audio = Some("D:/masters/song.wav".into());
    let first = &mut bad.tracks[0];
    first.tags = vec!["Scaffold".into()];
    first.role = "Opens the record.".into();
    first.duration = "4:60".into();
    first.events = vec![
        event("0:10", "Intro"),
        event("0:05", "Back"),
        event("9:99", "Late"),
    ];
    first.events[0].detail = Some(" ".into());
    first.song_file = Some("song.txt".into());
    let second = &mut bad.tracks[1];
    second.events = vec![event("0:00", "Only")];
    second.events.push(event("5:00", "At the end"));
    let issues = bad.issues();
    let has = |text: &str| {
        assert!(
            issues.iter().any(|issue| issue.contains(text)),
            "Missing \"{text}\" in {issues:#?}"
        )
    };
    has("id \"Bad--Id\"");
    has("artist is required");
    has("label is required");
    has("year 99");
    has("genreTags must contain 1 to 9");
    has("isSoundtrack must be true");
    has("scaffold placeholder text");
    has("coverUrl must be an absolute http or https address");
    has("youtubeUrl must be an https address");
    has("bandcampUrl must be an https address on bandcamp.com");
    has("localAudio is only for a review of kind own");
    has("runtime \"1:5\" must use");
    has("tracks[0].tags must contain 2 to 5");
    has("tracks[0].tags still have scaffold placeholder tags");
    has("tracks[0].role must start with \"Album role: \"");
    has("tracks[0].duration \"4:60\"");
    has("tracks[0].events[1].timestamp 0:05 must be after");
    has("tracks[0].events[2].timestamp \"9:99\"");
    has("tracks[0].events[0].detail must not be empty");
    has("tracks[0].songFile is only for a review of kind own");
    has("tracks[0].songFile must name a .mixjam or .mix file");
    has("tracks[1].events[1].timestamp 5:00 must be before the track end (5:00)");
}

#[test]
fn time_formats_and_the_runtime_tolerance_follow_the_review_rules() {
    assert_eq!(clock_seconds("0:00"), Some(0));
    assert_eq!(clock_seconds("12:34"), Some(754));
    for bad in ["1:5", "123:00", "1:60", ":30", "1:3a", "-1:30", "1:30:00"] {
        assert_eq!(clock_seconds(bad), None, "{bad}");
    }
    assert_eq!(runtime_seconds("59:59"), Some(3599));
    assert_eq!(runtime_seconds("1:05:33"), Some(3933));
    for bad in ["60:00", "01:05:33", "1:5:33", "1:05:3"] {
        assert_eq!(runtime_seconds(bad), None, "{bad}");
    }
    let mut album = review("a-b", "A", "B", 2000, &["Ambient"]);
    album.runtime = "9:30".into();
    assert!(
        album.issues().is_empty(),
        "30 seconds is within the tolerance"
    );
    album.runtime = "9:31".into();
    assert!(album.issues()[0].contains("differs from the track sum by 31 s"));
    assert_eq!(album.track_seconds(), Some(540));
}

#[test]
fn runtime_rejects_invalid_hour_fields_and_arithmetic_overflow() {
    for bad in ["1:60:00", "1:99:59", "1193047:00:00", "4294967295:00:00"] {
        assert_eq!(runtime_seconds(bad), None, "{bad}");
    }
    assert_eq!(runtime_seconds("1193046:28:15"), Some(u32::MAX));
    assert_eq!(runtime_seconds("1193046:28:16"), None);
}

#[test]
fn duplicate_track_numbers_are_rejected_before_saving() {
    let root = folder("duplicate-track-numbers");
    let mut shelf = Shelf::open(Some(root.clone()));
    let mut album = review("artist-album", "Artist", "Album", 1999, &["Electronic"]);
    album.tracks[1].num = album.tracks[0].num;
    let error = parse(&album.to_file_text()).unwrap_err();
    assert!(
        error.contains("tracks[1].num 1 repeats an earlier track number"),
        "{error}"
    );
    assert!(shelf.add(album.clone()).is_err());
    assert!(shelf.reviews().is_empty());
    assert!(!root.join("artist-album.json").exists());
    album.tracks[1].num = 2;
    shelf.add(album).unwrap();
    assert_eq!(shelf.reviews().len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn listening_links_reject_ambiguous_authorities_and_invalid_ports() {
    let mut album = review("artist-album", "Artist", "Album", 1999, &["Electronic"]);
    for bad in [
        "https://evil.example\\@www.youtube.com/watch?v=x",
        "https://user@www.youtube.com/watch?v=x",
        "https://www.youtube.com:invalid/watch?v=x",
        "https://www.youtube.com:99999/watch?v=x",
        "https://www.youtube.com/\u{0000}watch?v=x",
    ] {
        album.youtube_url = Some(bad.into());
        assert!(!album.issues().is_empty(), "{bad:?}");
    }
    album.youtube_url = Some("https://WWW.YOUTUBE.COM:443/watch?v=x".into());
    assert!(album.issues().is_empty());
}

#[test]
fn adding_preserves_invalid_files_and_files_created_after_loading() {
    let root = folder("add-preserves-files");
    let path = root.join("artist-album.json");
    let album = review("artist-album", "Artist", "Album", 1999, &["Electronic"]);
    std::fs::write(&path, "{unfinished review").unwrap();
    let mut shelf = Shelf::open(Some(root.clone()));
    assert_eq!(shelf.problems().len(), 1);
    assert!(shelf.add(album.clone()).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{unfinished review"
    );
    std::fs::remove_file(&path).unwrap();
    let mut shelf = Shelf::open(Some(root.clone()));
    std::fs::write(&path, album.to_file_text()).unwrap();
    let mut replacement = album.clone();
    replacement.title = "Different album".into();
    assert!(shelf.add(replacement).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        album.to_file_text()
    );
    assert!(shelf.reviews().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn own_work_takes_local_listening_links_and_a_song_file() {
    let mut own = review("eklipse-reality", "eklipse", "Reality", 2026, &["Techno"]);
    own.kind = Kind::Own;
    own.local_audio = Some("D:/masters/reality.wav".into());
    own.song_file = Some("D:/songs/reality.mixjam".into());
    own.bandcamp_url = Some("https://eklipse-music.bandcamp.com/album/nostalgia".into());
    own.tracks[0].local_audio = Some("D:/masters/01.WAV".into());
    own.tracks[0].song_file = Some("D:/songs/01.mix".into());
    assert_eq!(own.issues(), Vec::<String>::new());
    let text = own.to_file_text();
    for key in [
        "\"kind\": \"own\"",
        "\"localAudio\"",
        "\"songFile\"",
        "\"bandcampUrl\"",
    ] {
        assert!(text.contains(key), "{key}");
    }
    own.local_audio = Some("D:/masters/reality.mp3".into());
    assert!(own.issues()[0].contains("localAudio must name a .wav file"));
}

#[test]
fn a_missing_folder_is_explicit_and_bad_files_are_reported_by_name() {
    let none = Shelf::open(None);
    assert!(none.folder().is_none());
    assert!(none.reviews().is_empty());
    let gone = Shelf::open(Some(std::env::temp_dir().join("mixjam-no-such-reviews")));
    assert_eq!(gone.problems().len(), 1);
    let root = folder("load");
    let good = review("artist-album", "Artist", "Album", 1999, &["Electronic"]);
    std::fs::write(root.join("artist-album.json"), good.to_file_text()).unwrap();
    std::fs::write(root.join("renamed.json"), good.to_file_text()).unwrap();
    std::fs::write(root.join("broken.json"), "{").unwrap();
    std::fs::write(root.join("notes.txt"), "not a review").unwrap();
    let shelf = Shelf::open(Some(root.clone()));
    assert_eq!(shelf.reviews(), &[good]);
    let names: Vec<_> = shelf
        .problems()
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["broken.json", "renamed.json"]);
    assert!(
        shelf.problems()[1]
            .1
            .contains("does not match the file name")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn saving_validates_first_and_add_and_update_keep_their_meaning() {
    let root = folder("save");
    let mut shelf = Shelf::open(Some(root.clone()));
    let revision = shelf.revision();
    let mut album = review("artist-album", "Artist", "Album", 1999, &["Electronic"]);
    shelf.add(album.clone()).unwrap();
    assert!(shelf.revision() > revision);
    assert_eq!(
        std::fs::read_to_string(root.join("artist-album.json")).unwrap(),
        album.to_file_text()
    );
    assert!(
        shelf
            .add(album.clone())
            .unwrap_err()
            .contains("update_review")
    );
    album.title = "Album (Remastered)".into();
    shelf.update(album.clone()).unwrap();
    assert_eq!(
        shelf.review("artist-album").unwrap().title,
        "Album (Remastered)"
    );
    let mut other = album.clone();
    other.id = "other-album".into();
    assert!(
        shelf
            .update(other.clone())
            .unwrap_err()
            .contains("add_review")
    );
    // A review that breaks a rule is not written.
    other.tracks[0].role = "No prefix".into();
    let error = shelf.add(other).unwrap_err();
    assert!(error.contains("breaks 1 rule"), "{error}");
    assert!(!root.join("other-album.json").exists());
    assert_eq!(Shelf::open(Some(root.clone())).reviews(), &[album]);
    assert!(
        Shelf::open(None)
            .add(review("a-b", "A", "B", 2000, &["X"]))
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn agents_can_query_reviews_and_tracks_by_tag_energy_and_artist() {
    let root = folder("query");
    let mut shelf = Shelf::open(Some(root.clone()));
    shelf
        .add(review(
            "jarre-oxygene",
            "Jean-Michel Jarre",
            "Oxygène",
            1976,
            &["Electronic"],
        ))
        .unwrap();
    shelf
        .add(review(
            "eno-apollo",
            "Brian Eno",
            "Apollo",
            1983,
            &["Ambient", "New Age"],
        ))
        .unwrap();
    let mut own = review("eklipse-reality", "eklipse", "Reality", 2026, &["Techno"]);
    own.kind = Kind::Own;
    own.tracks[1].tags = vec!["Acid".into(), "Four-on-the-floor".into()];
    shelf.add(own).unwrap();

    let ids = |reviews: Vec<&Review>| reviews.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    let query = |query: Query| ids(shelf.find(&query));
    assert_eq!(query(Query::default()).len(), 3);
    assert_eq!(
        query(Query {
            artist: Some("jean michel".into()),
            ..Default::default()
        }),
        Vec::<String>::new(),
        "The artist filter looks for that part of the name."
    );
    assert_eq!(
        query(Query {
            artist: Some("JEAN-MICHEL".into()),
            ..Default::default()
        }),
        ["jarre-oxygene"]
    );
    assert_eq!(
        query(Query {
            search: Some("oxygene".into()),
            ..Default::default()
        }),
        ["jarre-oxygene"]
    );
    assert_eq!(
        query(Query {
            tag: Some("new-age".into()),
            ..Default::default()
        }),
        ["eno-apollo"]
    );
    assert_eq!(
        query(Query {
            kind: Some(Kind::Own),
            ..Default::default()
        }),
        ["eklipse-reality"]
    );
    // A track filter matches a review with at least one such track.
    assert_eq!(
        query(Query {
            tag: Some("four on the floor".into()),
            ..Default::default()
        }),
        ["eklipse-reality"]
    );

    let tracks = shelf.tracks(&Query {
        energy: Some(Energy::Peak),
        tag: Some("arpeggio".into()),
        ..Default::default()
    });
    let found: Vec<_> = tracks
        .iter()
        .map(|m| (m.review.as_str(), m.track.num))
        .collect();
    assert_eq!(found, [("eno-apollo", 2), ("jarre-oxygene", 2)]);
    assert_eq!(
        tracks[0].track.events.len(),
        2,
        "A match carries the timeline."
    );
    // A genre tag of the album matches each of its tracks.
    let ambient = shelf.tracks(&Query {
        tag: Some("ambient".into()),
        ..Default::default()
    });
    assert_eq!(
        ambient.len(),
        4,
        "Both Apollo tracks, and the first track of each other review"
    );
    let one = shelf.tracks(&Query {
        id: Some("eno-apollo".into()),
        ..Default::default()
    });
    assert_eq!(one.len(), 2);
    assert_eq!(Energy::parse("PEAK").unwrap(), Energy::Peak);
    assert!(Energy::parse("loud").is_err());
    assert_eq!(Kind::parse("own").unwrap(), Kind::Own);
    std::fs::remove_dir_all(root).unwrap();
}

fn collection_fixture() -> Vec<Review> {
    let mut reviews = vec![
        review(
            "a",
            "Jean-Michel Jarre",
            "Oxygène",
            1976,
            &["Electronic", "Ambient"],
        ),
        review("b", "Brian Eno", "Apollo", 1983, &["Ambient", "New Age"]),
        review("c", "Brian Eno", "Music for Films", 1978, &["Ambient"]),
        review("d", "Vangelis", "Voices", 1995, &["Electronic", "New-Age"]),
        review(
            "e",
            "Aphex Twin",
            "Selected Ambient Works",
            1976,
            &["Ambient", "Techno"],
        ),
        review(
            "f",
            "Vangelis",
            "Antarctica",
            1983,
            &["Soundtrack", "New Age"],
        ),
    ];
    reviews[3]
        .tracks
        .push(track(3, "0:30", Energy::Mid, &["Choir", "Pad"]));
    reviews[3].runtime = "9:30".into();
    reviews
}

#[test]
fn search_ignores_accents_and_looks_in_title_artist_and_genre_tags() {
    let reviews = collection_fixture();
    let found = |search: &str| {
        collection::visible(
            &reviews,
            &Filter {
                search: search.into(),
                ..Default::default()
            },
            Sort::Title,
        )
        .into_iter()
        .map(|index| reviews[index].id.as_str())
        .collect::<Vec<_>>()
    };
    assert_eq!(found("oxygene"), ["a"]);
    assert_eq!(found("OXYGÈNE"), ["a"]);
    assert_eq!(found("eno"), ["b", "c"]);
    assert_eq!(found("techno"), ["e"]);
    assert_eq!(found("  "), ["f", "b", "c", "a", "e", "d"]);
    assert!(found("no such album").is_empty());
}

#[test]
fn artists_combine_by_union_and_genres_by_intersection() {
    let reviews = collection_fixture();
    let ids = |filter: &Filter| {
        collection::visible(&reviews, filter, Sort::YearAscending)
            .into_iter()
            .map(|index| reviews[index].id.as_str())
            .collect::<Vec<_>>()
    };
    let mut filter = Filter::default();
    filter.artists.insert("Brian Eno".into());
    filter.artists.insert("Vangelis".into());
    assert_eq!(ids(&filter), ["c", "b", "f", "d"]);
    filter.genres.insert("New Age".into());
    assert_eq!(ids(&filter), ["b", "f", "d"], "New-Age is the same tag");
    filter.genres.insert("Ambient".into());
    assert_eq!(ids(&filter), ["b"]);
    // A tag that would leave no album is disabled; one that keeps some is not.
    assert!(!filter.genre_has_results(&reviews, "Techno"));
    assert!(filter.genre_has_results(&reviews, "Ambient"));
    filter.clear();
    assert!(filter.is_empty());
    assert_eq!(ids(&filter).len(), 6);
}

#[test]
fn the_genre_row_counts_tags_once_and_names_variants_by_the_common_spelling() {
    let reviews = collection_fixture();
    let counts = collection::genre_counts(&reviews);
    assert_eq!(
        counts,
        [
            ("Ambient".to_string(), 4),
            ("New Age".to_string(), 3),
            ("Electronic".to_string(), 2),
            ("Soundtrack".to_string(), 1),
            ("Techno".to_string(), 1),
        ]
    );
    assert_eq!(TOP_GENRES, 6);
    assert_eq!(collection::artists(&reviews)[0], "Aphex Twin");
}

#[test]
fn each_sort_breaks_ties_by_year_then_artist_then_title() {
    let reviews = collection_fixture();
    let order = |sort: Sort| {
        collection::visible(&reviews, &Filter::default(), sort)
            .into_iter()
            .map(|index| reviews[index].id.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(order(Sort::default()), ["e", "a", "c", "b", "f", "d"]);
    assert_eq!(order(Sort::YearDescending), ["d", "b", "f", "c", "e", "a"]);
    assert_eq!(order(Sort::Artist), ["e", "c", "b", "a", "f", "d"]);
    assert_eq!(order(Sort::Title), ["f", "b", "c", "a", "e", "d"]);
    assert_eq!(order(Sort::MostTracks), ["d", "e", "a", "c", "b", "f"]);
    let labels: Vec<_> = Sort::ALL.iter().map(|sort| sort.label()).collect();
    assert_eq!(
        labels,
        ["Year ↑", "Year ↓", "Artist A–Z", "Title A–Z", "Most tracks"]
    );
    for sort in Sort::ALL {
        assert_eq!(Sort::parse(sort.key()).unwrap(), sort);
    }
    assert_eq!(collection::count_label(3, 61), "3 / 61 albums");
    assert_eq!(collection::count_label(1, 1), "1 / 1 album");
    assert_eq!(collection::count_label(0, 0), "0 / 0 albums");
}

#[test]
fn every_shipped_review_obeys_the_rules() {
    let folder = crate::runtime_data::find(FOLDER).expect("The repository ships its reviews.");
    let shelf = Shelf::open(Some(folder));
    assert_eq!(shelf.problems(), &[] as &[(String, String)]);
    assert!(shelf.reviews().len() >= 61);
    for review in shelf.reviews() {
        assert!(
            review
                .cover_url
                .as_deref()
                .is_none_or(|url| !url.contains("utm_")),
            "{} keeps tracking parameters in its cover link",
            review.id
        );
    }
}
