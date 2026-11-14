//! Sounds panel and browser: sound rows, tooltips, sorting, previews and drags.

use super::*;

#[test]
fn sound_tooltip_retains_identity_timing_audio_metadata_and_full_unicode_name() {
    let (context, mut app) = fixture();
    let mut sample = app.session.library().samples[0].clone();
    sample.id = Product::D3.global_sample_id(42).unwrap();
    sample.code = "ORIGINAL42".into();
    sample.name = "Știință și ritm — atmosphère originală pentru o noapte foarte lungă".into();
    sample.group_name = "Sphere".into();
    sample.bars = 4;
    sample.source_bpm = 132.0;
    sample.frame_count = 60_000;
    sample.sample_rate = 48_000;
    sample.channels = 2;
    sample.wav_path = Some(PathBuf::from(r"\\?\C:\Saved sounds\Știință și ritm.wav"));
    let mut project = app.session.project().clone();
    for clip in &mut project.clips {
        clip.sample_id = sample.id;
    }
    app.session =
        Session::headless(Arc::new(Library::fixture(vec![sample.clone()])), project).unwrap();
    app.set_layout(|layout| layout.sounds = true);
    context.enable_accesskit();
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    // The Sounds panel's row, which elides the long name it is named by.
    let tile = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.role() == egui::accesskit::Role::Button
                && node.label() == Some(sample.name.as_str())
        })
        .filter_map(|(_, node)| node.bounds())
        .map(|b| {
            Rect::from_min_max(
                Pos2::new(b.x0 as f32, b.y0 as f32),
                Pos2::new(b.x1 as f32, b.y1 as f32),
            )
        })
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .expect("The Sounds panel must list the sound.");
    context.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(tile.min + Vec2::new(8.0, 8.0))],
        egui::Modifiers::NONE,
    );
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let tooltip = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text.contains("ORIGINAL42") => {
                Some(&text.galley.job.text)
            }
            _ => None,
        })
        .expect("Hovering a sound must expose its metadata.");
    for value in [
        sample.name.as_str(),
        "D3",
        "Sphere",
        "ORIGINAL42",
        "400042",
        "Native sample ID: 42",
        "4 bars",
        "132 BPM",
        "48000 Hz",
        "2 channels",
        "60000 frames",
        "1.250 s",
        r"C:\Saved sounds\Știință și ritm.wav",
    ] {
        assert!(tooltip.contains(value), "Missing sound metadata: {value}");
    }
    assert!(!tooltip.contains(r"\\?\"));
    assert_eq!(app.session.library().samples[0].wav_path, sample.wav_path);

    sample.wav_path = Some(PathBuf::from(r"\\?\UNC\studio\sounds\Știință.wav"));
    let details = sample_details(&sample, Some(Product::D3));
    assert!(details.contains(r"\\studio\sounds\Știință.wav"));
    assert!(!details.contains(r"\\?\"));
}

#[test]
fn sound_rows_tint_the_full_surface_and_keep_metadata_and_selection_visible() {
    let (_, app) = fixture();
    let sample = &app.session.library().samples[0];
    let category = group_color(&sample.group_name);
    let tint = Color32::from_rgba_unmultiplied(category.r(), category.g(), category.b(), 51);
    for selected in [false, true] {
        let context = egui::Context::default();
        configure_style(&context);
        let mut tile = Rect::NOTHING;
        let output = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                tile = sounds::sound_row(
                    ui,
                    sample,
                    Some(Product::T2),
                    &mixjam_techno::application::PeakState::Pending,
                    selected,
                    226.0,
                )
                .rect;
            });
        });
        let tint_index = output
            .shapes
            .iter()
            .position(|shape| {
                matches!(
                    &shape.shape,
                    egui::Shape::Rect(rect) if rect.rect == tile && rect.fill == tint
                )
            })
            .expect("The category tint must cover the full sound row at 20% opacity.");
        assert!(
            !output.shapes.iter().any(|shape| {
                matches!(
                    &shape.shape,
                    egui::Shape::Circle(circle) if tile.contains(circle.center)
                )
            }),
            "Sound rows must not carry a category dot."
        );
        let metadata = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text.contains(&sample.group_name)
                        && text.galley.job.text.contains("1 bar") =>
                {
                    Some((shape, text))
                }
                _ => None,
            })
            .expect("The category and bar count must remain visible.");
        let metadata_bounds = Rect::from_min_size(metadata.1.pos, metadata.1.galley.size());
        assert!(tile.contains_rect(metadata_bounds));
        assert!(metadata.0.clip_rect.contains_rect(metadata_bounds));
        if selected {
            let selection_index = output
                .shapes
                .iter()
                .position(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect)
                    if rect.rect == tile.shrink(0.5)
                        && rect.stroke.color == category
                        && rect.stroke.width > 0.0)
                })
                .expect("A selected sound must retain its category outline.");
            assert!(
                selection_index > tint_index,
                "The tint must not cover selection."
            );
        }
    }
}

