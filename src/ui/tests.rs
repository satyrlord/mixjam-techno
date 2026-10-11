#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::{Editor, ImportNotice},
        library::Library,
    };

    fn fixture() -> (egui::Context, MixJamApp) {
        let context = egui::Context::default();
        configure_style(&context);
        let sample = Sample {
            id: 1,
            code: "FIXTURE".into(),
            name: "good grasp 2".into(),
            bars: 1,
            group: 1,
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
        let mut app = MixJamApp::from_session(
            Session::headless(library, editor.into_project()).unwrap(),
            "UI fixture".into(),
            false,
        );
        app.capture_path = None;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        (context, app)
    }

    #[test]
    fn app_version_stays_at_the_bottom_right_with_long_status_and_notices() {
        let (context, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.source_notices.push("Imported source notice".into());
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        app.status = "A long status message with a saved sample path. ".repeat(100);
        for (width, height) in [(1440.0, 1000.0), (1024.0, 768.0), (960.0, 720.0)] {
            let size = Vec2::new(width, height);
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            let bounds = |label: &str| {
                output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(shape.clip_rect.contains_rect(rect), "Clipped {label}: {rect:?} in {:?}", shape.clip_rect);
                        Some(rect)
                    }
                    _ => None,
                }).unwrap_or_else(|| panic!("Missing status label: {label}"))
            };
            let version = bounds(&format!("v{}", mixjam_techno::APP_VERSION));
            // The bottom bar ends before the 64-point tool rail.
            assert!((version.right() - (width - 64.0 - 18.0)).abs() <= 1.0);
            assert!(version.top() >= height - 30.0);
            assert!(version.bottom() <= height);
            assert!(bounds(&app.status).right() < version.left());
            assert!(bounds("Import details").right() < version.left());
        }
    }

    #[test]
    fn version_link_opens_the_repository_through_the_system_browser() {
        let (context, mut app) = fixture();
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let version = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == format!("v{}", mixjam_techno::APP_VERSION) =>
            {
                assert!(text.galley.job.sections.iter().all(|section| section.format.color == ACCENT));
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            _ => None,
        }).expect("The version link must be visible.");
        pointer(&context, &mut app, version.center(), true, egui::Modifiers::NONE);
        let output = frame(&context, &mut app, vec![egui::Event::PointerButton {
            pos: version.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }], egui::Modifiers::NONE);
        assert!(output.platform_output.commands.iter().any(|command| matches!(
            command,
            egui::OutputCommand::OpenUrl(url)
                if url.url == "https://github.com/satyrlord/mixjam-techno" && url.new_tab
        )));
    }

    #[test]
    fn syncopate_titles_are_uppercase_without_changing_saved_text_or_body_font() {
        let (context, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.title = "Știință și ritm".into();
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        app.show_libraries = true;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        for label in [
            "MIXJAM",
            "TECHNO",
            "STIINTA SI RITM",
            "ARRANGEMENT",
            "SOUNDS",
            "SAMPLE LIBRARIES",
        ] {
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => Some(text),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing title: {label}"));
            assert!(text.galley.job.sections.iter().all(|section| {
                section.format.font_id.family == egui::FontFamily::Name("Syncopate Bold".into())
            }));
        }
        let body = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Mixer" => Some(text),
                _ => None,
            })
            .expect("The Mixer action must stay visible.");
        assert!(
            body.galley
                .job
                .sections
                .iter()
                .all(|section| { section.format.font_id.family == egui::FontFamily::Proportional })
        );
        assert_eq!(app.session.project().title, "Știință și ritm");
        assert_eq!(app.session.library().samples[0].name, "good grasp 2");
    }

    #[test]
    fn title_text_transliterates_accents_ligatures_and_combining_marks() {
        assert_eq!(title_text("Șș Şş Țț Ţţ Ââ Ăă Îî"), "SS SS TT TT AA AA II");
        assert_eq!(
            title_text("S\u{326}tiint\u{326}a\u{306} i\u{302}n ra\u{302}ve"),
            "STIINTA IN RAVE"
        );
        assert_eq!(
            title_text("Æther Øresund Łódź Straße"),
            "AETHER ORESUND LODZ STRASSE"
        );
        assert_eq!(title_text("Mix 01 / A-B + 2.5 *"), "MIX 01 / A-B + 2.5 *");
    }

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
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let tile = text_rect(&output, &sample.name);
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
    fn sound_tiles_tint_the_full_surface_and_keep_metadata_and_selection_visible() {
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
                    tile = sample_button(ui, sample, Some(Product::T2), selected).rect;
                });
            });
            let tint_index = output.shapes.iter().position(|shape| {
                matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == tile && rect.fill == tint)
            }).expect("The category tint must cover the full sound tile at 20% opacity.");
            assert!(!output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Circle(circle) if tile.contains(circle.center))
            }), "Sound tiles must not retain a category dot.");
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
    fn help_instructions_wrap_readably_and_bottom_guidance_remains_reachable() {
        const PLACE: &str =
            "Drag to a track, or select a sound and click a track. Up to 64 tracks.";
        let visible = |output: &egui::FullOutput, label: &str, viewport: Rect| {
            output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    let bounds = Rect::from_min_size(text.pos, text.galley.size());
                    viewport.contains_rect(bounds) && shape.clip_rect.contains_rect(bounds)
                }
                _ => false,
            })
        };
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            let (context, mut app) = fixture();
            app.show_help = true;
            for _ in 0..3 {
                frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let mut output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            let instruction = text_rect(&output, PLACE);
            assert!(
                instruction.width() > 200.0,
                "Instruction column is too narrow at {size:?}: {instruction:?}"
            );
            assert!(
                instruction.height() < 80.0,
                "Instruction wraps excessively at {size:?}: {instruction:?}"
            );
            assert!(visible(&output, PLACE, viewport));
            for label in [
                "Mixer: gain, pan, L/R outputs, and master gain. Both outputs off mutes a track.",
                "Tab focuses controls. Arrow keys adjust sliders. Space toggles outputs.",
            ] {
                if !visible(&output, label, viewport) {
                    frame_at_size(
                        &context,
                        &mut app,
                        vec![
                            egui::Event::PointerMoved(instruction.center()),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: Vec2::new(0.0, -10_000.0),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        egui::Modifiers::NONE,
                        size,
                    );
                    for _ in 0..12 {
                        output =
                            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
                    }
                }
                assert!(
                    visible(&output, label, viewport),
                    "Help guidance is unreachable at {size:?}: {label}"
                );
            }
        }
    }

    fn missing_fixture() -> (egui::Context, MixJamApp) {
        let (context, mut app) = fixture();
        let missing = MissingSample::native(
            Product::T2,
            42,
            "Lost lead".into(),
            132.0,
            4 * TICKS_PER_BAR,
            false,
            "The original audio file is absent.".into(),
        )
        .unwrap();
        let mut project = Project::default();
        project.clips.push(Clip {
            id: 1,
            sample_id: missing.id,
            track: 2,
            start_tick: TICKS_PER_BAR,
            length_ticks: 4 * TICKS_PER_BAR,
            offset_frames: 150,
            gain: 0.75,
        });
        project.missing_samples.push(missing);
        app.session = Session::headless(Arc::new(Library::fixture(vec![])), project).unwrap();
        app.show_libraries = false;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        (context, app)
    }

    fn sample_conflicts_fixture(tag: &str) -> (egui::Context, MixJamApp, PathBuf) {
        let (context, mut app) = fixture();
        let dir =
            std::env::temp_dir().join(format!("mixjam-ui-conflicts-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut candidates = Vec::new();
        for (name, value) in [
            ("First matching sample.wav", 0.1),
            ("Other matching sample.wav", 0.3),
        ] {
            let path = dir.join(name);
            Pcm {
                sample_rate: 1000,
                channels: 1,
                samples: vec![value; 2000],
            }
            .write_wav(&path)
            .unwrap();
            candidates.push(std::fs::canonicalize(path).unwrap());
        }
        let mut project = Project::default();
        for (index, id) in [1_000_000, 1_000_001].into_iter().enumerate() {
            project
                .imported_samples
                .push(crate::engine::ImportedSample {
                    id,
                    name: format!("Saved source {}", index + 1),
                    group_name: "Wave".into(),
                    path: candidates[0].clone(),
                    source_bpm: 140.0,
                    source_sample_rate: Some(1000),
                    origin: None,
                });
        }
        for (index, sample_id) in [1_000_000, 1_000_001, 1_000_000].into_iter().enumerate() {
            project.clips.push(Clip {
                id: index as u64 + 1,
                sample_id,
                track: index * 4,
                start_tick: index as u32 * TICKS_PER_BAR,
                length_ticks: TICKS_PER_BAR,
                offset_frames: 50,
                gain: 0.75,
            });
        }
        project
            .sample_conflicts
            .push(crate::engine::SampleConflict {
                reference: r"D:\Old sample folder\Loop\Shared beat.wav".into(),
                sample_ids: vec![1_000_000, 1_000_001],
                candidates,
                selected: 0,
                confirmed: false,
                source_spans: BTreeMap::new(),
            });
        let mut library = Library::fixture(vec![]);
        library.apply_imports(&project.imported_samples).unwrap();
        app.session = Session::headless(Arc::new(library), project).unwrap();
        app.show_libraries = false;
        context.enable_accesskit();
        for _ in 0..3 {
            frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        }
        (context, app, dir)
    }

    #[test]
    fn sample_editor_resets_its_range_when_a_conflict_replaces_the_same_source_id() {
        let (context, mut app, dir) = sample_conflicts_fixture("edit-range");
        app.session.confirm_sample_conflicts().unwrap();
        app.show_sample_conflicts = false;
        app.selected_sample = Some(1_000_000);
        app.show_sample_edit = true;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!((app.edit_start, app.edit_end), (0, 2000));
        app.edit_start = 200;
        app.edit_end = 600;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!((app.edit_start, app.edit_end), (200, 600));

        app.session.resolve_sample_conflict(0, 1).unwrap();
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.source_id(), Some(1_000_000));
        assert_eq!((app.edit_start, app.edit_end), (0, 2000));

        app.edit_start = 300;
        app.edit_end = 800;
        app.history_step(false);
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!((app.edit_start, app.edit_end), (0, 2000));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sample_conflict_modal_shows_paths_and_switches_every_instance_without_closing() {
        let (context, mut app, dir) = sample_conflicts_fixture("switch");
        let before = app.session.project().clips.clone();
        let candidates = app.session.sample_conflicts()[0].candidates.clone();
        let label = format!(
            "Use match 2\n{}",
            display_paths(&candidates[1].to_string_lossy())
        );
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "SAMPLE CONFLICTS");
        text_rect(&output, "3 instances · 2 matches");
        let (_, control) = accessible_node(&output, &label);
        assert_eq!(control.role(), egui::accesskit::Role::RadioButton);
        text_rect(
            &output,
            &format!(
                "Selected match 1\n{}",
                display_paths(&candidates[0].to_string_lossy())
            ),
        );
        let choice = text_rect(&output, &label);
        click(&context, &mut app, choice.center(), egui::Modifiers::NONE);
        assert_eq!(app.session.sample_conflicts()[0].selected, 1);
        assert!(
            app.session
                .project()
                .imported_samples
                .iter()
                .all(|sample| sample.path == candidates[1])
        );
        assert_eq!(app.session.project().clips, before);
        assert!(
            app.sample_conflicts_open(),
            "Choices must remain available until the user closes the modal."
        );
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let first = text_rect(
            &output,
            &format!(
                "Use match 1\n{}",
                display_paths(&candidates[0].to_string_lossy())
            ),
        );
        click(&context, &mut app, first.center(), egui::Modifiers::NONE);
        assert_eq!(app.session.sample_conflicts()[0].selected, 0);
        assert!(
            app.session
                .project()
                .imported_samples
                .iter()
                .all(|sample| sample.path == candidates[0])
        );
        for path in candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn escape_hides_unconfirmed_sample_conflicts_without_an_edit_until_reopened() {
        let (context, mut app, dir) = sample_conflicts_fixture("escape");
        let before = app.session.project().clone();
        assert!(app.sample_conflicts_open());
        frame(
            &context,
            &mut app,
            vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(!app.sample_conflicts_open());
        assert_eq!(app.session.project(), &before);
        assert!(!app.session.unsaved());
        assert!(!app.session.history_step(false).unwrap());
        assert!(!app.session.sample_conflicts()[0].confirmed);
        assert!(app.status.contains("not confirmed"));
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            !output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "SAMPLE CONFLICTS")
            }),
            "A dismissed resolver must stay hidden until it is reopened."
        );
        let reopen = text_rect(&output, "Sample conflicts");
        click(&context, &mut app, reopen.center(), egui::Modifiers::NONE);
        assert!(app.sample_conflicts_open());
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let keep = text_rect(&output, "Keep choices");
        click(&context, &mut app, keep.center(), egui::Modifiers::NONE);
        assert!(app.session.sample_conflicts()[0].confirmed);
        assert!(app.session.unsaved());
        for path in &app.session.sample_conflicts()[0].candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn sample_conflict_confirmation_stays_closed_and_status_can_reopen_choices() {
        let (context, mut app, dir) = sample_conflicts_fixture("confirm");
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let keep = text_rect(&output, "Keep choices");
        click(&context, &mut app, keep.center(), egui::Modifiers::NONE);
        assert!(!app.sample_conflicts_open());
        assert!(
            app.session
                .sample_conflicts()
                .iter()
                .all(|conflict| conflict.confirmed)
        );
        for _ in 0..3 {
            frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        }
        assert!(!app.sample_conflicts_open());
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let reopen = text_rect(&output, "Sample conflicts");
        click(&context, &mut app, reopen.center(), egui::Modifiers::NONE);
        assert!(app.sample_conflicts_open());
        frame(
            &context,
            &mut app,
            vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(!app.sample_conflicts_open());
        let reopened_project: Project =
            serde_json::from_str(&serde_json::to_string(app.session.project()).unwrap()).unwrap();
        let session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            reopened_project,
        )
        .unwrap();
        let reopened = MixJamApp::from_session(session, "Reopened".into(), false);
        assert!(
            !reopened.sample_conflicts_open(),
            "Confirmed choices must not warn again after reopening."
        );
        for path in &app.session.sample_conflicts()[0].candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn sample_conflict_modal_keeps_actions_and_wrapped_paths_inside_supported_viewports() {
        let (context, mut app, dir) = sample_conflicts_fixture("layout");
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            for _ in 0..3 {
                frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            for label in ["SAMPLE CONFLICTS", "Keep choices", "Play song"] {
                assert!(
                    viewport.contains_rect(text_rect(&output, label)),
                    "{label} lies outside {size:?}"
                );
            }
            for (index, path) in app.session.sample_conflicts()[0]
                .candidates
                .iter()
                .enumerate()
            {
                let label = format!(
                    "{} match {}\n{}",
                    if index == 0 { "Selected" } else { "Use" },
                    index + 1,
                    display_paths(&path.to_string_lossy())
                );
                assert!(viewport.contains_rect(text_rect(&output, &label)));
            }
        }
        for path in &app.session.sample_conflicts()[0].candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn missing_clip_keeps_full_span_label_selection_and_history() {
        let (context, mut app) = missing_fixture();
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let label = text_rect(&output, "Missing: Lost lead");
        let timeline = app.timeline_rect.unwrap();
        let clip = app.session.project().clips[0].clone();
        let rect = app.clip_rect(&clip, timeline);
        assert!((rect.width() - (4.0 * app.timeline_bar_width - 1.0)).abs() < 0.01);
        assert!((rect.left() - timeline.left() - app.timeline_bar_width).abs() < 0.01);
        assert!(rect.contains(label.center()));
        click(&context, &mut app, rect.center(), egui::Modifiers::NONE);
        assert_eq!(app.selected_clip, Some(clip.id));
        assert_eq!(app.source_name(), "Missing: Lost lead");
        app.preview(clip.sample_id);
        assert!(
            app.status
                .contains("Cannot preview missing sample Lost lead")
        );
        app.delete_selection();
        assert!(app.session.project().clips.is_empty());
        app.history_step(false);
        assert_eq!(app.session.project().clips[0], clip);
        assert_eq!(app.session.missing_samples().len(), 1);
    }

    #[test]
    fn missing_clip_context_menu_offers_manual_resolution_without_changing_project() {
        let (context, mut app) = missing_fixture();
        context.enable_accesskit();
        let before = app.session.project().clone();
        let pos = app
            .clip_rect(&before.clips[0], app.timeline_rect.unwrap())
            .center();
        for pressed in [true, false] {
            frame(
                &context,
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
            );
        }
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "Resolve manually");
        accessible_node(&output, "Resolve manually");
        let id = before.clips[0].sample_id;
        assert_eq!(app.context_missing_sample, Some(id));
        assert_eq!(app.session.project(), &before);

        let dir = std::env::temp_dir().join(format!("mixjam-ui-resolve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let placeholder = Session::headless(Arc::new(Library::fixture(vec![])), Project::default())
            .unwrap();
        app.session =
            std::mem::replace(&mut app.session, placeholder).with_settings_root(dir.join("settings"));
        let status = app.status.clone();
        app.resolve_missing_sample_with(id, None);
        assert_eq!(app.session.project(), &before);
        assert_eq!(app.context_missing_sample, Some(id));
        assert_eq!(app.status, status);
        let bad = dir.join("bad.wav");
        std::fs::write(&bad, b"not audio").unwrap();
        app.resolve_missing_sample_with(id, Some(bad.clone()));
        assert_eq!(app.session.project(), &before);
        assert!(app.status_error);
        let chosen = dir.join("chosen.wav");
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 44_100],
        }
        .write_wav(&chosen)
        .unwrap();
        app.resolve_missing_sample_with(999, Some(chosen.clone()));
        assert_eq!(app.session.project(), &before);
        app.resolve_missing_sample_with(id, Some(chosen.clone()));
        assert!(app.session.missing_samples().is_empty());
        assert_eq!(app.context_missing_sample, None);
        let resolved = &app.session.project().clips[0];
        assert_eq!(
            (resolved.track, resolved.start_tick, resolved.length_ticks, resolved.gain),
            (2, TICKS_PER_BAR, 4 * TICKS_PER_BAR, 0.75)
        );
        assert!(app.session.history_step(false).unwrap());
        assert_eq!(app.session.project(), &before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_metadata_keeps_original_reference_raw_fields_and_length_confidence() {
        let (_, app) = missing_fixture();
        let mut missing = app.session.missing_samples()[0].clone();
        missing
            .metadata
            .insert("Original code".into(), "LOST42".into());
        missing
            .metadata
            .insert("Saved raw flags".into(), "0x0017".into());
        let details = missing_sample_details(&missing);
        for value in [
            "Missing: Lost lead",
            "Playback: silence",
            "Native sample ID: 42",
            "132 BPM",
            "1024 ticks",
            "known",
            "Original code: LOST42",
            "Saved raw flags: 0x0017",
            "The original audio file is absent.",
        ] {
            assert!(
                details.contains(value),
                "Missing retained metadata: {value}"
            );
        }
        missing.length_inferred = true;
        missing.reference = MissingReference::External {
            path: PathBuf::from(r"C:\old sounds\Lost lead.wav"),
            origin: Some(Product::D2),
        };
        let details = missing_sample_details(&missing);
        assert!(details.contains(r"Original path: C:\old sounds\Lost lead.wav"));
        assert!(details.contains("inferred"));
        missing.reference = MissingReference::Generated {
            product: Product::D2,
            kind: "Drum".into(),
            data: serde_json::json!({"pattern": [1, 0, 1]}),
        };
        assert!(missing_sample_details(&missing).contains("\"pattern\":[1,0,1]"));
        let clip = missing_clip_details(&app.session.project().clips[0]);
        assert!(
            clip.contains("Track 3") && clip.contains("150 frames") && clip.contains("gain 0.75")
        );
    }

    #[test]
    fn missing_sample_details_are_accessible_and_processing_explains_silence() {
        let (context, mut app) = missing_fixture();
        context.enable_accesskit();
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        accessible_node(&output, "Import details");
        text_rect(&output, "1 missing · silent");
        let button = text_rect(&output, "Import details");
        click(&context, &mut app, button.center(), egui::Modifiers::NONE);
        assert!(app.show_import_details);
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("Native sample ID: 42"))));
        app.show_import_details = false;
        app.select_clip(1, false);
        app.show_sample_edit = true;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(
            &output,
            "Missing audio: clips play silence; processing is unavailable.",
        );
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Listen edit")));
    }

    fn frame(
        context: &egui::Context,
        app: &mut MixJamApp,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        frame_at_size(context, app, events, modifiers, Vec2::new(1440.0, 1000.0))
    }

    fn frame_at_size(
        context: &egui::Context,
        app: &mut MixJamApp,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
        size: Vec2,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            modifiers,
            focused: true,
            ..Default::default()
        };
        context.run(input, |ctx| app.draw(ctx))
    }

    fn pointer(
        context: &egui::Context,
        app: &mut MixJamApp,
        pos: Pos2,
        pressed: bool,
        modifiers: egui::Modifiers,
    ) {
        frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                },
            ],
            modifiers,
        );
    }

    fn click(context: &egui::Context, app: &mut MixJamApp, pos: Pos2, modifiers: egui::Modifiers) {
        pointer(context, app, pos, true, modifiers);
        pointer(context, app, pos, false, modifiers);
    }

    fn text_rect(output: &egui::FullOutput, label: &str) -> Rect {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("Missing text: {label}"))
    }

    fn accessible_node<'a>(
        output: &'a egui::FullOutput,
        label: &str,
    ) -> (egui::accesskit::NodeId, &'a egui::accesskit::Node) {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("Accessibility must be enabled for this test.")
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .map(|(id, node)| (*id, node))
            .unwrap_or_else(|| panic!("Missing accessible control: {label}"))
    }

    fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn focus_control(
        context: &egui::Context,
        app: &mut MixJamApp,
        output: &egui::FullOutput,
        label: &str,
    ) -> egui::FullOutput {
        let (target, _) = accessible_node(output, label);
        frame(
            context,
            app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
        )
    }

    #[test]
    fn mixer_master_stays_visible_while_scrolling_all_tracks_at_supported_sizes() {
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.show_mixer = true;
            for track in 2..TRACK_COUNT {
                app.session.place(1, track, 0).unwrap();
            }
            for _ in 0..12 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let master = text_rect(&output, "Master");
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(master));
            let bank = text_rect(&output, "Track 1");
            frame_at_size(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(bank.center()),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(-20_000.0, 0.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
                size,
            );
            for _ in 0..60 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            assert_eq!(text_rect(&output, "Master"), master, "Window: {size:?}");
            let last = text_rect(&output, "Track 64");
            assert!(last.right() < master.left(), "Window: {size:?}; {last:?}");
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == "Track 64" && shape.clip_rect.contains_rect(last))
            }));
            let bounds = accessible_node(&output, "Master gain").1.bounds().unwrap();
            let pos = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, bounds.y0 as f32 + 2.0);
            for pressed in [true, false] {
                frame_at_size(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    egui::Modifiers::NONE,
                    size,
                );
            }
            assert!(app.session.project().master_gain > 1.5, "Window: {size:?}");
        }
    }

    #[test]
    fn arrangement_and_mixer_routes_mirror_states_edits_and_history() {
        fn assert_routes(output: &egui::FullOutput, track: usize, left: bool, right: bool) {
            for (side, text, enabled) in [("left", "L", left), ("right", "R", right)] {
                for label in [format!("Arrangement track {track} {side} output"), format!("Track {track} {side} output")] {
                    let (_, node) = accessible_node(output, &label);
                    assert_eq!(node.toggled(), Some(if enabled { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False }), "{label}");
                    let bounds = node.bounds().unwrap();
                    let rect = Rect::from_min_max(Pos2::new(bounds.x0 as f32, bounds.y0 as f32), Pos2::new(bounds.x1 as f32, bounds.y1 as f32));
                    let painted = output.shapes.iter().find_map(|shape| match &shape.shape {
                        egui::Shape::Text(painted) if painted.galley.job.text == text && rect.contains(painted.pos) => Some(painted),
                        _ => None,
                    }).unwrap_or_else(|| panic!("Missing painted state for {label}"));
                    assert_eq!(painted.galley.job.sections[0].format.color, if enabled { ACCENT } else { MUTED }, "{label}");
                }
            }
        }
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.clips[1].track = 63;
        project.tracks[0].pan = -0.75;
        project.tracks[63].pan = 0.5;
        project.tracks[63].gain = 1.25;
        app.session = Session::headless(Arc::new(Library::fixture(app.session.library().samples.to_vec())), project).unwrap();
        app.show_mixer = true;
        ctx.enable_accesskit();
        for (left, right) in [(true, true), (true, false), (false, false), (false, true)] {
            for (side, enabled) in [(true, left), (false, right)] {
                app.session.edit(EditCommand::SetRouting { track: 0, left: side, enabled }).unwrap();
            }
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, 1, left, right);
            assert_routes(&output, 64, true, true);
        }
        for (label, track, left) in [
            ("Arrangement track 1 left output", 0, true),
            ("Track 64 right output", 63, false),
        ] {
            let before = app.session.project().tracks[track].clone();
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let bounds = accessible_node(&output, label).1.bounds().unwrap();
            click(&ctx, &mut app, Pos2::new((bounds.x0 + bounds.x1) as f32 * 0.5, (bounds.y0 + bounds.y1) as f32 * 0.5), egui::Modifiers::NONE);
            let changed = app.session.project().tracks[track].clone();
            assert_eq!(changed.left_enabled, if left { !before.left_enabled } else { before.left_enabled });
            assert_eq!(changed.right_enabled, if left { before.right_enabled } else { !before.right_enabled });
            assert_eq!(changed.pan, before.pan);
            assert_eq!(changed.gain, before.gain);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, track + 1, changed.left_enabled, changed.right_enabled);
            app.history_step(false);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, track + 1, before.left_enabled, before.right_enabled);
            app.history_step(true);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, track + 1, changed.left_enabled, changed.right_enabled);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Arrangement track 64 left output");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Space, egui::Modifiers::NONE)], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_routes(&output, 64, false, false);
        assert!(!app.session.wants_playback(), "Space on a lane output must toggle routing only.");
        assert_eq!(app.session.project().tracks[0].pan, -0.75);
        assert_eq!(app.session.project().tracks[63].pan, 0.5);
    }

    #[test]
    fn mixer_gain_labels_use_one_decimal_without_changing_precision() {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.tracks[0].gain = -0.7533;
        project.tracks[1].gain = 0.3333;
        project.master_gain = 0.7777;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        ctx.enable_accesskit();
        app.show_mixer = true;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for label in ["-0.8", "+0.3", "+0.8"] {
            text_rect(&output, label);
        }
        assert_eq!(app.session.project().tracks[0].gain, -0.7533);
        assert_eq!(app.session.project().tracks[1].gain, 0.3333);
        assert_eq!(app.session.project().master_gain, 0.7777);
        assert!(!app.session.unsaved());

        // Entering the rounded value is a real edit, even though the passive
        // label stays the same. Merely focusing must retain the full value.
        focus_control(&ctx, &mut app, &output, "Track 2 gain value");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "+0.3333");
        frame(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("+0.3".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().tracks[1].gain, 0.3);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "+0.3");
        assert_eq!(app.session.project().tracks[0].gain, -0.7533);
        assert_eq!(app.session.project().master_gain, 0.7777);
    }

    #[test]
    fn mixer_gain_pan_and_routes_support_accessibility_and_keyboard_input() {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.tracks[1].gain = 0.3333;
        project.master_gain = 0.7777;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        ctx.enable_accesskit();
        app.show_mixer = true;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let (_, gain) = accessible_node(&output, "Track 1 gain");
        assert_eq!(gain.role(), egui::accesskit::Role::Slider);
        assert_eq!(gain.numeric_value(), Some(1.0));
        assert_eq!(gain.min_numeric_value(), Some(-2.0));
        assert_eq!(gain.max_numeric_value(), Some(4.0));
        accessible_node(&output, "Track 2 gain");
        accessible_node(&output, "Master gain");
        let (_, left) = accessible_node(&output, "Track 1 left output");
        assert_eq!(left.role(), egui::accesskit::Role::CheckBox);
        assert_eq!(left.toggled(), Some(egui::accesskit::Toggled::True));
        focus_control(&ctx, &mut app, &output, "Track 2 gain value");
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Master gain value");
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 left output");
        assert_eq!(app.session.project().tracks[1].gain, 0.3333);
        assert_eq!(app.session.project().master_gain, 0.7777);
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(!app.session.project().tracks[0].left_enabled);
        assert!(app.session.project().tracks[1].left_enabled);
        assert_eq!(
            accessible_node(&output, "Track 1 left output").1.toggled(),
            Some(egui::accesskit::Toggled::False)
        );
        focus_control(&ctx, &mut app, &output, "Track 1 right output");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(!app.session.project().tracks[0].right_enabled);
        assert!(
            !app.session.wants_playback(),
            "A routing key must not start playback."
        );
        text_rect(&output, "Muted");

        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        frame(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("-0.7533".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().tracks[0].gain, -0.7533);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.session.project().tracks[0].gain, -0.7533);
        focus_control(&ctx, &mut app, &output, "Reset Track 1 gain to unity");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().tracks[0].gain, 1.0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 pan");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::ArrowRight, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project().tracks[0].pan > 0.0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Center Track 1 pan");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().tracks[0].pan, 0.0);
        assert_eq!(app.session.project().tracks[1].gain, 0.3333);
        assert_eq!(app.session.project().master_gain, 0.7777);
    }

    #[test]
    fn mixer_summaries_and_control_identity_follow_sparse_project_tracks() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        let mut samples = app.session.library().samples.to_vec();
        let mut second = samples[0].clone();
        second.id = 2;
        second.name = "Second sound".into();
        samples.push(second);
        let mut project = app.session.project().clone();
        project.clips[1].track = 63;
        app.session = Session::headless(Arc::new(Library::fixture(samples)), project).unwrap();
        app.session.place(2, 0, TICKS_PER_BAR * 2).unwrap();
        app.show_mixer = true;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "2 sounds");
        text_rect(&output, "good grasp 2");
        let track64 = accessible_node(&output, "Track 64 gain").0;
        let track1_ids = app
            .session
            .project()
            .clips
            .iter()
            .filter(|clip| clip.track == 0)
            .map(|clip| clip.id)
            .collect();
        app.session
            .edit(EditCommand::DeleteClips(track1_ids))
            .unwrap();
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(accessible_node(&output, "Track 64 gain").0, track64);
        focus_control(&ctx, &mut app, &output, "Track 64 gain");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project().tracks[63].gain > 1.0);
        assert_eq!(app.session.project().tracks[0].gain, 1.0);
        app.session.place(1, 0, 0).unwrap();
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(accessible_node(&output, "Track 64 gain").0, track64);
        text_rect(&output, "Track 1");
        text_rect(&output, "Track 64");
    }

    #[test]
    fn empty_mixer_keeps_guidance_master_and_action_errors_visible() {
        let (ctx, mut app) = fixture();
        app.new_mix();
        app.show_mixer = true;
        assert_eq!(DEFAULT_STATUS, "Ready");
        let (status, status_error) = startup_status(&mut app.session);
        assert_eq!((status.as_str(), status_error), (DEFAULT_STATUS, false));
        app.status = status;
        app.status_error = status_error;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "NO TRACKS");
        text_rect(&output, "Place a sound in the arrangement.");
        text_rect(&output, "Master");
        text_rect(&output, DEFAULT_STATUS);
        app.error("Cannot open the selected sample.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "Cannot open the selected sample.");
        assert!(app.status_error);
        assert_eq!(app.status, "Cannot open the selected sample.");
    }

    #[test]
    fn transport_controls_do_not_move_when_position_digits_change() {
        let (ctx, mut app) = fixture();
        let size = Vec2::new(960.0, 720.0);
        let mut positions = None;
        for bar in [1, 21, 111, 888, 999, 1000, 4096] {
            app.session
                .seek((bar - 1) * TICKS_PER_BAR + 3 * TICKS_PER_BAR / 4);
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let current = ["BPM", "Zoom", "Follow"].map(|label| text_rect(&output, label));
            if let Some(expected) = positions {
                assert_eq!(current, expected);
            }
            positions = Some(current);
            assert!(
                current[2].right() < size.x - 64.0,
                "Follow must fit beside the rail."
            );
            let label = format!("{bar:03} : 4");
            let time = text_rect(&output, &label);
            assert!(
                time.left() >= 18.0 && time.right() <= 130.0,
                "{label}: {time:?}"
            );
        }
    }

    #[test]
    fn sound_library_divider_resizes_retains_height_and_preserves_arrangement() {
        let (ctx, mut app) = fixture();
        for track in 2..TRACK_COUNT {
            app.session.place(1, track, 0).unwrap();
        }
        let base = app.session.library().samples[0].clone();
        let samples = (1..=128)
            .map(|id| {
                let mut sample = base.clone();
                sample.id = id;
                sample.name = format!("Resizable sound {id}");
                sample
            })
            .collect();
        app.session = Session::headless(
            Arc::new(Library::fixture(samples)),
            app.session.project().clone(),
        )
        .unwrap();
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let painted = |output: &egui::FullOutput| {
            output.shapes.iter().filter(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.starts_with("Resizable sound"))
        }).count()
        };
        let before_count = painted(&output);
        let panel = || {
            egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds"))
                .unwrap()
                .rect
        };
        let before = panel();
        let arrangement_before = app.timeline_drop_rect.unwrap().height();
        let start = Pos2::new(before.center().x, before.top() + 1.0);
        let target = start - Vec2::new(0.0, 150.0);
        pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
        for _ in 0..2 {
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(target)],
                egui::Modifiers::NONE,
            );
        }
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let expanded = panel();
        assert!(
            expanded.height() > before.height() + 100.0,
            "{before:?} -> {expanded:?}"
        );
        assert!(painted(&output) > before_count);
        assert!(app.timeline_drop_rect.unwrap().height() < arrangement_before);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            panel(),
            expanded,
            "The divider must keep its height after release."
        );
        // Shrinking the window clamps a previous large panel to preserve the lanes.
        for _ in 0..2 {
            frame_at_size(
                &ctx,
                &mut app,
                vec![],
                egui::Modifiers::NONE,
                Vec2::new(960.0, 720.0),
            );
        }
        assert!(panel().height() <= 336.0);
        assert!(app.timeline_drop_rect.unwrap().height() >= 80.0);
        assert!(!app.session.unsaved());
    }

    #[test]
    fn follow_keeps_playhead_visible_and_off_preserves_manual_viewport() {
        let (ctx, mut app) = fixture();
        for track in 2..TRACK_COUNT {
            app.session.place(1, track, 0).unwrap();
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let visible = app.timeline_drop_rect.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(visible.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -1000.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        // Let egui's wheel smoothing settle before checking the other axis.
        for _ in 0..60 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let vertical = app.timeline_rect.unwrap().top();
        let project_before = app.session.project().clone();
        let mut first_follow_left = None;
        for (tick, zoom) in [
            (40 * TICKS_PER_BAR, 1.0),
            (41 * TICKS_PER_BAR, 1.0),
            (41 * TICKS_PER_BAR, 2.0),
            (0, 2.0),
        ] {
            app.zoom = zoom;
            app.session.seek(tick);
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let canvas = app.timeline_rect.unwrap();
            let ruler = app.timeline_ruler_rect.unwrap();
            let x = canvas.left() + tick as f32 / TICKS_PER_BAR as f32 * app.timeline_bar_width;
            assert!(
                x >= ruler.left() - 1.0 && x <= ruler.right(),
                "tick {tick}: {x}, {ruler:?}"
            );
            assert!(
                (canvas.top() - vertical).abs() < 1.0,
                "Follow must retain the vertical offset: {} -> {} at tick {tick}.",
                vertical,
                canvas.top()
            );
            let left = canvas.left();
            if tick == 40 * TICKS_PER_BAR {
                first_follow_left = Some(left);
            } else if tick == 41 * TICKS_PER_BAR && zoom == 1.0 {
                assert_eq!(
                    Some(left),
                    first_follow_left,
                    "Follow must not scroll on every tick."
                );
            }
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(app.timeline_rect.unwrap().left(), left);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let toggle = text_rect(&output, "Follow").center();
        click(&ctx, &mut app, toggle, egui::Modifiers::NONE);
        assert!(!app.follow_playhead);
        let left = app.timeline_rect.unwrap().left();
        app.session.seek(50 * TICKS_PER_BAR);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_rect.unwrap().left(), left);
        click(&ctx, &mut app, toggle, egui::Modifiers::NONE);
        assert!(app.follow_playhead);
        assert!(app.timeline_rect.unwrap().left() < left);
        // Panning while paused does not fight the Follow setting.
        let visible = app.timeline_drop_rect.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(visible.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(300.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        for _ in 0..60 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let panned = app.timeline_rect.unwrap().left();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_rect.unwrap().left(), panned);
        assert_eq!(app.session.project(), &project_before);
    }

    #[test]
    fn native_canvas_shift_selection_group_drag_and_delete_are_wired() {
        let (ctx, mut app) = fixture();
        let canvas = app.timeline_rect.unwrap();
        let first = app
            .clip_rect(&app.session.project().clips[0], canvas)
            .center();
        let second = app
            .clip_rect(&app.session.project().clips[1], canvas)
            .center();
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        click(&ctx, &mut app, second, egui::Modifiers::SHIFT);
        assert_eq!(app.selected_clips.len(), 2, "Shift must add a second clip.");
        let target = first + Vec2::new(app.timeline_bar_width, 0.0);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        assert_eq!(app.session.project().clips[0].start_tick, TICKS_PER_BAR);
        assert_eq!(app.session.project().clips[1].start_tick, TICKS_PER_BAR * 2);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert!(
            app.session.project().clips.is_empty(),
            "Delete must remove the selected group."
        );
        app.history_step(false);
        assert_eq!(app.session.project().clips.len(), 2);
    }

    #[test]
    fn a_short_drag_keeps_the_bar_and_an_erased_clip_leaves_the_selection() {
        let (ctx, mut app) = fixture();
        let canvas = app.timeline_rect.unwrap();
        let bar = app.timeline_bar_width;
        let first = app
            .clip_rect(&app.session.project().clips[0], canvas)
            .center();
        let second = app
            .clip_rect(&app.session.project().clips[1], canvas)
            .center();
        let starts = |app: &MixJamApp| {
            app.session
                .project()
                .clips
                .iter()
                .map(|clip| clip.start_tick)
                .collect::<Vec<_>>()
        };
        let before = starts(&app);
        // The nearest bar decides, to the left as well as to the right.
        for (distance, bars) in [(-0.3, 0), (0.3, 0), (0.7, 1), (-0.7, -1)] {
            let from = app
                .clip_rect(&app.session.project().clips[1], canvas)
                .center();
            let target = from + Vec2::new(bar * distance, 0.0);
            pointer(&ctx, &mut app, from, true, egui::Modifiers::NONE);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(target)],
                egui::Modifiers::NONE,
            );
            pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
            let expected = before[1] as i64 + bars * i64::from(TICKS_PER_BAR);
            assert_eq!(
                i64::from(app.session.project().clips[1].start_tick),
                expected,
                "A drag of {distance} bars must move the clip {bars} bars."
            );
            if bars != 0 {
                app.history_step(false);
            }
        }
        assert_eq!(starts(&app), before);
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        click(&ctx, &mut app, second, egui::Modifiers::SHIFT);
        let erased = app.session.project().clips[0].id;
        app.erase = true;
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        app.erase = false;
        assert!(!app.selected_clips.contains(&erased));
        app.delete_selection();
        assert!(
            app.session.project().clips.is_empty(),
            "Delete must still remove the rest of the selection."
        );
    }

    #[test]
    fn native_canvas_alt_drag_copies_group_and_undo_restores_once() {
        let (ctx, mut app) = fixture();
        let canvas = app.timeline_rect.unwrap();
        let first = app
            .clip_rect(&app.session.project().clips[0], canvas)
            .center();
        app.selected_clips = [1, 2].into_iter().collect();
        app.selected_clip = Some(1);
        let target = first + Vec2::new(app.timeline_bar_width * 3.0, 0.0);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::ALT);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::ALT,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::ALT);
        assert_eq!(app.session.project().clips.len(), 4);
        assert_eq!(app.selected_clips.len(), 2);
        app.history_step(false);
        assert_eq!(app.session.project().clips.len(), 2);
    }

    #[test]
    fn fine_snap_and_two_line_labels_preserve_unicode_boundaries() {
        let (_, app) = fixture();
        let rect = app.timeline_rect.unwrap();
        let x = rect.left() + app.timeline_bar_width * 1.59;
        assert_eq!(app.position_tick(x, rect, false), 256);
        assert_eq!(app.position_tick(x, rect, true), 400);
        assert_eq!(clip_label("good grasp 2", 54.0, 11.0), "good\ngrasp 2");
        assert_eq!(clip_label("électronique", 10.0, 11.0).lines().count(), 2);
        let short = visible_clip_label("good grasp 2", 54.0, 9.0, 16.0);
        assert_eq!(
            short.lines().count(),
            1,
            "Short clip rows must keep the text on one visible line."
        );
        assert!(short.ends_with('…'));
        assert_eq!(
            visible_clip_label("good grasp 2", 54.0, 11.0, 26.0),
            "good\ngrasp 2"
        );
    }

    #[test]
    fn imported_catalog_tracks_history_atomically_and_keeps_source_tempo() {
        let (_, mut app) = fixture();
        app.palette_product = Some(Product::T1);
        let path = std::env::temp_dir().join(format!(
            "mixjam-techno-ui-import-{}.wav",
            std::process::id()
        ));
        let pcm = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.25; 1000],
        };
        pcm.write_wav(&path).unwrap();
        let id = app.register_file(&path, "Wave", 90.0).unwrap();
        assert_eq!(
            app.session.library().sample_by_id(id).unwrap().source_bpm,
            90.0
        );
        assert_eq!(app.session.project().imported_samples.len(), 1);
        assert!(app.palette_product.is_none());
        assert!(
            app.palette_indices()
                .iter()
                .any(|&index| app.session.library().samples[index].id == id)
        );
        app.history_step(false);
        assert!(app.session.library().sample_by_id(id).is_none());
        app.history_step(true);
        assert!(app.session.library().sample_by_id(id).is_some());
        app.history_step(false);
        std::fs::remove_file(&path).unwrap();
        let before = app.session.project().clone();
        app.history_step(true);
        assert_eq!(
            app.session.project(),
            &before,
            "A missing imported file must reject redo atomically."
        );
        assert!(app.session.library().sample_by_id(id).is_none());
        assert!(app.status_error);
    }

    #[test]
    fn unsaved_mix_requires_a_pending_action_before_replacement() {
        let (ctx, mut app) = fixture();
        app.session.edit(EditCommand::SetTempo(160.0)).unwrap();
        app.request_action(PendingAction::New, &ctx);
        assert!(matches!(app.pending_action, Some(PendingAction::New)));
        assert_eq!(app.session.project().clips.len(), 2);
        assert!(!app.allow_close);
    }

    #[test]
    fn scrolling_reaches_track_64_and_keeps_labels_and_ruler_interactive() {
        let (ctx, mut app) = fixture();
        for track in 2..TRACK_COUNT {
            app.session.place(1, track, 0).unwrap();
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks.len(), 64);
        assert!(app.timeline_add_track.is_none());
        let visible = app.timeline_drop_rect.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(visible.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -2500.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let canvas = app.timeline_rect.unwrap();
        let visible = app.timeline_drop_rect.unwrap();
        let row_y = canvas.top() + 63.5 * app.timeline_row_height;
        assert!(visible.contains(Pos2::new(visible.center().x, row_y)));
        click(
            &ctx,
            &mut app,
            Pos2::new(visible.left() - 44.0, row_y),
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.armed_track, 63,
            "Track labels must share the lane scroll offset."
        );
        click(
            &ctx,
            &mut app,
            Pos2::new(visible.left() - 70.0, row_y),
            egui::Modifiers::NONE,
        );
        assert!(
            !app.session.project().tracks[63].left_enabled,
            "route click at {row_y}; current canvas {:?}, visible {:?}, row {}; disabled tracks {:?}",
            app.timeline_rect,
            app.timeline_drop_rect,
            app.timeline_row_height,
            app.session
                .project()
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, track)| !track.left_enabled)
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        );
        assert!(app.session.project().tracks[0].left_enabled);
        let ruler = app.timeline_ruler_rect.unwrap();
        let seek_x = ruler.left() + 2.5 * app.timeline_bar_width;
        click(
            &ctx,
            &mut app,
            Pos2::new(seek_x, ruler.center().y),
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.cursor_tick(), 2 * TICKS_PER_BAR);
        app.selected_sample = Some(1);
        click(
            &ctx,
            &mut app,
            Pos2::new(seek_x, row_y),
            egui::Modifiers::NONE,
        );
        assert!(
            app.session
                .project()
                .clips
                .iter()
                .any(|clip| clip.track == 63 && clip.start_tick == 2 * TICKS_PER_BAR)
        );
        let ruler_before = app.timeline_ruler_rect.unwrap();
        let visible = app.timeline_drop_rect.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(visible.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-400.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let ruler = app.timeline_ruler_rect.unwrap();
        let canvas = app.timeline_rect.unwrap();
        assert!(canvas.left() < ruler.left() - 100.0);
        assert_eq!(
            ruler.left(),
            ruler_before.left(),
            "Ruler viewport stays fixed while bars scroll."
        );
        let pos = Pos2::new(ruler.left() + 30.0, ruler.center().y);
        click(&ctx, &mut app, pos, egui::Modifiers::NONE);
        let expected = app.position_tick(pos.x, app.timeline_rect.unwrap(), false);
        assert_eq!(app.session.cursor_tick(), expected);
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
        app.group = Some("kick".into());
        assert_eq!(app.palette_indices(), vec![2]);
        app.group = None;
        app.palette_product = Some(Product::T1);
        assert_eq!(app.palette_indices(), vec![1]);
        app.palette_product = None;
        app.group = Some(String::new());
        assert_eq!(app.palette_indices(), vec![3]);
    }

    #[test]
    fn failed_open_and_in_use_library_removal_preserve_the_active_project() {
        let (_, mut app) = fixture();
        let previous = app.session.project().clone();
        let library = app.session.library().samples.as_ptr();
        app.open_path(Path::new("missing-ui-fixture.invalid"));
        assert!(app.status_error);
        assert_eq!(app.session.project(), &previous);
        assert!(std::ptr::eq(
            library,
            app.session.library().samples.as_ptr()
        ));
        app.remove_library(Product::T2);
        assert!(app.status_error);
        assert!(app.status.contains("This mix uses"));
        assert_eq!(app.session.project(), &previous);
        assert!(std::ptr::eq(
            library,
            app.session.library().samples.as_ptr()
        ));
    }

    #[test]
    fn persisted_import_notices_are_visible_at_startup_and_keep_details() {
        let (_, app) = fixture();
        let mut project = app.session.project().clone();
        project.import_notices.push(ImportNotice {
            source_bpm: 146.0,
            target_bpm: 140.0,
        });
        let mut reopened = MixJamApp::from_session(
            Session::headless(
                Arc::new(Library::fixture(app.session.library().samples.to_vec())),
                project,
            )
            .unwrap(),
            "Ready.".into(),
            false,
        );
        assert!(reopened.status.contains("146 BPM changed to 140 BPM"));
        reopened.message("Ready to edit.");
        assert_eq!(reopened.session.project().import_notices.len(), 1);
        reopened.set_startup_messages(&["One library folder is unavailable.".into()]);
        assert!(reopened.status_error);
        assert!(
            reopened
                .status
                .contains("One library folder is unavailable")
        );
    }

    #[test]
    fn native_tempo_menu_and_pan_controls_accept_user_input() {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let mut bpm = 140.0;
        let mut pan = 0.0;
        let mut draw = |events: Vec<egui::Event>| {
            let mut tempo_rect = Rect::NOTHING;
            let mut pan_rect = Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        tempo_rect = bpm_picker(ui, "fixture_tempo", &mut bpm).rect;
                        pan_rect = pan_control(ui, 0, &mut pan).rect;
                    });
                },
            );
            (output, tempo_rect, pan_rect)
        };
        let (_, tempo, _) = draw(vec![]);
        let mouse = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        draw(mouse(tempo.center(), true));
        draw(mouse(tempo.center(), false));
        let (output, _, _) = draw(vec![]);
        let choice = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "180"
                {
                    Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            })
            .expect("The open tempo menu must offer 180 BPM.");
        draw(mouse(choice, true));
        draw(mouse(choice, false));
        let (_, _, pan_rect) = draw(vec![]);
        draw(mouse(
            Pos2::new(pan_rect.right() - 2.0, pan_rect.center().y),
            true,
        ));
        draw(mouse(
            Pos2::new(pan_rect.right() - 2.0, pan_rect.center().y),
            false,
        ));
        assert_eq!(bpm, 180.0);
        assert!(pan > 0.95 && pan <= 1.0);
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
        let painted_samples = output.shapes.iter().filter(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.starts_with("Corpus"))
        }).count();
        assert_eq!(app.palette_indices().len(), 31_620);
        assert!(
            painted_samples > 0 && painted_samples < 100,
            "A 31,620-sound catalog must paint the visible rows, not every sample: {painted_samples}"
        );
    }
    #[test]
    fn sparse_tracks_keep_model_ids_for_drag_copy_selection_and_mixer() {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.clips[1].track = 63;
        project.tracks[31].gain = 1.25;
        project.tracks[31].pan = 0.4;
        project.tracks[31].left_enabled = false;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        let hidden = app.session.project().tracks[31].clone();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks, vec![0, 63, 1]);
        assert_eq!(app.timeline_add_track, Some(1));
        let canvas = app.timeline_rect.unwrap();
        let first = app
            .clip_rect(&app.session.project().clips[0], canvas)
            .center();
        let second = app
            .clip_rect(&app.session.project().clips[1], canvas)
            .center();
        assert!((second.y - first.y - app.timeline_row_height).abs() < 0.1);
        let target = Pos2::new(first.x + 3.0 * app.timeline_bar_width, second.y);
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        let moved = app
            .session
            .project()
            .clips
            .iter()
            .find(|clip| clip.id == 1)
            .unwrap();
        assert_eq!(moved.track, 63);
        assert_eq!(moved.start_tick, 3 * TICKS_PER_BAR);
        app.history_step(false);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.timeline_rect.unwrap();
        let first = app
            .clip_rect(&app.session.project().clips[0], canvas)
            .center();
        let second = app
            .clip_rect(&app.session.project().clips[1], canvas)
            .center();
        let target = Pos2::new(first.x + 3.0 * app.timeline_bar_width, second.y);
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::ALT);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::ALT,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::ALT);
        assert_eq!(app.session.project().clips.len(), 3);
        assert_eq!(app.session.project().clips[0].track, 0);
        assert!(
            app.session
                .project()
                .clips
                .iter()
                .any(|clip| clip.track == 63 && clip.start_tick == 3 * TICKS_PER_BAR)
        );
        app.history_step(false);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.timeline_rect.unwrap();
        let origin = canvas.min + Vec2::splat(1.0);
        let target = Pos2::new(
            canvas.left() + 2.5 * app.timeline_bar_width,
            canvas.top() + 2.0 * app.timeline_row_height - 1.0,
        );
        pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        assert_eq!(app.selected_clips, [1, 2].into_iter().collect());
        let visible = app.timeline_drop_rect.unwrap();
        let second_y = canvas.top() + 1.5 * app.timeline_row_height;
        click(
            &ctx,
            &mut app,
            Pos2::new(visible.left() - 70.0, second_y),
            egui::Modifiers::NONE,
        );
        assert!(!app.session.project().tracks[63].left_enabled);
        assert!(app.session.project().tracks[1].left_enabled);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 350.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.mixer(ui));
            },
        );
        let labels: BTreeSet<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.job.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(labels.contains("Track 1") && labels.contains("Track 64"));
        assert!(!labels.contains("Track 2") && !labels.contains("Track 32"));
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.timeline_rect.unwrap();
        let add = Pos2::new(
            canvas.left() + 3.5 * app.timeline_bar_width,
            canvas.top() + 2.5 * app.timeline_row_height,
        );
        app.selected_sample = Some(1);
        click(&ctx, &mut app, add, egui::Modifiers::NONE);
        assert!(
            app.session
                .project()
                .clips
                .iter()
                .any(|clip| clip.track == 1)
        );
        assert_eq!(
            app.session
                .project()
                .clips
                .iter()
                .find(|clip| clip.id == 2)
                .unwrap()
                .track,
            63
        );
        assert_eq!(app.session.project().tracks[31], hidden);
        assert_eq!(app.session.project().tracks.len(), 64);
    }

    #[test]
    fn empty_project_record_chooser_can_create_track_64_without_empty_rows() {
        let (ctx, mut app) = fixture();
        app.new_mix();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks, vec![0]);
        assert_eq!(app.timeline_add_track, Some(0));
        assert!(occupied_tracks(app.session.project()).is_empty());
        assert!((app.timeline_rect.unwrap().height() - app.timeline_row_height).abs() < 0.1);
        let menu_ctx = egui::Context::default();
        configure_style(&menu_ctx);
        let mut selected = 0;
        let mut menu = |events: Vec<egui::Event>| {
            let mut rect = Rect::NOTHING;
            let output = menu_ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = recording_destination(ui, &mut selected, &[]).rect;
                    });
                },
            );
            (output, rect)
        };
        let (_, rect) = menu(vec![]);
        let mouse = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        menu(mouse(rect.center(), true));
        menu(mouse(rect.center(), false));
        let (output, _) = menu(vec![]);
        let first = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "New track 1"
                {
                    Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            })
            .unwrap();
        menu(vec![
            egui::Event::PointerMoved(first),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -5000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..5 {
            menu(vec![]);
        }
        let (output, _) = menu(vec![]);
        let last = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "New track 64"
                {
                    let pos = Rect::from_min_size(text.pos, text.galley.size()).center();
                    shape.clip_rect.contains(pos).then_some(pos)
                } else {
                    None
                }
            })
            .expect("The recording chooser must reach unused track 64.");
        menu(vec![egui::Event::PointerMoved(last)]);
        menu(mouse(last, true));
        menu(mouse(last, false));
        assert_eq!(selected, 63);
        app.armed_track = selected;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks, vec![63]);
        assert_eq!(app.timeline_add_track, Some(63));
        let canvas = app.timeline_rect.unwrap();
        app.selected_sample = Some(1);
        let drop = canvas.min + Vec2::new(30.0, app.timeline_row_height / 2.0);
        click(&ctx, &mut app, drop, egui::Modifiers::NONE);
        assert_eq!(app.session.project().clips.len(), 1);
        assert_eq!(app.session.project().clips[0].track, 63);
        assert_eq!(app.session.project().tracks.len(), 64);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(occupied_tracks(app.session.project()), vec![63]);
        assert!(app.timeline_tracks.len() <= 2);
    }
    #[test]
    fn dropping_a_group_below_the_add_target_cancels_without_deleting_or_history() {
        let (ctx, mut app) = fixture();
        app.selected_clips = [1, 2].into_iter().collect();
        app.selected_clip = Some(1);
        let before = app.session.project().clone();
        let was_unsaved = app.session.unsaved();
        let canvas = app.timeline_rect.unwrap();
        let first = app
            .clip_rect(&app.session.project().clips[0], canvas)
            .center();
        let below = Pos2::new(first.x, canvas.bottom() + 15.0);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(below)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, below, false, egui::Modifiers::NONE);
        assert_eq!(app.session.project(), &before);
        assert_eq!(app.selected_clips, [1, 2].into_iter().collect());
        assert_eq!(app.selected_clip, Some(1));
        assert_eq!(app.session.unsaved(), was_unsaved);
        assert!(app.status.contains("Move canceled"));
        assert!(
            !app.session.history_step(false).unwrap(),
            "Canceled movement must not add an edit-history entry."
        );
    }

    #[test]
    fn rail_icons_keep_tooltips_accessible_actions_and_cached_textures() {
        let (ctx, mut app) = fixture();
        let icons = app.rail_icons.as_ref().unwrap().as_ref().unwrap().clone();
        // The rail paints icons as textured meshes; header buttons paint
        // egui images as textured rectangles. Both carry the texture ID.
        let image_center = |output: &egui::FullOutput, icon: RailIcon| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == icons.texture_id(icon) => {
                        Some(mesh.calc_bounds().center())
                    }
                    egui::Shape::Rect(rect)
                        if rect.brush.as_ref().is_some_and(|brush| {
                            brush.fill_texture_id == icons.texture_id(icon)
                                && brush.uv != Rect::ZERO
                        }) =>
                    {
                        Some(rect.rect.center())
                    }
                    _ => None,
                })
                .expect("The view must paint its bundled icon texture.")
        };
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            !output.shapes.iter().any(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    let label = text.galley.job.text.as_str();
                    text.pos.x > 1364.0
                        && text.pos.y > 50.0
                        && (RailIcon::ALL.iter().any(|icon| icon.label() == label)
                            || matches!(label, "MIX" | "Record"))
                } else {
                    false
                }
            }),
            "Rail action names must not be painted beside the icons."
        );
        let mixer = image_center(&output, RailIcon::Mixer);
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(mixer)],
            egui::Modifiers::NONE,
        );
        let hovered = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            hovered.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Mixer")
            }),
            "The icon must preserve its action name in a hover tooltip."
        );
        ctx.enable_accesskit();
        let undo = image_center(&hovered, RailIcon::Undo);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(undo)],
            egui::Modifiers::NONE,
        );
        // Tooltips wait until the pointer rests after leaving the rail icon.
        for _ in 0..12 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let hovered_undo = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            hovered_undo.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Undo")
            }),
            "The header icon must preserve its action name in a hover tooltip."
        );
        for label in ["Undo", "Redo", "Help"] {
            let (_, node) = accessible_node(&hovered_undo, label);
            assert_eq!(node.role(), egui::accesskit::Role::Button, "{label}");
        }
        pointer(&ctx, &mut app, mixer, true, egui::Modifiers::NONE);
        let clicked = frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerButton {
                pos: mixer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert!(app.show_mixer);
        assert!(clicked.platform_output.events.iter().any(|event| {
            matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some("Mixer"))
        }), "The icon must expose its action name to accessibility output.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let erase = image_center(&output, RailIcon::Erase);
        click(&ctx, &mut app, erase, egui::Modifiers::NONE);
        assert!(app.erase);
        app.selected_clips = [1, 2].into_iter().collect();
        app.selected_clip = Some(1);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let delete = image_center(&output, RailIcon::Delete);
        click(&ctx, &mut app, delete, egui::Modifiers::NONE);
        assert!(app.session.project().clips.is_empty());
        app.history_step(false);
        assert_eq!(app.session.project().clips.len(), 2);
        let cached = app.rail_icons.as_ref().unwrap().as_ref().unwrap();
        assert!(
            Arc::ptr_eq(&icons, cached),
            "Rail textures must be reused across frames and actions."
        );
    }

    #[test]
    fn folder_tempo_list_scrolls_to_provider_controls_at_compact_size() {
        let dir = std::env::temp_dir().join(format!("mixjam-ui-tempos-{}", std::process::id()));
        for index in 0..60 {
            let folder = dir.join(format!("Folder {index:02}"));
            std::fs::create_dir_all(&folder).unwrap();
            for name in ["a", "b", "c"] {
                Pcm { sample_rate: 100, channels: 1, samples: vec![0.25; 200] }
                    .write_wav(&folder.join(format!("{name}.wav"))).unwrap();
            }
        }
        let library = Library::open_wav_folder(&dir, 120.0).unwrap();
        assert_eq!(library.folder_tempos().len(), 60);
        let session = Session::headless(Arc::new(library), Project::default()).unwrap();
        let mut app = MixJamApp::from_session(session, String::new(), false);
        app.show_libraries = true;
        let context = egui::Context::default();
        configure_style(&context);
        let size = Vec2::new(960.0, 720.0);
        for _ in 0..3 { frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size); }
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let position = text_rect(&output, "Folder tempos (60)").center();
        for pressed in [true, false] {
            frame_at_size(&context, &mut app, vec![egui::Event::PointerMoved(position),
                egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary,
                    pressed, modifiers: egui::Modifiers::NONE }], egui::Modifiers::NONE, size);
        }
        for _ in 0..20 {
            frame_at_size(&context, &mut app, vec![egui::Event::PointerMoved(position),
                egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -500.0), modifiers: egui::Modifiers::NONE }],
                egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let provider = text_rect(&output, "Optional product providers");
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(provider),
            "Provider controls must remain reachable after expanding a large tempo list: {provider:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn scan_adoption_clears_selection_and_drag_from_the_previous_palette() {
        let dir = std::env::temp_dir().join(format!("mixjam-ui-scan-{}", std::process::id()));
        let old = dir.join("old");
        let new = dir.join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        for (folder, value) in [(&old, 0.25), (&new, 0.5)] {
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![value; 1000],
            }
            .write_wav(&folder.join("sound.wav"))
            .unwrap();
        }
        let library = Library::open_wav_folder(&old, 140.0).unwrap();
        let source = library.samples[0].id;
        let session = Session::headless(Arc::new(library), Project::default())
            .unwrap()
            .with_settings_root(dir.join("settings"));
        let mut app = MixJamApp::from_session(session, String::new(), false);
        let context = egui::Context::default();
        app.select_wave_folder(&new);
        app.selected_sample = Some(source);
        app.drag = Some(DragItem::Sample(source));
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while app.session.wave_scan().is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_application(&context);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.session.library().samples[0].id, source);
        assert!(
            app.selected_sample.is_none(),
            "The old ID now identifies another sound."
        );
        assert!(
            app.drag.is_none(),
            "An in-flight drag must not place the replacement sound."
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn adoption_status_and_import_notice_survive_the_next_application_poll() {
        let (context, mut app) = fixture();
        let path =
            std::env::temp_dir().join(format!("mixjam-ui-adoption-{}.json", std::process::id()));
        let mut project = Project::default();
        project.import_notices.push(ImportNotice {
            source_bpm: 146.0,
            target_bpm: 140.0,
        });
        project.save(&path).unwrap();
        app.open_path(&path);
        let status = app.status.clone();
        assert!(status.contains("Opened") && status.contains("146 BPM changed to 140 BPM"));
        app.poll_application(&context);
        assert_eq!(app.status, status);
        app.new_mix();
        let status = app.status.clone();
        app.poll_application(&context);
        assert_eq!(app.status, status);
        std::fs::remove_file(path).unwrap();
    }
}
