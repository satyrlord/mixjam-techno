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
            Session::headless(library, editor.project).unwrap(),
            "UI fixture".into(),
            false,
        );
        app.capture_path = None;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        (context, app)
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
        assert!(labels.contains("1") && labels.contains("64"));
        assert!(!labels.contains("2") && !labels.contains("32"));
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
        let image_center = |output: &egui::FullOutput, icon: RailIcon| {
            output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Mesh(mesh) = &shape.shape
                        && mesh.texture_id == icons.texture_id(icon)
                    {
                        Some(mesh.calc_bounds().center())
                    } else {
                        None
                    }
                })
                .expect("The rail must paint its bundled icon texture.")
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