#[test]
fn browser_keeps_unknown_categories_and_product_identity_accessible() {
    let (_, mut app) = fixture();
    let first = app.session.library().samples[0].clone();
    let mut t1 = first.clone();
    t1.id = Product::T1.global_sample_id(731).unwrap();
    t1.group_name = "Rap".into();
    let mut kick = first.clone();
    kick.id = Product::D1.global_sample_id(73).unwrap();
    kick.group_name = "Kick".into();
    let mut unnamed = first.clone();
    unnamed.id = Product::H1.global_sample_id(74).unwrap();
    unnamed.group_name.clear();
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![first, t1, kick, unnamed])),
        app.session.project().clone(),
    )
    .unwrap();
    assert_eq!(app.palette_indices(), vec![0, 1, 2, 3]);
    app.sounds.group = Some("kick".into());
    assert_eq!(app.palette_indices(), vec![2]);
    app.sounds.group = None;
    app.sounds.product = Some(Product::T1);
    assert_eq!(app.palette_indices(), vec![1]);
    app.sounds.product = None;
    app.sounds.group = Some(String::new());
    assert_eq!(app.palette_indices(), vec![3]);
}

#[test]
fn palette_sorts_by_name_length_bpm_and_category_and_keeps_library_order_for_ties() {
    let (_, mut app) = fixture();
    let first = app.session.library().samples[0].clone();
    let samples = [
        ("delta", 4, 140.0, "Loop"),
        ("Alpha", 1, 180.0, "voice"),
        ("charlie", 2, 90.0, "Bass"),
        ("bravo", 2, 140.0, "Bass"),
    ]
    .into_iter()
    .zip(1..)
    .map(|((name, bars, source_bpm, group), id)| Sample {
        id,
        name: name.into(),
        bars,
        source_bpm,
        group_name: group.into(),
        ..first.clone()
    })
    .collect();
    app.session = Session::headless(
        Arc::new(Library::fixture(samples)),
        app.session.project().clone(),
    )
    .unwrap();
    for (sort, expected) in [
        (PaletteSort::Library, [0, 1, 2, 3]),
        (PaletteSort::Name, [1, 3, 2, 0]),
        (PaletteSort::Length, [1, 2, 3, 0]),
        (PaletteSort::Bpm, [2, 0, 3, 1]),
        (PaletteSort::Category, [2, 3, 0, 1]),
    ] {
        app.sounds.sort = sort;
        assert_eq!(app.palette_indices(), expected, "{sort:?}");
        assert_eq!(*app.palette_view().0, expected, "{sort:?}");
    }
    // Reversed, ties still keep library order.
    app.sounds.descending = true;
    for (sort, expected) in [
        (PaletteSort::Library, [3, 2, 1, 0]),
        (PaletteSort::Name, [0, 2, 3, 1]),
        (PaletteSort::Length, [0, 2, 3, 1]),
        (PaletteSort::Bpm, [1, 0, 3, 2]),
        (PaletteSort::Category, [1, 0, 2, 3]),
    ] {
        app.sounds.sort = sort;
        assert_eq!(app.palette_indices(), expected, "{sort:?} descending");
        assert_eq!(*app.palette_view().0, expected, "{sort:?} descending");
    }
}

#[test]
fn sort_direction_exposes_its_state_and_reorders_sounds_through_accessibility() {
    let (ctx, mut app) = fixture();
    let first = app.session.library().samples[0].clone();
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![
            Sample {
                name: "Zebra".into(),
                ..first.clone()
            },
            Sample {
                id: 2,
                name: "alpha".into(),
                ..first
            },
        ])),
        app.session.project().clone(),
    )
    .unwrap();
    app.sounds.sort = PaletteSort::Name;
    app.set_layout(|layout| layout.sounds = true);
    let project = app.session.project().clone();
    let unsaved = app.session.unsaved();
    ctx.enable_accesskit();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        accessible_node(&output, "Sort direction").1.value(),
        Some("A to Z")
    );
    assert_eq!(*app.palette_view().0, [1, 0]);
    for (value, order) in [("Z to A", [0, 1]), ("A to Z", [1, 0])] {
        let button = accessible_node(&output, "Sort direction").0;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Click,
                    target: button,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
        );
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            accessible_node(&output, "Sort direction").1.value(),
            Some(value)
        );
        assert_eq!(*app.palette_view().0, order);
    }
    assert_eq!(app.session.project(), &project);
    assert_eq!(app.session.unsaved(), unsaved);
    assert!(!app.session.has_history_step(false));
    ctx.fonts_mut(|fonts| {
        assert!(fonts.has_glyphs(&FontId::proportional(14.0), "\u{2191}\u{2193}"));
    });
}

#[test]
fn full_catalog_frame_paints_only_visible_sample_rows() {
    let (ctx, mut app) = fixture();
    let base = app.session.library().samples[0].clone();
    let samples = (0..31_620)
        .map(|index| {
            let mut sample = base.clone();
            sample.id = index + 1;
            sample.name = format!("Corpus sample {index}");
            sample.group_name = "Kick".into();
            sample
        })
        .collect();
    app.session = Session::headless(
        Arc::new(Library::fixture(samples)),
        app.session.project().clone(),
    )
    .unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let painted_samples = output
        .shapes
        .iter()
        .filter(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text.starts_with("Corpus")
            )
        })
        .count();
    assert_eq!(app.palette_indices().len(), 31_620);
    assert!(
        painted_samples > 0 && painted_samples < 100,
        "A 31,620-sound catalog must paint the visible rows, not every sample: {painted_samples}"
    );
}

#[test]
fn the_sounds_panel_names_its_empty_list_and_offers_the_way_out() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.set_layout(|layout| layout.sounds = true);
    app.sounds.search = "no such sound".into();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // Headings paint in capitals.
    assert!(shows_text(&output, "NO MATCHING SOUNDS"));
    click_named(&ctx, &mut app, "Clear filters");
    assert!(app.sounds.search.is_empty() && app.sounds.group.is_none());
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!shows_text(&output, "NO MATCHING SOUNDS"));
    app.session =
        Session::headless(Arc::new(Library::fixture(vec![])), Project::default()).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(shows_text(&output, "NO SOUNDS"));
    assert!(has_control(&output, "Open sample libraries"));
}

#[test]
fn a_sound_row_selects_previews_places_and_drags() {
    #[derive(Debug, PartialEq)]
    struct Outcome {
        selected: Option<u32>,
        place_on_click: bool,
        clips: usize,
        dragging: bool,
        previewing: bool,
        status: String,
    }
    let outcome = |app: &MixJamApp| Outcome {
        selected: app.selection.sample,
        place_on_click: app.selection.place_on_click,
        clips: app.session.project().clips.len(),
        dragging: matches!(app.gestures.drag, Some(DragItem::Sample(1))),
        previewing: app.session.is_previewing(),
        status: app.status.clone(),
    };
    let act = |row: bool, action: &str| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        if row {
            click_named(&ctx, &mut app, "Sounds");
        }
        app.selection.sample = None;
        app.selection.place_on_click = false;
        // An empty track, so a placed clip adds to the song.
        app.recording.track = 3;
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(named_count(&output, "good grasp 2"), usize::from(row));
        let at = effects_control_rect(&output, "good grasp 2").center();
        let click = |app: &mut MixJamApp| {
            pointer(&ctx, app, at, true, egui::Modifiers::NONE);
            pointer(&ctx, app, at, false, egui::Modifiers::NONE);
        };
        match action {
            "click" => click(&mut app),
            "double-click" => {
                click(&mut app);
                click(&mut app);
            }
            "enter" => {
                focus_control(&ctx, &mut app, &output, "good grasp 2");
                frame(
                    &ctx,
                    &mut app,
                    vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                    egui::Modifiers::NONE,
                );
            }
            _ => {
                pointer(&ctx, &mut app, at, true, egui::Modifiers::NONE);
                for step in 1..=4 {
                    frame(
                        &ctx,
                        &mut app,
                        vec![egui::Event::PointerMoved(
                            at + Vec2::new(8.0 * step as f32, 0.0),
                        )],
                        egui::Modifiers::NONE,
                    );
                }
            }
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        outcome(&app)
    };
    // A click selects, a double-click selects too, Enter places, and a
    // drag carries.
    assert_eq!(act(true, "click").selected, Some(1));
    assert_eq!(act(true, "double-click").selected, Some(1));
    assert_eq!(act(true, "enter").clips, 3);
    assert!(act(true, "drag").dragging);
}

#[test]
fn the_sounds_panel_asks_peaks_only_for_the_rows_it_shows() {
    let context = egui::Context::default();
    configure_style(&context);
    let samples = (1..=600)
        .map(|id| Sample {
            id,
            code: format!("S{id}"),
            name: format!("sound {id:03}"),
            bars: 1 + id % 4,
            group_name: "Loop".into(),
            offset: 0,
            length: 0,
            frame_count: 75600,
            sample_rate: 44100,
            channels: 1,
            source_bpm: 140.0,
            wav_path: None,
            digest: None,
        })
        .collect();
    let library = Arc::new(Library::fixture(samples));
    let mut app = MixJamApp::from_session(
        Session::headless(library, Project::default()).unwrap(),
        "UI fixture".into(),
        false,
    );
    app.capture.path = None;
    app.session.set_theme_fixture(theme::current());
    app.open_sounds_panel();
    for _ in 0..3 {
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    }
    let asked = app.session.requested_peaks();
    assert!(
        (5..40).contains(&asked),
        "Only the rows on screen ask for peaks, not all 600: {asked}"
    );
}
