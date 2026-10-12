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

    fn assert_folded_assets(ctx: &egui::Context, output: &egui::FullOutput, density: f32, palette: &str) {
        let density_index = [1.0_f32,1.5,2.0].iter().position(|prepared|*prepared==density).expect("Unsupported display density");
        let textures: Vec<_> = ctx.tex_manager().read().allocated()
            .filter(|(_,meta)|meta.name.starts_with(&format!("folded-metal-{palette}-")) && meta.name.ends_with(&format!("-{density_index}")))
            .map(|(id,meta)|(*id,meta.size,meta.name.clone())).collect();
        let dimensions = [(112,64),(112,64),(112,64),(112,64),(192,12),(192,12),(12,96),(12,96),(36,36),(48,24)];
        for (index,(width,height)) in dimensions.into_iter().enumerate() {
            let part=if index<8 {format!("part-{index}")} else if index==8 {"knob".into()} else {"primary".into()};
            let name=format!("folded-metal-{palette}-{part}-{density_index}");
            let expected=[(width as f32*density) as usize,(height as f32*density) as usize];
            assert!(textures.iter().any(|(_,size,key)|*key==name && *size==expected),"{name} must load its prepared density dimensions {expected:?}.");
        }
        let body=match palette {
            "Marine"=>Color32::from_rgb(32,83,91),
            "Granite"=>Color32::from_rgb(52,57,60),
            "Red"=>Color32::from_rgb(154,45,39),
            "Olive"=>Color32::from_rgb(60,68,40),
            "Utility"=>Color32::from_rgb(60,65,64),
            _=>panic!("Unknown chassis palette"),
        };
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Rect(rect) if rect.fill==body && rect.rect.width()>300.0 && rect.rect.height()>100.0)),"The {palette} chassis must retain its assigned body color.");
        fn check(shape:&egui::Shape,textures:&[(egui::TextureId,[usize;2],String)],density:f32,parts:&mut [bool;8]) {
            match shape {
                egui::Shape::Vec(shapes)=>for shape in shapes {check(shape,textures,density,parts);},
                egui::Shape::Mesh(mesh)=>{
                    let Some((_,size,name))=textures.iter().find(|(id,_,_)|*id==mesh.texture_id) else {return;};
                    if let Some(part)=name.split("-part-").nth(1).and_then(|part|part.split('-').next()).and_then(|part|part.parse::<usize>().ok()) {parts[part]=true;}
                    for quad in mesh.vertices.as_chunks::<4>().0 {
                        let mut dest=Rect::NOTHING;let mut uv=Rect::NOTHING;
                        for vertex in quad {
                            dest.extend_with(vertex.pos);uv.extend_with(vertex.uv);
                            for edge in [vertex.pos.x,vertex.pos.y] {assert!((edge*density-(edge*density).round()).abs()<0.001,"Folded Metal raster vertices align to physical pixels.");}
                        }
                        assert!((dest.width()*density-uv.width()*size[0] as f32).abs()<0.001,"Do not stretch material or control raster width.");
                        assert!((dest.height()*density-uv.height()*size[1] as f32).abs()<0.001,"Do not stretch material or control raster height.");
                    }
                }
                _=>{},
            }
        }
        let mut parts=[false;8];
        for shape in &output.shapes {check(&shape.shape,&textures,density,&mut parts);}
        assert!(parts.into_iter().all(|painted|painted),"Paint every corner and edge of the Folded Metal chassis.");
    }

    fn effects_control_rect(output: &egui::FullOutput, name: &str) -> Rect {
        let bounds = accessible_node(output, name).1.bounds().unwrap();
        Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        )
    }

    fn effects_text_mesh_bounds(output: &egui::FullOutput, text: &str) -> Rect {
        output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Text(shape) if shape.galley.job.text == text => {
                Some(shape.galley.mesh_bounds.translate(shape.pos.to_vec2()))
                }
            _ => None,
        }).reduce(|bounds, next| bounds.union(next))
            .unwrap_or_else(|| panic!("Missing glyph mesh: {text}"))
    }

    #[test]
    fn effects_disabled_actions_explain_themselves_on_hover() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        app.show_effects = true;
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for label in ["Listen", "Save sound"] {
            assert!(accessible_node(&output, label).1.is_disabled());
            let center = effects_control_rect(&output, label).center();
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(center)], egui::Modifiers::NONE,);
            let mut shown = false;
            for _ in 0..4 {
                shown |= frame(&ctx, &mut app, vec![], egui::Modifiers::NONE).shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.job.text == "Select an available sound to process it."));
            }
            assert!(shown, "Disabled {label} must show why it is unavailable on hover.");
        }
    }

    #[test]
    fn effects_compact_skin_fits_the_minimum_window_and_collapses_without_losing_settings() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.selected_sample = Some(1);
            app.show_effects = true;
            app.effects.filter = Some(crate::effects::Filter { cutoff_hz: 4567.0, ..Default::default() });
            app.effects.echo = Some(Default::default());
            app.effects.robotize = Some(Default::default());
            app.effects.dirty = 0.375;
            let before = app.effects.clone();
            let size = Vec2::new(960.0, 720.0);
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            let mut output = egui::FullOutput::default();
            for _ in 0..4 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let window = effects_control_rect(&output, "EFFECT STUDIO");
            let panel =window;
            assert!(viewport.contains_rect(window),
                "The complete module must fit at {density}x.");
            let footer = effects_control_rect(&output, "Effect Studio status");
            assert!(window.contains_rect(footer));
            assert_folded_assets(&ctx,&output,density,"Red");
            assert_eq!(accessible_node(&output, "Source sound").1.value(), Some("good grasp 2"));
            for name in ["Source sound", "Cutoff", "Resonance", "Overdrive", "Filter volume",
                "Delay (beats)", "Feedback", "Echo volume", "Color", "Robotize volume",
                "Dirty distortion", "Noise distortion", "Output volume", "Listen", "Save sound", "Reset",] {
                let bounds = effects_control_rect(&output, name);
                assert!(panel.contains_rect(bounds), "{name} must fit the compact skin at {density}x.");
                assert!(!bounds.intersects(footer), "{name} must not overlap status.");
            }
            for text in ["Cutoff", "Resonance", "Overdrive", "Filter volume", "Delay (beats)",
                "Feedback", "Echo volume", "Color", "Robotize volume", "Dirty distortion",
                "Noise distortion", "Output volume", "Listen", "Save sound", "Reset",] {
                assert!(panel.contains_rect(text_rect(&output, text)), "{text} must remain readable at {density}x.");
            }
            for name in ["Cutoff", "Resonance", "Overdrive", "Filter volume", "Delay (beats)",
                "Feedback", "Echo volume", "Color", "Robotize volume", "Dirty distortion",
                "Noise distortion", "Output volume",] {
                let slot = effects_control_rect(&output, &format!("{name} value"));
                let number = output.shapes.iter().filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text.parse::<f64>().is_ok() => {
                        let bounds = text.galley.mesh_bounds.translate(text.pos.to_vec2());
                        slot.contains(bounds.center()).then_some(bounds)
                    }
                    _ => None,
                }).reduce(|bounds, next| bounds.union(next))
                    .unwrap_or_else(|| panic!("Missing numeric glyphs for {name} at {density}x."));
                let caption = effects_text_mesh_bounds(&output, name);
                let gap = caption.top() - number.bottom();
                assert!(gap >= 4.0,
                    "{name} needs 4 logical pixels between number and caption glyphs at {density}x; measured {gap:.3}, number={number:?}, caption={caption:?}.");
            }
            for (section, controls) in [("Robotize", ["Color", "Robotize volume"].as_slice()),
                ("Distortion / output", ["Dirty distortion", "Noise distortion", "Output volume"].as_slice(),),] {
                let heading = effects_text_mesh_bounds(&output, section);
                for name in controls {
                    let knob = effects_control_rect(&output, name).expand(1.0);
                    let mut ticks = 0;
                    for shape in &output.shapes {
                        if let egui::Shape::LineSegment { points, stroke } = &shape.shape
                            && stroke.width <= 1.1
                            && points[0].distance(points[1]) <= 4.0
                            && points.iter().all(|point| knob.contains(*point))
                        {
                            ticks += 1;
                            let bounds = Rect::from_two_pos(points[0], points[1]).expand(stroke.width / 2.0);
                            assert!(!heading.intersects(bounds),
                                "{section} heading glyphs must not touch {name} tick marks at {density}x: heading={heading:?}, tick={bounds:?}.");
                        }
                    }
                    assert!(ticks > 0, "{name} must have native tick marks to check.");
                }
            }
            assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Help")));
            focus_control(&ctx, &mut app, &output, "Collapse Effect Studio");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let collapsed = effects_control_rect(&output, "EFFECT STUDIO");
            assert_eq!(collapsed.min, window.min, "Collapse must stay in place.");
            assert!((collapsed.height() - 44.0).abs() < 0.1);
            assert_eq!(collapsed.width(), window.width());
            for hidden in ["Source sound", "Cutoff", "Listen", "Effect Studio status"] {
                assert!(!output.platform_output.accesskit_update.as_ref().unwrap().nodes.iter()
                    .any(|(_, node)| node.label() == Some(hidden)), "{hidden} must hide when collapsed.");
            }
            focus_control(&ctx, &mut app, &output, "Expand Effect Studio");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(effects_control_rect(&output, "EFFECT STUDIO"), window);
            assert_eq!(app.effects, before, "Collapse must preserve processing settings.");
            focus_control(&ctx, &mut app, &output, "Effect Studio help");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            assert!(app.show_effects_help, "The Help icon must support keyboard activation.");
            app.show_effects_help = false;
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Collapse Effect Studio");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Close Effect Studio");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            assert!(!app.show_effects, "The collapsed module must still close.");
            assert_eq!(app.effects, before);
        }
    }

    #[test]
    fn effects_switches_discard_settings_and_disable_only_their_own_controls() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.show_effects = true;
        app.effects = EffectsState {
            filter: Some(crate::effects::Filter { cutoff_hz: 4567.0, ..Default::default() }),
            echo: Some(crate::effects::Echo { feedback: 0.7, ..Default::default() }),
            robotize: Some(crate::effects::Robotize { frequency_hz: 333.0, ..Default::default() }),
            dirty: 0.25, noise: 0.125, volume: 0.75,
        };
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for name in ["Listen", "Save sound"] {
            assert!(accessible_node(&output, name).1.is_disabled(), "Processing needs a selected source.");
        }
        app.selected_sample = Some(1);
        for (switch, knob) in [("Filter", "Cutoff"), ("Echo", "Feedback"), ("Robotize", "Color"),] {
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert!(!accessible_node(&output, knob).1.is_disabled());
            focus_control(&ctx, &mut app, &output, switch);
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let (target, node) = accessible_node(&output, knob);
            assert!(node.is_disabled());
            assert!(accessible_node(&output, &format!("{knob} value")).1.is_disabled());
            let before = app.effects.clone();
            frame(&ctx, &mut app, vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue, target,
                data: Some(egui::accesskit::ActionData::NumericValue(0.9)),
            },)], egui::Modifiers::NONE,);
            assert_eq!(app.effects, before, "A disabled control must not change effect state.");
            for name in ["Dirty distortion", "Noise distortion", "Output volume", "Listen", "Save sound", "Reset",] {
                assert!(!accessible_node(&output, name).1.is_disabled(), "{name} must remain available.");
            }
            focus_control(&ctx, &mut app, &output, switch);
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        }
        assert_eq!(app.effects.filter, Some(Default::default()));
        assert_eq!(app.effects.echo, Some(Default::default()));
        assert_eq!(app.effects.robotize, Some(Default::default()));
        assert_eq!((app.effects.dirty, app.effects.noise, app.effects.volume), (0.25, 0.125, 0.75));
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Reset");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.effects, EffectsState::default());
        assert_eq!(app.effects_feedback.message.as_deref(), Some("Effects reset. Filter, Echo, and Robotize are off."));
    }

    #[test]
    fn effects_knob_input_is_stable_bounded_and_accessible() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            app.show_effects = true;
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let origin = effects_control_rect(&output, "Output volume").center();
            for _ in 0..12 {
                frame(&ctx, &mut app, vec![egui::Event::PointerMoved(origin)], egui::Modifiers::NONE,);
                assert_eq!(app.effects.volume, 1.0, "Hover must not change a knob.");
            }
            frame(&ctx, &mut app, vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, 20.0), modifiers: egui::Modifiers::NONE,
            }], egui::Modifiers::NONE,);
            assert!((app.effects.volume - 1.01).abs() < 0.0001);
            for _ in 0..20 {
                frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                assert!((app.effects.volume - 1.01).abs() < 0.0001, "One wheel event must apply once.");
            }
            pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
            for offset in [15.0, 30.0, 15.0, 0.0] {
                frame(&ctx, &mut app, vec![egui::Event::PointerMoved(origin + Vec2::new(offset, 0.0))], egui::Modifiers::NONE,);
                let expected = 1.01 + offset / 75.0;
                assert!((app.effects.volume - expected).abs() < 0.0001, "Drag must track motion and reversal at {density}x.");
                for _ in 0..3 {
                    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                    assert!((app.effects.volume - expected).abs() < 0.0001, "A stopped pointer must hold its value.");
                }
            }
            pointer(&ctx, &mut app, origin, false, egui::Modifiers::NONE);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let (target, node) = accessible_node(&output, "Output volume");
            assert_eq!(node.min_numeric_value(), Some(0.0));
            assert_eq!(node.max_numeric_value(), Some(2.0));
            focus_control(&ctx, &mut app, &output, "Output volume");
            for (key, modifiers, expected) in [
                (egui::Key::Home, egui::Modifiers::NONE, 0.0),
                (egui::Key::ArrowUp, egui::Modifiers::NONE, 0.01),
                (egui::Key::ArrowUp, egui::Modifiers::SHIFT, 0.011),
                (egui::Key::End, egui::Modifiers::NONE, 2.0),
                (egui::Key::ArrowRight, egui::Modifiers::NONE, 2.0),
            ] {
                frame(&ctx, &mut app, vec![key_event(key, modifiers)], modifiers);
                assert!((app.effects.volume - expected).abs() < 0.0001, "Keyboard adjustment must respect the range.");
            }
            frame(&ctx, &mut app, vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue, target,
                data: Some(egui::accesskit::ActionData::NumericValue(50.0)),
            },)], egui::Modifiers::NONE,);
            assert_eq!(app.effects.volume, 2.0);
            frame(&ctx, &mut app, vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Decrement, target, data: None,
            },)], egui::Modifiers::NONE,);
            assert!((app.effects.volume - 1.99).abs() < 0.0001);
            click(&ctx, &mut app, origin, egui::Modifiers::NONE);
            click(&ctx, &mut app, origin, egui::Modifiers::NONE);
            assert_eq!(app.effects.volume, 1.0, "Double-click must restore the control default.");
        }
    }

    #[test]
    fn effects_logarithmic_knobs_step_along_the_dial() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.show_effects = true;
        app.effects.filter = Some(Default::default());
        app.effects.robotize = Some(crate::effects::Robotize { frequency_hz: 1.0, ..Default::default() });
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let (target, _) = accessible_node(&output, "Cutoff");
        focus_control(&ctx, &mut app, &output, "Cutoff");
        frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.effects.filter.as_ref().unwrap().cutoff_hz, 2143.0, "A key must turn Cutoff 1% of its dial.");
        frame(&ctx, &mut app, vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Decrement, target, data: None,
        },)], egui::Modifiers::NONE,);
        assert_eq!(app.effects.filter.as_ref().unwrap().cutoff_hz, 2000.0);
        for key in [egui::Key::End, egui::Key::ArrowUp] {
            frame(&ctx, &mut app, vec![key_event(key, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        }
        assert_eq!(app.effects.filter.as_ref().unwrap().cutoff_hz, 20000.0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Color");
        frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.effects.robotize.as_ref().unwrap().frequency_hz, 2.0, "Rounding must not hold the low end in place.");
    }

    #[test]
    fn effects_missing_audio_keeps_processing_disabled_and_details_reachable() {
        let (ctx, mut app) = missing_fixture();
        ctx.enable_accesskit();
        app.select_clip(1, false);
        app.show_effects = true;
        for _ in 0..3 { frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, Vec2::new(960.0, 720.0),); }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, Vec2::new(960.0, 720.0),);
        for name in ["Listen", "Save sound"] { assert!(accessible_node(&output, name).1.is_disabled()); }
        assert!(effects_control_rect(&output, "Effect Studio status").contains_rect(effects_control_rect(&output, "Import details")));
        text_rect(&output, "Missing audio. Open Import details to resolve the sound.",);
        focus_control(&ctx, &mut app, &output, "Import details");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(app.show_import_details);
    }

    #[test]
    fn effects_save_feedback_preserves_source_tempo_and_selects_the_saved_effect() {
        let (ctx, mut app) = fixture();
        let dir = std::env::temp_dir().join(format!("mixjam-ui-effects-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut sample = app.session.library().samples[0].clone();
        sample.source_bpm = 160.0;
        app.session = Session::headless(Arc::new(Library::fixture(vec![sample])), app.session.project().clone(),)
            .unwrap().with_settings_root(dir.join("settings"));
        app.selected_sample = Some(1);
        app.show_effects = true;
        app.effects.dirty = 0.25;
        let before = app.effects.clone();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let source_bpm = app.source_bpm();
        let request = || FileRequest::SaveSound {
            pcm: Pcm { sample_rate: 1000, channels: 1, samples: vec![0.25; 1000], },
            name: "effect result".into(), group: "Effect", source_bpm, stretch: None,
        };
        app.finish_path(request(), None);
        assert_eq!(app.effects_feedback.message.as_deref(), Some("Save cancelled."));
        assert!(!app.effects_feedback.error);
        assert_eq!(app.selected_sample, Some(1));
        let path = dir.join("result.wav");
        app.finish_path(request(), Some(path.clone()));
        let saved = app.selected_sample.expect("Saving must select the new Effect sound.");
        assert_ne!(saved, 1);
        assert_eq!(app.group.as_deref(), Some("Effect"));
        assert_eq!(app.source_bpm(), 160.0);
        assert_eq!(app.effects, before);
        assert_eq!(app.effects_feedback.source, Some(saved));
        assert_eq!(app.effects_feedback.message.as_deref(), Some("Saved result.wav to Effect."));
        assert!(!app.effects_feedback.error);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "Saved result.wav to Effect.");
        assert_eq!(crate::audio::read_wav(&path).unwrap().frames(), 1000);
        app.finish_path(request(), Some(dir.join("absent-parent/result.wav")));
        assert!(app.effects_feedback.error);
        assert!(app.effects_feedback.message.as_deref().unwrap().starts_with("Cannot save the sound:"));
        assert_eq!(app.selected_sample, Some(saved));
        assert_eq!(app.session.library().samples.len(), 2, "A failed save must not add another source.");
        assert_eq!(app.effects, before);
        app.selected_sample = Some(1);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.effects_feedback.source, Some(1));
        assert!(app.effects_feedback.message.is_none(), "Changing source must clear the previous save receipt.");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stretch_hover_does_not_change_tempo_and_wheel_input_changes_it_once() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.selected_sample = Some(1);
        app.show_stretch = true;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for label in ["Original BPM knob", "Target BPM knob"] {
            let bounds = accessible_node(&output, label).1.bounds().unwrap();
            let position = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,);
            let before = (app.stretch_source_bpm, app.stretch_target_bpm);
            for _ in 0..12 {
                output = frame(&ctx, &mut app, vec![egui::Event::PointerMoved(position)], egui::Modifiers::NONE,);
                assert_eq!((app.stretch_source_bpm, app.stretch_target_bpm), before, "Hovering {label} must not change its value.");
            }
            frame(&ctx, &mut app, vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, 20.0),
                modifiers: egui::Modifiers::NONE,
            }], egui::Modifiers::NONE,);
            let expected = if label == "Original BPM knob" { (141.0, 140.0) } else { (141.0, 160.0) };
            assert_eq!((app.stretch_source_bpm, app.stretch_target_bpm), expected);
            for _ in 0..20 {
                output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                assert_eq!((app.stretch_source_bpm, app.stretch_target_bpm), expected, "A wheel event must not keep changing {label} on later frames.");
            }
        }
    }

    #[test]
    fn stretch_knob_drag_tracks_total_motion_and_holds_when_pointer_stops() {
        for density in [1.0, 1.5, 2.0] {
            for target in [false, true] {
                let (ctx, mut app) = fixture();
                ctx.set_pixels_per_point(density);
                ctx.enable_accesskit();
                app.selected_sample = Some(1);
                app.show_stretch = true;
                for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
                let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                let label = if target { "Target BPM knob" } else { "Original BPM knob" };
                let bounds = accessible_node(&output, label).1.bounds().unwrap();
                let origin = Pos2::new(
                    (bounds.x0 + bounds.x1) as f32 / 2.0,
                    (bounds.y0 + bounds.y1) as f32 / 2.0,
                );
                let scale = if target { 2.5 } else { 150.0 / 270.0 };
                let value = |app: &MixJamApp| { if target { app.stretch_target_bpm } else { app.stretch_source_bpm
                    } };
                // Pause and reverse within the same gesture, then start a new one.
                for (start, offsets) in [(origin, [12.0, 24.0, 36.0, 24.0, 0.0]), (origin, [-12.0, -24.0, -36.0, -24.0, 0.0]),] {
                    pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
                    for (index,offset) in offsets.into_iter().enumerate() {
                        let position = start + Vec2::new(offset * scale, 0.0);
                        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(position)], egui::Modifiers::NONE,);
                        let expected = if !target {140.0+f64::from(offset)}
                            else if offsets[0]>0.0 {[160.0,160.0,180.0,160.0,140.0][index]}
                            else {[120.0,120.0,120.0,140.0,160.0][index]};
                        assert_eq!(value(&app), expected, "{label} at {density}x, offset {offset}");
                        for _ in 0..3 {
                            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                            assert_eq!(value(&app), expected, "Holding {label} still must retain its value.");
                        }
                    }
                    pointer(&ctx, &mut app, start, false, egui::Modifiers::NONE);
                    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                    assert_eq!(value(&app),if target && offsets[0]<0.0 {160.0} else {140.0},"Reversing away from a hard stop must immediately move the shared dial.");
                }
            }
        }
    }

    #[test]
    fn stretch_opens_with_source_tempo_and_accepts_accessible_adjustments() {
        let (ctx, mut app) = fixture();
        let mut sample = app.session.library().samples[0].clone();
        sample.source_bpm = 160.0;
        app.session = Session::headless(Arc::new(Library::fixture(vec![sample])), app.session.project().clone(),).unwrap();
        app.selected_sample = Some(1);
        ctx.enable_accesskit();
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let bounds = accessible_node(&output, "Stretch").1.bounds().unwrap();
        click(&ctx, &mut app, Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,), egui::Modifiers::NONE,);
        assert!(app.show_stretch);
        assert_eq!(app.stretch_source_bpm, 160.0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let (node_id, node) = accessible_node(&output, "Target BPM knob");
        assert_eq!(node.min_numeric_value(), Some(120.0));
        assert_eq!(node.max_numeric_value(), Some(180.0));
        frame(&ctx, &mut app, vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::SetValue,
            target: node_id,
            data: Some(egui::accesskit::ActionData::NumericValue(155.0)),
        },)], egui::Modifiers::NONE,);
        assert_eq!(app.stretch_target_bpm, 160.0);
        frame(&ctx, &mut app, vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Decrement,
            target: node_id,
            data: None,
        },)], egui::Modifiers::NONE,);
        assert_eq!(app.stretch_target_bpm, 140.0);
    }

    #[test]
    fn stretch_artwork_uses_one_texel_per_physical_pixel_at_all_prepared_densities() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            app.show_stretch = true;
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_folded_assets(&ctx,&output,density,"Marine");
        }
    }

    #[test]
    fn stretch_header_controls_use_corner_positions_and_a_complete_collapsed_skin() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            app.show_stretch = true;
            app.stretch_source_bpm = 160.0;
            app.stretch_target_bpm = 120.0;
            for _ in 0..4 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let expanded = effects_control_rect(&output, "TIME STRETCHER");
            for (name, x) in [("Collapse Time Stretcher", 24.0),
                ("Close Time Stretcher", expanded.width() - 24.0),] {
                assert!(
                    (effects_control_rect(&output, name).center().x - expanded.left() - x).abs() <= 1.0 / density);
            }
            assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Help")));
            focus_control(&ctx, &mut app, &output, "Collapse Time Stretcher");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            for _ in 0..4 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let collapsed = effects_control_rect(&output, "TIME STRETCHER");
            assert_eq!(collapsed.min, expanded.min);
            assert_eq!(collapsed.width(), expanded.width());
            assert!((collapsed.height() - 44.0).abs() < 0.1);
            for hidden in ["Source sound", "Listen to stretched sound", "Time Stretcher status",] {
                assert!(!output.platform_output.accesskit_update.as_ref().unwrap().nodes.iter()
                    .any(|(_, node)| node.label() == Some(hidden)), "{hidden} must hide when collapsed.");
            }
            focus_control(&ctx, &mut app, &output, "Expand Time Stretcher");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            for _ in 0..4 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(effects_control_rect(&output, "TIME STRETCHER"), expanded);
            assert_eq!((app.stretch_source_bpm, app.stretch_target_bpm), (160.0, 120.0));
            focus_control(&ctx, &mut app, &output, "Time Stretcher help");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            assert!(app.show_stretch_help, "The help icon must support keyboard activation.");
            app.show_stretch_help = false;
            ctx.data_mut(|data| { data.insert_temp(egui::Id::new(("hardware-collapsed", "Time Stretcher")), true)
            });
            for _ in 0..4 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Close Time Stretcher");
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            assert!(!app.show_stretch, "Close must work in the collapsed state.");
        }
    }

    #[test]
    fn musical_tools_collapsed_help_icons_preserve_processing_drafts() {
        for (tool, title, collapse, help, expand, collapse_id, height) in [
            ("stretch", "TIME STRETCHER", "Collapse Time Stretcher", "Time Stretcher help",
                "Expand Time Stretcher",
                "Time Stretcher",
                44.0,),
            ("effects", "EFFECT STUDIO", "Collapse Effect Studio", "Effect Studio help",
                "Expand Effect Studio",
                "Effect Studio",
                44.0,),
            ("step", "STEP SYNTH", "Collapse Step synth", "Step synth help",
                "Expand Step synth",
                "Step synth",
                44.0,),
        ] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.selected_sample = Some(1);
            app.stretch_source_bpm = 160.0;
            app.stretch_target_bpm = 120.0;
            app.effects.echo = Some(Default::default());
            app.effects.dirty = 0.375;
            app.step_synth.steps[19] = Some(-7);
            app.step_synth.active_bars = [true, false, true, false];
            app.step_synth.bpm = 160.0;
            match tool {
                "stretch" => app.show_stretch = true,
                "effects" => app.show_effects = true,
                "step" => app.show_step_synth = true,
                _ => unreachable!(),
            }
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let before = (app.effects.clone(), app.step_synth.clone(), app.stretch_source_bpm,
                app.stretch_target_bpm, app.source_id(), app.session.project().clone(),);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, collapse);
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let window = effects_control_rect(&output, title);
            assert!((window.height() - height).abs() < 0.1, "{title} must be collapsed.");
            assert!(window.contains_rect(effects_control_rect(&output, help)),
                "The collapsed header must keep {help} available.");
            assert!(!accessible_node(&output, help).1.is_disabled());
            focus_control(&ctx, &mut app, &output, help);
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            accessible_node(&output, &title_text(help));
            assert!(match tool {
                "stretch" => app.show_stretch && app.show_stretch_help,
                "effects" => app.show_effects && app.show_effects_help,
                "step" => app.show_step_synth && app.show_step_synth_help,
                _ => unreachable!(),
            });
            assert_eq!(ctx.data(|data| data.get_temp::<bool>(egui::Id::new(("hardware-collapsed",collapse_id)))), Some(true));
            accessible_node(&output, expand);
            focus_control(&ctx, &mut app, &output, &format!("Close {help}"));
            frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert!((effects_control_rect(&output, title).height() - height).abs() < 0.1,
                "Closing {help} must return to the collapsed tool.");
            assert_eq!((app.effects.clone(), app.step_synth.clone(), app.stretch_source_bpm,
                app.stretch_target_bpm, app.source_id(), app.session.project().clone()), before,
                "Opening and closing {help} must preserve the source, all processing drafts, and song.");
            assert!(!app.session.is_previewing() && !app.session.wants_playback() && !app.session.is_stretching());
        }
    }

    #[test]
    fn stretch_rotaries_accept_keyboard_input_and_keep_supported_target_tempos() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.selected_sample = Some(1);
        app.show_stretch = true;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Original BPM knob");
        frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.stretch_source_bpm, 141.0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Target BPM knob");
        let focused_knob = ctx.memory(|memory| memory.focused());
        frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowRight, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.stretch_target_bpm, 160.0);
        assert_eq!(ctx.memory(|memory| memory.focused()), focused_knob, "Arrow keys must keep focus on the target knob.");
        frame(&ctx, &mut app, vec![key_event(egui::Key::End, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.stretch_target_bpm, 180.0);
        frame(&ctx, &mut app, vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(!app.show_stretch);
    }

    #[test]
    fn stretch_escape_closes_an_open_list_or_a_window_above_before_the_tool() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        // Test frames share one instant; a closing window must not keep fading.
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.selected_sample = Some(1);
        app.show_stretch = true;
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let bounds = accessible_node(&output, "Source sound").1.bounds().unwrap();
        click(&ctx, &mut app, Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,), egui::Modifiers::NONE,);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(egui::Popup::is_any_open(&ctx));
        frame(&ctx, &mut app, vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(app.show_stretch, "Esc must close the source list, not the tool.");
        for _ in 0..2 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        assert!(!egui::Popup::is_any_open(&ctx));
        // The tool's own Help opens above it and takes Esc first.
        app.show_stretch_help = true;
        for _ in 0..2 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        frame(&ctx, &mut app, vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(app.show_stretch, "Esc must not close the tool under another window.");
        app.show_stretch_help = false;
        for _ in 0..2 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        frame(&ctx, &mut app, vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(!app.show_stretch);
    }

    #[test]
    fn stretch_idle_status_returns_after_a_preview_and_marks_unheard_settings() {
        for (audition, original, expected) in [
            (None, 160.0, "Duration +33%. Pitch preservation is approximate. Listen before saving.",),
            (Some((1, 160.0, 120.0)), 160.0, "Duration +33%. Pitch preservation is approximate.",),
            (Some((1, 160.0, 120.0)), 150.0, "Duration +25%. Pitch preservation is approximate. Listen before saving.",),
            (Some((1, 160.0, 120.0)), 120.0, "Same tempo. Duration is unchanged.",),
        ] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            let (mut original, mut target, mut open) = (original, 120.0, true);
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    focused: true, ..Default::default()
                }, |ctx| {
                    stretch::show(ctx, &mut open,
                        stretch::Source { name: "Sound", id: Some(1), samples: &[], missing: false, bpm: 160.0, },
                        stretch::Feedback { processing: false, listening: false, saved: false, audition, message: None, error: false, },
                        &mut original, &mut target,).unwrap();
                },);
            }
            text_rect(&output, expected);
        }
    }

    #[test]
    fn stretch_actions_fit_supported_windows_and_require_an_available_source() {
        for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0), Vec2::new(1440.0, 1000.0),] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.show_stretch = true;
            app.selected_sample = Some(1);
            for _ in 0..3 { frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            for text in ["TIME STRETCHER", "Original BPM", "Target BPM", "Listen", "Save stretched sound", "Same tempo. Duration is unchanged.",] {
                let text_bounds = text_rect(&output, text);
                assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(text_bounds), "{text} outside {size:?}: {text_bounds:?}");
            }
            for label in ["Listen to stretched sound", "Save stretched sound"] {
                assert!(!accessible_node(&output, label).1.is_disabled());
            }
            app.selected_sample = None;
            app.selected_clip = None;
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            for label in ["Listen to stretched sound", "Save stretched sound"] {
                assert!(accessible_node(&output, label).1.is_disabled());
            }
        }
    }

    #[test]
    fn stretch_source_names_stay_bounded_and_search_selects_the_matching_sound() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            let mut sample = app.session.library().samples[0].clone();
            sample.name = format!("{} distinctive ending.wav", "Long source name ".repeat(40));
            let mut other = sample.clone();
            other.id = 2;
            other.name = "Unique searchable sound".into();
            other.source_bpm = 160.0;
            app.session = Session::headless(Arc::new(Library::fixture(vec![sample, other])), app.session.project().clone(),).unwrap();
            app.selected_sample = Some(1);
            app.show_stretch = true;
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let bounds = accessible_node(&output, "Source sound").1.bounds().unwrap();
            let window = accessible_node(&output, "TIME STRETCHER").1.bounds().unwrap();
            assert!(bounds.x0 >= window.x0 && bounds.x1 <= window.x1);
            assert!(bounds.x1 - bounds.x0 <= 620.0, "Long source label expanded the slot: {bounds:?}");
            click(&ctx, &mut app, Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,), egui::Modifiers::NONE,);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Find source sound");
            let output = frame(&ctx, &mut app, vec![egui::Event::Text("unique".into())], egui::Modifiers::NONE,);
            let (_, node) = accessible_node(&output, "Unique searchable sound");
            let bounds = node.bounds().unwrap();
            assert!(bounds.x1 - bounds.x0 <= 620.0);
            click(&ctx, &mut app, Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,), egui::Modifiers::NONE,);
            assert_eq!(app.selected_sample, Some(2));
            assert_eq!(app.stretch_source_bpm, 160.0);
            assert!(!egui::Popup::is_any_open(&ctx));
        }
    }

    #[test]
    fn stretch_target_pointer_spans_the_full_arc_and_original_reset_uses_source_tempo() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        let mut sample = app.session.library().samples[0].clone();
        sample.source_bpm = 160.0;
        app.session = Session::headless(Arc::new(Library::fixture(vec![sample])), app.session.project().clone(),).unwrap();
        app.selected_sample = Some(1);
        app.show_stretch = true;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for (bpm, angle) in [(120.0, -225.0_f32), (140.0, -135.0), (160.0, -45.0), (180.0, 45.0),] {
            app.stretch_target_bpm = bpm;
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let bounds = accessible_node(&output, "Target BPM knob").1.bounds().unwrap();
            let center = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,);
            let expected = center + Vec2::angled(angle.to_radians()) * (18.0 * 0.62);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment { points, stroke } if (stroke.width - 2.0).abs() < 0.01 && points[1].distance(expected) < 0.05)), "No pointer at {bpm} BPM / {angle} degrees");
        }
        app.stretch_source_bpm = 100.0;
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let bounds = accessible_node(&output, "Original BPM knob").1.bounds().unwrap();
        let center = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,);
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        assert_eq!(app.stretch_source_bpm, 160.0);
    }

    #[test]
    fn stretch_shift_changes_only_new_drag_motion() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.selected_sample = Some(1);
        app.show_stretch = true;
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let bounds = accessible_node(&output, "Original BPM knob").1.bounds().unwrap();
        let origin = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,);
        pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(origin + Vec2::new(12.0 * 150.0 / 270.0, 0.0),)], egui::Modifiers::NONE,);
        assert_eq!(app.stretch_source_bpm, 152.0);
        frame(&ctx, &mut app, vec![], egui::Modifiers::SHIFT);
        assert_eq!(app.stretch_source_bpm, 152.0);
        let end = origin + Vec2::new(24.0 * 150.0 / 270.0, 0.0);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(end)], egui::Modifiers::SHIFT,);
        assert_eq!(app.stretch_source_bpm, 153.0);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.stretch_source_bpm, 153.0);
        pointer(&ctx, &mut app, end, false, egui::Modifiers::NONE);
    }

    #[test]
    fn stretch_panel_exposes_busy_preview_saved_and_error_states() {
        for (processing, listening, saved, message, error) in [
            (true, false, false, Some("Processing sound..."), false),
            (false, true, false, None, false),
            (false, false, true, Some("Saved result.wav."), false),
            (false, false, false, Some("Audio output is unavailable."), true,),
        ] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.style_mut(|style| style.animation_time = 0.0);
            ctx.enable_accesskit();
            let mut original = 160.0;
            let mut target = 120.0;
            let mut open = true;
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    focused: true, ..Default::default()
                }, |ctx| {
                    stretch::show(ctx, &mut open,
                        stretch::Source { name: "Sound", id: Some(1), samples: &[], missing: false, bpm: 160.0, },
                        stretch::Feedback { processing, listening, saved, audition: Some((1, 160.0, 120.0)), message, error, },
                        &mut original, &mut target,).unwrap();
                },);
            }
            let action_name = if listening { "Stop listening" } else { "Listen to stretched sound" };
            assert_eq!(accessible_node(&output, action_name).1.is_disabled(), processing);
            assert_eq!(accessible_node(&output, "Save stretched sound").1.is_disabled(), processing);
            if let Some(message) = message { text_rect(&output, message); }
            if saved {
                text_rect(&output, "Saved");
            }
            if processing {
                text_rect(&output, "Processing..."); }
            if listening {
                text_rect(&output, "Stop listening");
        }
    }
    }

    fn stretch_layout_output(density: f32, missing: bool, feedback: stretch::Feedback<'_>,) -> egui::FullOutput {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.set_pixels_per_point(density);
        ctx.style_mut(|style| style.animation_time = 0.0);
        ctx.enable_accesskit();
        let mut original = 160.0;
        let mut target = 120.0;
        let mut open = true;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                focused: true, ..Default::default()
            }, |ctx| {
                stretch::show(ctx, &mut open,
                    stretch::Source { name: "Sound", id: Some(1), samples: &[], missing, bpm: 160.0, },
                    stretch::Feedback {
                        processing: feedback.processing, listening: feedback.listening,
                        saved: feedback.saved, audition: feedback.audition,
                        message: feedback.message, error: feedback.error,
                    },
                    &mut original, &mut target,).unwrap();
            },);
        }
        output
    }

    #[test]
    fn stretch_action_labels_remain_centered_in_every_playback_state_and_density() {
        for density in [1.0, 1.5, 2.0] {
            for (processing, listening, saved, listen_text) in [
                (false, false, false, "Listen"),
                (false, true, false, "Stop listening"),
                (true, false, false, "Processing..."),
                (false, false, true, "Listen"),
            ] {
                let output = stretch_layout_output(density, false, stretch::Feedback {
                    processing, listening, saved, audition: Some((1, 160.0, 120.0)),
                    message: None, error: false,
                },);
                for (accessible_name, visible_text) in [
                    (if listening { "Stop listening" } else { "Listen to stretched sound" }, listen_text,),
                    ("Save stretched sound", "Save stretched sound"),
                ] {
                    let bounds = accessible_node(&output, accessible_name).1.bounds().unwrap();
                    let button = Rect::from_min_max(
                        Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                        Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
                    );
                    // Measure visible glyph meshes, including the one-pixel shadow.
                    let painted = output.shapes.iter().filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == visible_text => {
                            Some(text.visual_bounding_rect())
                            }
                        _ => None,
                    }).reduce(|bounds, text| bounds.union(text)).unwrap_or_else(|| panic!("Missing action label: {visible_text}"));
                    assert!(button.contains_rect(painted), "{visible_text} leaves its button at {density}x: {painted:?} / {button:?}");
                    let offset =painted.center() - (button.center() - Vec2::new(0.0, 1.0));
                    assert! (offset.x.abs() <= 1.0 &&offset.y.abs() <= 3.0,
                        "Shared button label must stay centered: {offset:?}");
                }
            }
        }
    }

    #[test]
    fn stretch_status_and_error_messages_are_confined_to_the_footer_below_the_skin() {
        let long_preview = "Listening: 160 to 120 BPM. Settings changed; listen again. ".repeat(20);
        let long_save = format!("Saved {}result.wav.", "long folder name/".repeat(60));
        for density in [1.0, 1.5, 2.0] {
            for (processing, listening, saved, message, error, missing) in [
                (false, false, false, None, false, false),
                (true, false, false, Some("Processing sound..."), false, false,),
                (false, true, false, Some(long_preview.as_str()), false, false,),
                (false, false, true, Some(long_save.as_str()), false, false),
                (false, false, false, Some("Audio output is unavailable."), true, false,),
                (false, false, false, None, false, true),
            ] {
                let output = stretch_layout_output(density, missing, stretch::Feedback {
                    processing, listening, saved, audition: Some((1, 160.0, 120.0)), message, error,
                },);
                let bounds = accessible_node(&output, "Time Stretcher status").1.bounds().unwrap();
                let footer = Rect::from_min_max(
                    Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                    Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
                );
                let window = effects_control_rect(& output, "TIME STRETCHER");
                let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0));
                assert!(viewport.contains_rect(window) && window.contains_rect(footer));
                for name in [if listening { "Stop listening" } else { "Listen to stretched sound" }, "Save stretched sound",] {
                    assert!(accessible_node(&output, name).1.bounds().unwrap().y1 < f64::from(footer.top()), "Footer overlaps {name}");
                }
                let status = if missing {
                    "Missing audio. Open Import details to resolve the sound."
                } else {
                    message.unwrap_or("Duration +33%. Pitch preservation is approximate.")
                };
                let painted = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == status => { Some((shape, text))
                        }
                    _ => None,
                }).unwrap_or_else(|| panic!("Missing footer message: {status}"));
                let text_bounds = painted.1.visual_bounding_rect();
                assert!(footer.contains_rect(painted.0.clip_rect), "Message clip leaves its footer: {:?} / {footer:?}", painted.0.clip_rect);
                assert!(footer.contains_rect(text_bounds.intersect(painted.0.clip_rect)), "Visible status leaves its footer");
                if status.len() > 200 {
                    assert!(painted.1.galley.rows.len() > 1, "Long messages must wrap");
                    assert!(text_bounds.bottom() > painted.0.clip_rect.bottom(), "Long message must be clipped to its scroll region");
                } else {
                    let offset = (text_bounds.center() - painted.0.clip_rect.center()) * density;
                    assert!(offset.x.abs() <= 1.0 && offset.y.abs() <= 1.0,
                        "Footer message is not visibly centered at {density}x: {offset:?}");
                }
                if missing {
                    let details = accessible_node(&output, "Import details").1.bounds().unwrap();
                    let details = Rect::from_min_max(Pos2::new(details.x0 as f32, details.y0 as f32), Pos2::new(details.x1 as f32, details.y1 as f32),);
                    assert!(footer.contains_rect(details), "Recovery control leaves its footer");
                    assert!((details.center().y - footer.center().y).abs() * density <= 1.0);
                    let text = output.shapes.iter().find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == "Import details" => { Some(text.visual_bounding_rect())
                            }
                        _ => None,
                    }).unwrap();
                    assert!((text.center() - (details.center()-Vec2::new(0.0,1.0))).length() <= 3.0,"Shared button text must be centered in its raised face.");
                    assert!(painted.0.clip_rect.right() < details.left(), "Missing audio message overlaps recovery control");
                }
            }
        }
    }

    #[test]
    fn stretch_async_errors_are_local_and_changed_or_closed_requests_do_not_open_a_picker() {
        let (ctx, mut app) = fixture();
        app.selected_sample = Some(1);
        app.show_stretch = true;
        for change in [0, 1, 2] {
            app.show_stretch = true;
            app.start_stretch(StretchPurpose::Save);
            assert!(app.stretch_feedback.pending.is_some());
            if change == 1 { app.stretch_target_bpm = 180.0; }
            if change == 2 { app.show_stretch = false; }
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while app.stretch_feedback.pending.is_some() && std::time::Instant::now() < deadline {
                app.poll_application(&ctx);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(app.stretch_feedback.pending.is_none());
            assert!(app.path_prompt.is_none());
            if change == 0 {
                assert!(app.stretch_feedback.error);
                assert!(app.stretch_feedback.message.as_ref().is_some_and(|message| !message.is_empty()));
            } else {
                assert!(!app.stretch_feedback.error);
                assert_eq!(app.stretch_feedback.message.as_deref(), Some("Settings changed. Process the sound again."));
            }
        }
    }

    #[test]
    fn stretch_save_confirmation_tracks_success_cancel_failure_and_output_tempo() {
        let (_, mut app) = fixture();
        let dir = std::env::temp_dir().join(format!("mixjam-ui-stretch-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        app.selected_sample = Some(1);
        let settings = StretchSettings { source_id: 1, source_bpm: 160.0, target_bpm: 120.0, };
        let request = || FileRequest::SaveSound {
            pcm: Pcm { sample_rate: 1000, channels: 1, samples: vec![0.25; 1000], },
            name: "stretch result".into(), group: "Wave", source_bpm: settings.target_bpm, stretch: Some(settings),
        };
        app.finish_path(request(), Some(dir.join("result.wav")));
        assert!(!app.stretch_feedback.error);
        // A save is a receipt: the tool keeps its source and both tempos.
        assert_eq!(app.selected_sample, Some(1));
        assert_eq!((app.stretch_source_bpm, app.stretch_target_bpm), (140.0, 140.0));
        assert_eq!(app.stretch_feedback.saved, Some(settings));
        assert_eq!(app.stretch_feedback.message.as_deref(), Some("Saved result.wav to Wave."));
        let saved: Vec<_> = app.session.library().samples.iter().filter(|sample| sample.id != 1).map(|sample| sample.id).collect();
        assert_eq!(saved.len(), 1);
        assert_eq!(app.session.source_bpm(Some(saved[0])), 120.0);
        app.finish_path(request(), None);
        assert_eq!(app.stretch_feedback.message.as_deref(), Some("Save cancelled."));
        assert!(app.stretch_feedback.saved.is_none());
        app.finish_path(request(), Some(dir.join("absent-parent/result.wav")));
        assert!(app.stretch_feedback.error);
        assert!(app.stretch_feedback.saved.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stretch_stop_and_close_reopen_discard_successful_pending_results() {
        let (ctx, mut app, _port) = agent_fixture();
        let dir = std::env::temp_dir().join(format!("mixjam-ui-stretch-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.wav");
        Pcm { sample_rate: 1000, channels: 1, samples: vec![0.25; 1000], }.write_wav(&path).unwrap();
        let source = app.register_file(&path, "Wave", 140.0).unwrap();
        for action in [0, 1, 2] {
            app.selected_sample = Some(source);
            app.show_stretch = true;
            app.start_stretch(if action == 2 { StretchPurpose::Save } else { StretchPurpose::Listen });
            if action == 0 {
                app.stop();
            } else {
                app.show_stretch = false;
                app.close_stretch_request();
                app.show_stretch = true;
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while (app.session.is_stretching() || app.stretch_feedback.pending.is_some()) && std::time::Instant::now() < deadline {
                app.poll_application(&ctx);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(!app.session.is_stretching());
            assert!(app.stretch_feedback.pending.is_none());
            assert!(app.path_prompt.is_none(), "An abandoned Save must not open a picker.");
            assert!(app.stretch_feedback.audition.is_none());
            assert!(!app.stretch_feedback.error, "An abandoned Listen must not attempt audio output.");
            assert_eq!(app.stretch_feedback.message.as_deref(), Some(if action == 0 { "Preview cancelled by another playback action." } else { "Processing request cancelled." }));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stretch_preview_failure_is_a_local_error_that_begins_with_cannot() {
        let (ctx, mut app, _port) = agent_fixture();
        let dir = std::env::temp_dir().join(format!("mixjam-ui-stretch-listen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.wav");
        Pcm { sample_rate: 1000, channels: 1, samples: vec![0.25; 1000], }.write_wav(&path).unwrap();
        app.selected_sample = Some(app.register_file(&path, "Wave", 140.0).unwrap());
        app.show_stretch = true;
        app.start_stretch(StretchPurpose::Listen);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while app.stretch_feedback.pending.is_some() && std::time::Instant::now() < deadline {
            app.poll_application(&ctx);
            std::thread::sleep(Duration::from_millis(1));
        }
        // The headless session has no audio output.
        assert!(app.stretch_feedback.error);
        assert!(app.stretch_feedback.audition.is_none());
        assert_eq!(app.stretch_feedback.message.as_deref(), Some("Cannot play the preview: Audio output is unavailable."));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stretch_recovery_control_clears_action_border_and_context_help_starts_with_stretching() {
        for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0), Vec2::new(1440.0, 1000.0),] {
            let (ctx, mut app) = missing_fixture();
            ctx.enable_accesskit();
            app.selected_sample = Some(app.session.project().missing_samples[0].id);
            app.show_stretch = true;
            for _ in 0..3 { frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let details = accessible_node(&output, "Import details").1.bounds().unwrap();
            let save = accessible_node(&output, "Save stretched sound").1.bounds().unwrap();
            let footer = accessible_node(&output, "Time Stretcher status").1.bounds().unwrap();
            assert!(details.y0 > save.y1, "Recovery control overlaps the Save border at {size:?}: {details:?} / {save:?}");
            assert!(details.x0 >= footer.x0 && details.x1 <= footer.x1 && details.y0 >= footer.y0 && details.y1 <= footer.y1,
                "Recovery control leaves the status footer at {size:?}: {details:?} / {footer:?}");
            let help = accessible_node(&output, "Time Stretcher help").1.bounds().unwrap();
            let position = Pos2::new((help.x0 + help.x1) as f32 / 2.0, (help.y0 + help.y1) as f32 / 2.0,);
            pointer(&ctx, &mut app, position, true, egui::Modifiers::NONE);
            pointer(&ctx, &mut app, position, false, egui::Modifiers::NONE);
            assert!(app.show_stretch_help);
            for _ in 0..3 { frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let label = "Original BPM describes the recording. Target BPM sets the result to 120, 140, 160, or 180 BPM.";
            let bounds = text_rect(&output, label);
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds));
        }
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
        pointer(&context, &mut app, version.center(), true, egui::Modifiers::NONE,);
        let output = frame(&context, &mut app, vec![egui::Event::PointerButton {
            pos: version.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }], egui::Modifiers::NONE,);
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
                egui::Shape::Text(text) if text.galley.job.text == "Find a sound" => Some(text),
                _ => None,
            })
            .expect("The sound search must keep its body font.");
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
                    tile = sample_button(ui, sample, Some(Product::T2), selected, 148.0).rect;
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
            const ACTIONS: [&str; 11] = [
                "Place sound", "Preview", "Move / copy", "Select group", "Fine snap", "Delete",
                "Seek / loop", "Play / pause", "Undo / redo", "Open / save", "Export",
            ];
            let mut seen_actions = BTreeSet::new();
            let inspect_actions = |output: &egui::FullOutput, seen: &mut BTreeSet<String>| {
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape
                        && ACTIONS.contains(&text.galley.job.text.as_str())
                    {
                        let action = &text.galley.job.text;
                        assert_eq!(text.galley.rows.len(), 1,
                            "Help action {action:?} must stay on one line at {size:?}.");
                        assert!((action.chars().count() as f32 * 3.0..=160.0).contains(&text.galley.size().x),
                            "Help action {action:?} needs a readable column at {size:?}: {:?}.", text.galley.size());
                        seen.insert(action.clone());
                    }
                }
            };
            inspect_actions(&output, &mut seen_actions);
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
                // Later sections follow this guidance, so scroll toward it in
                // steps and stop when it appears.
                let mut steps = 0;
                while !visible(&output, label, viewport) && steps < 60 {
                    steps += 1;
                    frame_at_size(
                        &context,
                        &mut app,
                        vec![
                            egui::Event::PointerMoved(instruction.center()),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: Vec2::new(0.0, -40.0),
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
                    inspect_actions(&output, &mut seen_actions);
                }
                assert!(
                    visible(&output, label, viewport),
                    "Help guidance is unreachable at {size:?}: {label}"
                );
            }
            for action in ACTIONS {
                assert!(seen_actions.contains(action), "Help action {action:?} must remain reachable at {size:?}.");
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
    fn sample_editor_readiness_keeps_actions_visible_and_offers_missing_source_recovery() {
        for (available, missing, frames, start, end) in [
            (false, false, 0, 0, 0),
            (false, true, 0, 0, 0),
            (true, false, 0, 0, 0),
            (true, false, 32, 0, 32),
            (true, false, 32, 17, 17),
            (true, false, 32, 18, 17),
            (true, false, 32, 31, 32),
        ] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.enable_accesskit();
            ctx.style_mut(|style| {
                style.interaction.tooltip_delay = 0.0;
                style.animation_time = 0.0;
            });
            let ready = available && !missing && frames > 0 && start < end && end <= frames;
            let range = format!("{start}..{end}");
            let (mut open, mut start, mut end, mut pitch) = (true, start, end, 0.0);
            let mut render = |events| {
                let mut action = sample_editor::Action::None;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ctx| {
                        action = sample_editor::show(
                            ctx,
                            &mut open,
                            sample_editor::Source {
                                name: "Source",
                                frames,
                                sample_rate: 1000,
                                available,
                                missing,
                            },
                            &mut start,
                            &mut end,
                            &mut pitch,
                        )
                        .unwrap();
                    },
                );
                (output, action)
            };
            for _ in 0..3 {
                render(vec![]);
            }
            let (mut output, _) = render(vec![]);
            for label in ["Listen edit", "Save edited sound"] {
                assert_eq!(
                    !accessible_node(&output, label).1.is_disabled(),
                    ready,
                    "{label}: available={available}, missing={missing}, frames={frames}, {range}"
                );
            }
            accessible_node(&output, "Sample editor status");
            let actions = if missing {
                vec![("Import details", sample_editor::Action::Details)]
            } else if ready {
                vec![
                    ("Listen edit", sample_editor::Action::Listen),
                    ("Save edited sound", sample_editor::Action::Save),
                ]
            } else {
                vec![]
            };
            for (label, expected) in actions {
                let target = accessible_node(&output, label).0;
                render(vec![egui::Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::Focus,
                        target,
                        data: None,
                    },
                )]);
                let (_, action) = render(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
                assert_eq!(
                    std::mem::discriminant(&action),
                    std::mem::discriminant(&expected),
                    "{label} must return its action without performing audio or file work in the view."
                );
                (output, _) = render(vec![]);
            }
            if !ready {
                let reason = if missing {
                    "This sound is missing and plays silence. Open Import details to inspect or resolve it."
                } else if !available || frames == 0 {
                    "Select a nonempty sound or clip before editing."
                } else {
                    "End frame must be greater than Start frame and within the source."
                };
                let copies = |output: &egui::FullOutput| { output.shapes.iter().filter(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text)
                        if text.galley.job.text == reason
                            && shape.clip_rect.contains_rect(text.visual_bounding_rect()))
                        }).count()
                };
                let footer_copies = copies(&output);
                assert!(footer_copies > 0, "The status footer must explain unavailable processing.");
                for label in ["Listen edit", "Save edited sound"] {
                    let center = effects_control_rect(&output, label).center();
                    render(vec![egui::Event::PointerMoved(center)]);
                    let mut shown = false;
                    for _ in 0..4 {
                        let (hovered, action) = render(vec![]);
                        assert!(matches!(action, sample_editor::Action::None));
                        shown |= copies(&hovered) > footer_copies;
                    }
                    assert!(shown, "Disabled {label} must explain why it is unavailable on hover.");
                }
            }
        }
    }

    #[test]
    fn sample_editor_native_fields_slider_and_steppers_edit_only_the_draft() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.selected_sample = Some(1);
        app.show_sample_edit = true;
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let project = app.session.project().clone();
        let unsaved = app.session.unsaved();
        let set_value = |app: &mut MixJamApp, label: &str, value: f64| {
            let output = frame(&ctx, app, vec![], egui::Modifiers::NONE);
            let target = accessible_node(&output, label).0;
            frame(
                &ctx,
                app,
                vec![egui::Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::SetValue,
                        target,
                        data: Some(egui::accesskit::ActionData::NumericValue(value)),
                    },
                )],
                egui::Modifiers::NONE,
            );
        };
        let activate = |app: &mut MixJamApp, label: &str, modifiers: egui::Modifiers| {
            let output = frame(&ctx, app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, app, &output, label);
            frame(
                &ctx,
                app,
                vec![key_event(egui::Key::Enter, modifiers)],
                modifiers,
            );
        };
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for (label, min, max) in [
            ("Start frame", 0.0, 75599.0),
            ("End frame", 1.0, 75600.0),
            ("Pitch value", -24.0, 24.0),
            ("Pitch (semitones)", -24.0, 24.0),
        ] {
            let node = accessible_node(&output, label).1;
            assert_eq!(node.min_numeric_value(), Some(min), "{label}");
            assert_eq!(node.max_numeric_value(), Some(max), "{label}");
        }
        set_value(&mut app, "Start frame", 17.0);
        set_value(&mut app, "End frame", 19.0);
        assert_eq!((app.edit_start, app.edit_end), (17, 19));
        activate(&mut app, "Increase End frame", egui::Modifiers::NONE);
        assert_eq!(app.edit_end, 20);
        activate(&mut app, "Decrease Start frame", egui::Modifiers::SHIFT);
        assert_eq!(
            app.edit_start, 0,
            "Shift frame stepping clamps at the source boundary."
        );
        activate(&mut app, "Increase Start frame", egui::Modifiers::NONE);
        assert_eq!(app.edit_start, 1);
        set_value(&mut app, "Start frame", 100000.0);
        set_value(&mut app, "End frame", 0.0);
        assert_eq!((app.edit_start, app.edit_end), (75599, 1));
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for label in ["Listen edit", "Save edited sound"] {
            assert!(
                accessible_node(&output, label).1.is_disabled(),
                "A reversed trim must remain editable without allowing audio processing."
            );
        }
        set_value(&mut app, "Start frame", 0.0);
        set_value(&mut app, "End frame", 75600.0);
        set_value(&mut app, "Pitch value", 3.5);
        assert_eq!(app.edit_pitch, 3.5);
        activate(&mut app, "Increase Pitch", egui::Modifiers::NONE);
        assert!((app.edit_pitch - 3.6).abs() < 0.0001);
        activate(&mut app, "Decrease Pitch", egui::Modifiers::SHIFT);
        assert!((app.edit_pitch - 2.6).abs() < 0.0001);
        set_value(&mut app, "Pitch (semitones)", -12.0);
        assert_eq!(app.edit_pitch, -12.0);
        set_value(&mut app, "Pitch value", 100.0);
        assert_eq!(app.edit_pitch, 24.0);
        activate(&mut app, "Increase Pitch", egui::Modifiers::NONE);
        assert_eq!(app.edit_pitch, 24.0);
        set_value(&mut app, "Pitch value", -100.0);
        assert_eq!(app.edit_pitch, -24.0);
        activate(&mut app, "Decrease Pitch", egui::Modifiers::NONE);
        assert_eq!(app.edit_pitch, -24.0);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Pitch value");
        frame(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("-4.5678".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.edit_pitch, -4.5678,
            "The amber readout supports exact keyboard entry."
        );
        assert_eq!(app.session.project(), &project);
        assert_eq!(app.session.unsaved(), unsaved);
        assert!(
            !app.session.is_previewing() && !app.session.wants_playback(),
            "Editing the tool draft must not start preview or transport."
        );
    }

    #[test]
    fn sample_editor_compact_layout_uses_complete_density_assets_and_preserves_collapsed_edits() {
        let size = Vec2::new(960.0, 720.0);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            ctx.set_pixels_per_point(density);
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.selected_sample = Some(1);
            app.show_sample_edit = true;
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            app.edit_start = 123;
            app.edit_end = 34567;
            app.edit_pitch = -3.56789;
            let before = (app.edit_start, app.edit_end, app.edit_pitch);
            let mut output = egui::FullOutput::default();
            for _ in 0..4 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let window = effects_control_rect(&output, "SAMPLE EDITOR");
            let panel = window;
            assert_folded_assets(&ctx,&output,density,"Granite");
            assert!(
                viewport.contains_rect(window)
            );
            let footer = effects_control_rect(&output, "Sample editor status");
            assert!(window.contains_rect(footer));
            let controls = [
                "Start frame",
                "End frame",
                "Pitch (semitones)",
                "Pitch value",
                "Listen edit",
                "Save edited sound",
                "Collapse Sample editor",
                "Close Sample editor",
            ];
            let bounds = controls.map(|label| effects_control_rect(&output, label));
            for (label, bounds) in controls.iter().zip(bounds) {
                assert!(
                    panel.contains_rect(bounds),
                    "{label} must fit the skin at {density}x."
                );
                assert!(!bounds.intersects(footer), "{label} must not touch status.");
            }
            for (index, first) in bounds.iter().enumerate() {
                for (second, name) in bounds.iter().zip(controls).skip(index + 1) {
                    assert!(
                        !first.intersects(*second),
                        "Controls must not overlap at {density}x: {} and {name}.",
                        controls[index]
                    );
                }
            }
            for label in [
                "SAMPLE EDITOR",
                "Start frame",
                "End frame",
                "Pitch (semitones)",
                "Listen edit",
                "Save edited sound",
            ] {
                assert!(
                    panel.contains_rect(text_rect(&output, label)),
                    "{label} must remain readable inside the panel at {density}x."
                );
            }
            for label in ["Listen edit", "Save edited sound"] {
                assert!(
                    effects_control_rect(&output, label)
                        .shrink(2.0)
                        .contains_rect(effects_text_mesh_bounds(&output, label)),
                    "{label} needs clear space inside its button at {density}x."
                );
            }
            focus_control(&ctx, &mut app, &output, "Collapse Sample editor");
            frame(
                &ctx,
                &mut app,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,
            );
            for _ in 0..3 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let collapsed = effects_control_rect(&output, "SAMPLE EDITOR");
            assert_eq!(collapsed.min, window.min
            );
            assert_eq!(collapsed.width(), window.width());
            assert!((collapsed.height() - 44.0).abs() < 0.1);
            for label in [
                "Start frame",
                "End frame",
                "Pitch value",
                "Listen edit",
                "Sample editor status",
            ] {
                assert!(
                    !output
                        .platform_output
                        .accesskit_update
                        .as_ref()
                        .unwrap()
                        .nodes
                        .iter()
                        .any(|(_, node)| node.label() == Some(label)),
                    "{label} must hide while collapsed."
                );
            }
            assert_eq!((app.edit_start, app.edit_end, app.edit_pitch), before);
            focus_control(&ctx, &mut app, &output, "Expand Sample editor");
            frame(
                &ctx,
                &mut app,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,
            );
            for _ in 0..3 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            assert_eq!(effects_control_rect(&output, "SAMPLE EDITOR"), window);
            assert_eq!((app.edit_start, app.edit_end, app.edit_pitch), before);
            focus_control(&ctx, &mut app, &output, "Close Sample editor");
            frame(
                &ctx,
                &mut app,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,
            );
            assert!(!app.show_sample_edit);
            assert_eq!((app.edit_start, app.edit_end, app.edit_pitch), before);
        }
    }

    #[test]
    fn collapsed_sample_editor_retains_trim_when_palette_changes_away_and_back() {
        let (ctx,mut app)=fixture();ctx.enable_accesskit();
        let a=app.session.library().samples[0].clone();
        let mut b=a.clone();b.id=2;b.name="Other source".into();
        app.session=Session::headless(Arc::new(Library::fixture(vec![a,b])),app.session.project().clone()).unwrap();
        app.selected_sample=Some(1);app.show_sample_edit=true;
        for _ in 0..3 {frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);}
        app.edit_start=100;app.edit_end=1000;app.edit_pitch=3.25;
        let before=(app.edit_start,app.edit_end,app.edit_pitch);
        let output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
        focus_control(&ctx,&mut app,&output,"Collapse Sample editor");
        frame(&ctx,&mut app,vec![key_event(egui::Key::Enter,egui::Modifiers::NONE)],egui::Modifiers::NONE);
        app.selected_sample=Some(2);frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
        app.selected_sample=Some(1);let output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
        assert_eq!((app.edit_start,app.edit_end,app.edit_pitch),before);
        focus_control(&ctx,&mut app,&output,"Expand Sample editor");
        frame(&ctx,&mut app,vec![key_event(egui::Key::Enter,egui::Modifiers::NONE)],egui::Modifiers::NONE);
        frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
        assert_eq!((app.edit_start,app.edit_end,app.edit_pitch),before);
    }

    #[test]
    fn sample_editor_trim_uses_stereo_source_frames_and_an_exclusive_end() {
        let (context, mut app, dir) = sample_conflicts_fixture("sample-editor-trim");
        app.session.confirm_sample_conflicts().unwrap();
        app.show_sample_conflicts = false;
        app.selected_sample = Some(1_000_000);
        app.show_sample_edit = true;
        let path = app.session.sample_conflicts()[0].candidates[0].clone();
        Pcm {
            sample_rate: 1000,
            channels: 2,
            samples: (0..2000)
                .flat_map(|frame| {
                    let value = frame as f32 / 4000.0;
                    [value, -value]
                })
                .collect(),
        }
        .write_wav(&path)
        .unwrap();
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!((app.edit_start, app.edit_end), (0, 2000));
        app.edit_start = 17;
        app.edit_end = 19;
        app.edit_pitch = 0.0;
        let source = app.session.source_pcm(app.source_id()).unwrap();
        let edited = app.edited_pcm().unwrap();
        assert_eq!(
            (edited.channels, edited.sample_rate, edited.frames()),
            (2, 1000, 2)
        );
        assert_eq!(
            edited.samples,
            source.samples[34..38],
            "Start is inclusive, end is exclusive, and each stereo frame includes both channels."
        );
        app.edit_start = 1999;
        app.edit_end = 2000;
        assert_eq!(
            app.edited_pcm().unwrap().samples,
            source.samples[3998..4000]
        );
        for (start, end) in [(10, 10), (11, 10), (0, 2001)] {
            app.edit_start = start;
            app.edit_end = end;
            assert!(
                app.edited_pcm().is_err(),
                "Invalid trim {start}..{end} must not render."
            );
        }
        app.edit_start = 0;
        app.edit_end = 2000;
        app.edit_pitch = 12.0;
        assert_eq!(
            app.edited_pcm().unwrap().frames(),
            1000,
            "Pitch changes source speed and duration after trimming."
        );
        app.edit_pitch = -12.0;
        assert_eq!(app.edited_pcm().unwrap().frames(), 4000);
        std::fs::remove_dir_all(dir).unwrap();
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
        for label in ["Listen edit", "Save edited sound"] {
            assert!(accessible_node(&output, label).1.is_disabled(),
                "Missing source actions remain visible and explain why they are unavailable.");
        }
        accessible_node(&output, "Import details");
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

    fn activate_control(
        context: &egui::Context,
        app: &mut MixJamApp,
        output: &egui::FullOutput,
        label: &str,
    ) -> egui::FullOutput {
        let target = step_synth_accessible_node(output, label).0;
        frame(
            context,
            app,
            vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus, target, data: None,
            },)],
            egui::Modifiers::NONE,
        );
        frame(
            context,
            app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(context, app, vec![], egui::Modifiers::NONE)
    }

    fn step_synth_accessible_node<'a>(
        output: &'a egui::FullOutput,
        label: &str,
    ) -> (egui::accesskit::NodeId, &'a egui::accesskit::Node) {
        // The main menu also has Effects. Use tree ownership because the window
        // bounds can still describe the collapsed size on the first expanded frame.
        let window = accessible_node(output, "STEP SYNTH").0;
        let nodes = &output.platform_output.accesskit_update.as_ref().unwrap().nodes;
        let mut descendants = BTreeSet::from([window]);
        for _ in 0..64 {
            let before = descendants.len();
            for (id, node) in nodes {
                if descendants.contains(id) { descendants.extend(node.children().iter().copied()); }
            }
            if descendants.len() == before { break; }
        }
        nodes.iter()
            .find(|(id, node)| descendants.contains(id) && node.label() == Some(label))
            .map(|(id, node)| (*id, node))
            .unwrap_or_else(|| panic!("Missing Step synth control: {label}"))
    }

    fn step_synth_control_rect(output: &egui::FullOutput, label: &str) -> Rect {
        let bounds = step_synth_accessible_node(output, label).1.bounds().unwrap();
        Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        )
    }

    fn step_synth_text_mesh_bounds(output: &egui::FullOutput, label: &str) -> Rect {
        let window = effects_control_rect(output, "STEP SYNTH");
        output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Text(shape) if shape.galley.job.text == label => {
                let bounds = shape.galley.mesh_bounds.translate(shape.pos.to_vec2());
                window.contains_rect(bounds).then_some(bounds)
            }
            _ => None,
        }).reduce(|bounds, next| bounds.union(next))
            .unwrap_or_else(|| panic!("Missing Step synth glyphs: {label}"))
    }

    #[test]

    fn step_synth_edits_one_note_per_step_and_clears_only_the_viewed_bar() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| {
            style.animation_time = 0.0;
            style.scroll_animation = egui::style::ScrollAnimation::none();
        });
        app.selected_sample = Some(1);
        app.show_step_synth = true;
        app.step_synth.steps[1] = Some(-36);
        app.step_synth.steps[16] = Some(-48);
        app.step_synth.steps[48] = Some(48);
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = activate_control(&ctx, &mut app, &output, "Step 1, +12 semitones");
        assert_eq!(app.step_synth.steps[0], Some(12), "A new pitch must replace the existing note.");
        let output = activate_control(&ctx, &mut app, &output, "Step 1, +12 semitones");
        assert_eq!(app.step_synth.steps[0], None, "Activating the selected note must clear it.");
        let output = activate_control(&ctx, &mut app, &output, "Step 1, +11 semitones");
        let output = activate_control(&ctx, &mut app, &output, "View bar 2");
        assert_eq!(app.step_synth_bar, 1);
        let output = activate_control(&ctx, &mut app, &output, "Step 5, +12 semitones");
        assert_eq!(app.step_synth.steps[20], Some(12));
        let before_clear = app.step_synth.clone();
        let output = activate_control(&ctx, &mut app, &output, "Clear bar");
        assert!(app.step_synth.steps[16..32].iter().all(Option::is_none));
        assert_eq!(&app.step_synth.steps[..16], &before_clear.steps[..16]);
        assert_eq!(&app.step_synth.steps[32..], &before_clear.steps[32..]);
        let output = activate_control(&ctx, &mut app, &output, "View bar 4");
        assert_eq!(app.step_synth.steps[48], Some(48), "Viewing a bar must retain loaded pitches outside the visible grid.");
        activate_control(&ctx, &mut app, &output, "View bar 1");
        assert_eq!(app.step_synth.steps[0], Some(11));
        assert_eq!(app.step_synth.steps[1], Some(-36));
        assert_eq!(app.step_synth.active_bars, before_clear.active_bars, "Viewing a bar must not enable it for playback.");
    }

    #[test]
    fn step_synth_compact_layout_preserves_the_draft_and_keeps_both_panes_reachable() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            ctx.style_mut(|style| {
                style.animation_time = 0.0;
                style.scroll_animation = egui::style::ScrollAnimation::none();
            });
            for _ in 0..2 {frame_at_size(&ctx,&mut app,vec![],egui::Modifiers::NONE,Vec2::new(960.0,720.0));}
            app.selected_sample = Some(1);
            app.show_step_synth = true;
            app.step_synth.steps[63] = Some(-48);
            app.step_synth.effects.echo = Some(Default::default());
            let before = app.step_synth.clone();
            let size = Vec2::new(960.0, 720.0);
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            let mut output = egui::FullOutput::default();
            for _ in 0..4 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let window = effects_control_rect(&output, "STEP SYNTH");
            let panel = window;
            assert_folded_assets(&ctx,&output,density,"Olive");
            assert!(viewport.contains_rect(window));
            let footer = step_synth_control_rect(&output, "Step synth status");
            assert!(window.contains_rect(footer),"Footer {footer:?} outside window {window:?} at {density}x");
            assert_eq!(accessible_node(&output, "Step synth source sound").1.value(), Some("good grasp 2"));
            for label in ["Choose source file", "Use palette sound", "View bar 1", "View bar 4",
                "Active 1", "Active 4", "1/16 cut", "Loop source", "Step synth BPM", "Clear bar",
                "Listen sequence", "Save sequence", "Effects", "Save pattern", "Load pattern",] {
                let bounds = step_synth_control_rect(&output, label);
                assert!(panel.contains_rect(bounds), "{label} must fit the skin at {density}x.");
                assert!(!bounds.intersects(footer), "{label} must not overlap the status footer.");
            }
            for label in ["Choose source file", "Use palette sound", "Clear bar", "Listen sequence",
                "Save sequence", "Effects", "Save pattern", "Load pattern",] {
                let glyphs = step_synth_text_mesh_bounds(&output, label);
                let button = step_synth_control_rect(&output, label);
                assert!(button.shrink(2.0).contains_rect(glyphs), "{label} must have clear space inside its button at {density}x.");
            }
            for bar in 1..=3 {
                let caption = step_synth_text_mesh_bounds(&output, &format!("Active {bar}"));
                let next_switch = step_synth_control_rect(&output, &format!("Active {}", bar + 1));
                assert!(next_switch.left() - caption.right() >= 12.0,
                    "Active bar captions need clear space before the next switch at {density}x.");
            }
            let bottom_note = "Step 16, -12 semitones";
            focus_control(&ctx, &mut app, &output, bottom_note);
            for _ in 0..4 { frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let cell = step_synth_control_rect(&output, bottom_note);
            let note_view = Rect::from_min_size(panel.min + Vec2::new(32.0, 198.0), Vec2::new(784.0, 306.0));
            assert!(note_view.contains_rect(cell), "Keyboard focus must scroll the lowest pitch into view at {density}x.");
            assert!(!cell.intersects(step_synth_control_rect(&output, "Load pattern")));
            let output = activate_control(&ctx, &mut app, &output, "Effects");
            assert!(app.step_synth_effects_expanded);
            accessible_node(&output, "Note grid");
            text_rect(&output, "Sequence effects");
            text_rect(&output, "Echo");
            let row_y=step_synth_control_rect(&output,"Cutoff").center().y;
            for name in ["Cutoff","Resonance","Overdrive","Filter volume"] {
                let dial=step_synth_control_rect(&output,name);
                let value=step_synth_control_rect(&output,&format!("{name} value"));
                assert!((dial.center().y-row_y).abs()<0.1,"Shared effect dials must align in a row.");
                assert!(value.top()>dial.bottom(),"Numeric value must sit below its dial.");
                assert!((value.center().x-dial.center().x).abs()<2.0,"Numeric value must align below {name}.");
            }
            let before_collapse=effects_control_rect(&output,"STEP SYNTH");
            let output = activate_control(&ctx, &mut app, &output, "Collapse Step synth");
            let collapsed = effects_control_rect(&output, "STEP SYNTH");
            assert_eq!(collapsed.min, before_collapse.min);
            assert_eq!(collapsed.width(), window.width());
            assert!((collapsed.height() - 44.0).abs() < 0.1);
            for hidden in ["Step synth source sound", "Note grid", "Load pattern", "Step synth status",] {
                assert!(!output.platform_output.accesskit_update.as_ref().unwrap().nodes.iter()
                    .any(|(_, node)| node.label() == Some(hidden)), "{hidden} must hide when collapsed.");
            }
            let output = activate_control(&ctx, &mut app, &output, "Expand Step synth");
            assert_eq!(effects_control_rect(&output, "STEP SYNTH"), before_collapse);
            assert!(app.step_synth_effects_expanded, "Collapse must preserve the chosen pane.");
            assert_eq!(app.step_synth, before, "Pane changes, scrolling, and collapse must preserve the draft.");
            let output = activate_control(&ctx, &mut app, &output, "Note grid");
            assert!(!app.step_synth_effects_expanded);
            accessible_node(&output, "Step 16, -12 semitones");
            activate_control(&ctx, &mut app, &output, "Step synth help");
            assert!(app.show_step_synth_help);
            app.show_step_synth_help = false;
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            activate_control(&ctx, &mut app, &output, "Close Step synth");
            assert!(!app.show_step_synth);
            assert_eq!(app.step_synth, before);
        }
    }

    #[test]
    fn step_synth_source_readiness_disables_audio_actions_but_keeps_draft_and_recovery_available() {
        for (available, missing, explicit_file, active) in [
            (false, false, false, true),
            (false, true, false, true),
            (true, false, false, true),
            (true, true, true, true),
            (true, false, false, false),
        ] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.enable_accesskit();
            let mut state = StepSynthState::default();
            if !active { state.active_bars.fill(false); }
            let (mut open, mut view_bar, mut effects) = (true, 0, false);
            let mut render = |events| {
                let mut action = step_synth::Action::None;
                let output = ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    focused: true, events, ..Default::default()
                }, |ctx| {
                    action = step_synth::show(ctx, &mut open,
                        step_synth::Source { name: "Source", available, missing, explicit_file, },
                        step_synth::Feedback { message: None, error: false, },
                        &mut state, &mut view_bar, &mut effects,).unwrap();
                },);
                (output, action)
            };
            for _ in 0..3 { render(vec![]); }
            let (mut output, _) = render(vec![]);
            let ready = available && (!missing || explicit_file) && active;
            for label in ["Listen sequence", "Save sequence"] {
                assert_eq!(!accessible_node(&output, label).1.is_disabled(), ready,
                    "{label}: available={available}, missing={missing}, file={explicit_file}, active={active}");
            }
            for label in ["Choose source file", "Use palette sound", "Save pattern", "Load pattern",
                "Active 1", "1/16 cut", "Loop source", "Step synth BPM", "Step 1, +12 semitones",] {
                assert!(!accessible_node(&output, label).1.is_disabled(), "{label} must remain available for editing or recovery.");
            }
            let mut actions = vec![
                ("Choose source file", step_synth::Action::ChooseSource),
                ("Use palette sound", step_synth::Action::PaletteSource),
                ("Save pattern", step_synth::Action::SavePattern),
                ("Load pattern", step_synth::Action::LoadPattern),
                ("Step synth help", step_synth::Action::Help),
            ];
            if missing && !explicit_file { actions.push(("Import details", step_synth::Action::Details)); }
            if ready {
                actions.push(("Listen sequence", step_synth::Action::Listen));
                actions.push(("Save sequence", step_synth::Action::SaveSequence));
            }
            for (label, expected) in actions {
                let target = accessible_node(&output, label).0;
                render(vec![egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus, target, data: None,
                },)]);
                let (_, action) = render(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
                assert_eq!(std::mem::discriminant(&action), std::mem::discriminant(&expected),
                    "{label} must return its Session action without performing audio or file work in the view.");
                (output, _) = render(vec![]);
            }
        }
    }

    #[test]
    fn step_synth_options_apply_to_the_draft_without_starting_transport() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.selected_sample = Some(1);
        app.show_step_synth = true;
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = activate_control(&ctx, &mut app, &output, "Active 2");
        assert_eq!(app.step_synth.active_bars, [true, true, false, false]);
        let output = activate_control(&ctx, &mut app, &output, "Active 1");
        assert_eq!(app.step_synth.active_bars, [false, true, false, false]);
        let output = activate_control(&ctx, &mut app, &output, "1/16 cut");
        let output = activate_control(&ctx, &mut app, &output, "Loop source");
        assert!(app.step_synth.cut_sixteenth && app.step_synth.loop_source);
        let output = activate_control(&ctx, &mut app, &output, "Step synth BPM");
        focus_control(&ctx, &mut app, &output, "160");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth.bpm, 160.0);
        let output = activate_control(&ctx, &mut app, &output, "Active 2");
        for label in ["Listen sequence", "Save sequence"] {
            assert!(accessible_node(&output, label).1.is_disabled(), "Audio actions need at least one active bar.");
        }
        let output = activate_control(&ctx, &mut app, &output, "Active 4");
        for label in ["Listen sequence", "Save sequence"] {
            assert!(!accessible_node(&output, label).1.is_disabled());
        }
        assert_eq!(app.step_synth_bar, 0, "Active bars and viewed bars must stay independent.");
        assert!(!app.session.wants_playback(), "Editing generator options must not start transport.");
        assert!(!app.session.is_previewing());
        assert!(app.step_synth.validate().is_ok());
    }

    #[test]
    fn step_synth_file_source_can_recover_a_missing_palette_source_and_reports_read_errors_locally() {
        let (ctx, mut app) = missing_fixture();
        ctx.enable_accesskit();
        app.select_clip(1, false);
        app.show_step_synth = true;
        let draft = app.step_synth.clone();
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(step_synth_accessible_node(&output, "Listen sequence").1.is_disabled());
        step_synth_accessible_node(&output, "Import details");
        let absent_source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/step-synth-tests")
            .join(format!("absent-source-{}.wav", std::process::id()));
        assert!(!absent_source.exists());
        app.finish_path(FileRequest::SynthSource, Some(absent_source.clone()));
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth_file.as_ref(), Some(&absent_source));
        assert!(!step_synth_accessible_node(&output, "Listen sequence").1.is_disabled(),
            "An explicit file must bypass the missing palette source.");
        let output = activate_control(&ctx, &mut app, &output, "Listen sequence");
        assert!(app.status_error && app.step_synth_feedback.error);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some(app.status.as_str()));
        assert!(!app.session.is_previewing());
        assert_eq!(app.step_synth, draft);
        let output = activate_control(&ctx, &mut app, &output, "Use palette sound");
        assert!(app.step_synth_file.is_none());
        assert!(app.step_synth_feedback.message.is_none());
        assert!(!app.step_synth_feedback.error);
        assert!(step_synth_accessible_node(&output, "Listen sequence").1.is_disabled());
        activate_control(&ctx, &mut app, &output, "Import details");
        assert!(app.show_import_details);
        assert_eq!(app.step_synth, draft);
    }

    #[test]
    fn step_synth_pattern_paths_keep_the_draft_on_cancel_or_error_and_restore_complete_state() {
        let (ctx, mut app) = fixture();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/step-synth-tests")
            .join(format!("pattern-paths-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        app.show_step_synth = true;
        app.step_synth.bpm = 160.0;
        app.step_synth.active_bars = [true, false, true, false];
        app.step_synth.steps[1] = Some(-48);
        app.step_synth.steps[63] = Some(48);
        app.step_synth.cut_sixteenth = true;
        app.step_synth.loop_source = true;
        app.step_synth.effects.dirty = 0.25;
        let saved = app.step_synth.clone();
        app.finish_path(FileRequest::SavePattern, None);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Pattern save cancelled."));
        app.finish_path(FileRequest::LoadPattern, None);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Pattern load cancelled."));
        app.finish_path(FileRequest::SynthSource, None);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Source selection cancelled."));
        assert!(!app.step_synth_feedback.error);
        assert_eq!(app.step_synth, saved);
        assert!(app.step_synth_file.is_none());
        let pattern = dir.join("sequence.json");
        app.finish_path(FileRequest::SavePattern, Some(pattern.clone()));
        assert!(!app.status_error);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Saved Step synth pattern."));
        assert_eq!(app.session.load_pattern(&pattern).unwrap(), saved);
        app.step_synth = StepSynthState::default();
        app.finish_path(FileRequest::LoadPattern, Some(pattern));
        assert!(!app.status_error);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Loaded Step synth pattern."));
        assert_eq!(app.step_synth, saved, "Loading must restore notes, active bars, tempo, source options, and effects.");
        let broken = dir.join("broken.json");
        std::fs::write(&broken, b"{ not a pattern }").unwrap();
        app.finish_path(FileRequest::LoadPattern, Some(broken));
        assert!(app.status_error);
        assert!(app.step_synth_feedback.error);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some(app.status.as_str()));
        assert_eq!(app.step_synth, saved, "A failed load must retain the current editable draft.");
        app.finish_path(FileRequest::SavePattern, Some(dir.join("absent-parent/sequence.json")),);
        assert!(app.status_error);
        assert!(app.step_synth_feedback.error);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some(app.status.as_str()));
        assert_eq!(app.step_synth, saved);
        assert_eq!(app.selected_sample, None);
        assert_eq!(app.step_synth_bar, 0);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth, saved);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn step_synth_feedback_is_dropped_when_the_source_changes_but_kept_for_its_own_results() {
        let (ctx, mut app) = missing_fixture();
        ctx.enable_accesskit();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/step-synth-tests")
            .join(format!("feedback-source-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        app.selected_sample = Some(1);
        app.show_step_synth = true;
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        app.finish_path(FileRequest::LoadPattern, None);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Pattern load cancelled."),
            "Feedback must remain while the source is unchanged.");
        app.select_clip(1, false);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(app.step_synth_feedback.message.is_none(), "A new palette source must drop the previous source's feedback.");
        assert!(output.platform_output.accesskit_update.as_ref().unwrap().nodes.iter().any(|(_, node)|
            [node.label(), node.value()].contains(&Some("Missing audio. Choose a source file or open Import details."))),
            "The footer must return to the missing-audio guidance.");
        app.finish_path(FileRequest::SynthSource, Some(dir.join("source.wav")));
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Selected a source file. Listen before saving."));
        app.selected_sample = Some(1);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Selected a source file. Listen before saving."),
            "Palette selection must not affect feedback while an explicit file is the source.");
        app.step_synth_file = None;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        app.finish_path(FileRequest::SaveSound {
            pcm: Pcm { sample_rate: 1000, channels: 1, samples: vec![0.25; 1000], },
            name: "sequence".into(), group: "Step synth", source_bpm: 140.0, stretch: None,
        }, Some(dir.join("sequence.wav")),);
        assert_ne!(app.selected_sample, Some(1), "Saving must select the new Step synth sound.");
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.step_synth_feedback.message.as_deref(), Some("Saved sequence.wav to Step synth."),
            "Selecting the saved sound must keep the save confirmation.");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn step_synth_pane_switch_keeps_keyboard_focus_and_disabled_actions_explain_themselves() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        app.selected_sample = Some(1);
        app.show_step_synth = true;
        for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = activate_control(&ctx, &mut app, &output, "Effects");
        assert!(app.step_synth_effects_expanded);
        assert_eq!(output.platform_output.accesskit_update.as_ref().unwrap().focus, step_synth_accessible_node(&output, "Note grid").0,
            "Switching panes from the keyboard must keep focus on the pane button.");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(!app.step_synth_effects_expanded, "A second Enter must return to the note grid.");
        assert_eq!(output.platform_output.accesskit_update.as_ref().unwrap().focus, step_synth_accessible_node(&output, "Effects").0);
        app.step_synth.active_bars.fill(false);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let listen = step_synth_control_rect(&output, "Listen sequence");
        // The footer shows the same sentence, so count the tooltip as an added copy.
        let copies = |output: &egui::FullOutput| { output.shapes.iter().filter(|shape| { matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Select at least one generator bar.")
                }).count()
        };
        let before = copies(&output);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(listen.center())], egui::Modifiers::NONE,);
        let mut hovered = before;
        for _ in 0..4 {
            hovered = hovered.max(copies(&frame(&ctx, &mut app, vec![], egui::Modifiers::NONE,)));
        }
        assert!(hovered > before, "A disabled audio action must show why it is unavailable on hover.");
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
            app.bottom_module = BottomModule::Mixer;
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
                for label in [format!("Arrangement track {track} {side} output"), format!("Track {track} {side} output"),] {
                    let (_, node) = accessible_node(output, &label);
                    assert_eq!(node.toggled(), Some(if enabled { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False }), "{label}");
                    let bounds = node.bounds().unwrap();
                    let rect = Rect::from_min_max(Pos2::new(bounds.x0 as f32, bounds.y0 as f32), Pos2::new(bounds.x1 as f32, bounds.y1 as f32),);
                    let painted = output.shapes.iter().find_map(|shape| match &shape.shape {
                        egui::Shape::Text(painted) if painted.galley.job.text == text && rect.contains(painted.pos) =>
                            { Some(painted)
                            }
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
        app.session = Session::headless(Arc::new(Library::fixture(app.session.library().samples.to_vec())), project,).unwrap();
        app.bottom_module = BottomModule::Mixer;
        ctx.enable_accesskit();
        for (left, right) in [(true, true), (true, false), (false, false), (false, true)] {
            for (side, enabled) in [(true, left), (false, right)] {
                app.session.edit(EditCommand::SetRouting { track: 0, left: side, enabled, }).unwrap();
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
            click(&ctx, &mut app, Pos2::new((bounds.x0 + bounds.x1) as f32 * 0.5, (bounds.y0 + bounds.y1) as f32 * 0.5,), egui::Modifiers::NONE,);
            let changed = app.session.project().tracks[track].clone();
            assert_eq!(changed.left_enabled, if left { !before.left_enabled } else { before.left_enabled });
            assert_eq!(changed.right_enabled, if left { before.right_enabled } else { !before.right_enabled });
            assert_eq!(changed.pan, before.pan);
            assert_eq!(changed.gain, before.gain);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, track + 1, changed.left_enabled, changed.right_enabled,);
            app.history_step(false);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, track + 1, before.left_enabled, before.right_enabled,);
            app.history_step(true);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_routes(&output, track + 1, changed.left_enabled, changed.right_enabled,);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Arrangement track 64 left output");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Space, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_routes(&output, 64, false, false);
        assert!(!app.session.wants_playback(), "Space on a lane output must toggle routing only.");
        assert_eq!(app.session.project().tracks[0].pan, -0.75);
        assert_eq!(app.session.project().tracks[63].pan, 0.5);
    }

    #[test]
    fn mixer_db_gain_labels_do_not_change_saved_amplitude_precision() {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.tracks[0].gain = 0.7533;
        project.tracks[0].polarity_inverted = true;
        project.tracks[1].gain = 0.3333;
        project.master_gain = 0.7777;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        ctx.enable_accesskit();
        app.bottom_module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        for label in ["-2.5 dB", "-9.5 dB", "-2.2 dB"] {
            text_rect(&output, label);
        }
        assert_eq!(app.session.project().tracks[0].gain, 0.7533);
        assert!(app.session.project().tracks[0].polarity_inverted);
        assert_eq!(app.session.project().tracks[1].gain, 0.3333);
        assert_eq!(app.session.project().master_gain, 0.7777);
        assert!(!app.session.unsaved());

        // A passive display or focus change must not round-trip amplitude
        // through its rounded dB label and change the saved sound.
        focus_control(&ctx, &mut app, &output, "Track 2 gain value");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Master gain value");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 left output");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.session.project().tracks[1].gain, 0.3333);
        assert_eq!(app.session.project().master_gain, 0.7777);
        assert!(!app.session.unsaved());

        // Explicitly entering a rounded dB value is a real amplitude edit.
        focus_control(&ctx, &mut app, &output, "Track 2 gain value");
        frame(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("-9.5".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!((app.session.project().tracks[1].gain - 10.0_f32.powf(-9.5 / 20.0)).abs() < 0.000_001);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let (_, gain) = accessible_node(&output, "Track 2 gain");
        assert!((gain.numeric_value().unwrap() + 9.5).abs() < 0.001);
        assert_eq!(app.session.project().tracks[0].gain, 0.7533);
        assert!(app.session.project().tracks[0].polarity_inverted);
        assert!(!app.session.project().tracks[1].polarity_inverted);
        assert_eq!(app.session.project().master_gain, 0.7777);
    }

    #[test]
    fn mixer_increasing_silent_and_very_quiet_gain_does_not_stick_or_silence() {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.tracks[0].gain = 0.0;
        project.tracks[1].gain = 0.000_001;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())), project,
        ).unwrap();
        ctx.enable_accesskit();
        app.bottom_module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(app.session.project().tracks[0].gain > 0.0,
            "Increasing the numeric gain from silence must produce a positive gain.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 2 gain");
        frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(app.session.project().tracks[1].gain > 0.000_001,
            "Increasing a gain below -96 dB must increase it instead of silencing it.");
    }

    #[test]
    fn mixer_numeric_silence_origin_preserves_focus_and_exact_entries() {
        for initial_gain in [0.0, 10.0_f32.powf(-96.0 / 20.0), 0.000_001] {
            let (ctx, mut app) = fixture();
            let mut project = app.session.project().clone();
            project.tracks[0].gain = initial_gain;
            app.session = Session::headless(
                Arc::new(Library::fixture(app.session.library().samples.to_vec())), project,
            ).unwrap();
            ctx.enable_accesskit();
            app.bottom_module = BottomModule::Mixer;
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Track 1 gain value");
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Track 1 left output");
            assert_eq!(app.session.project().tracks[0].gain.to_bits(), initial_gain.to_bits());
            assert!(!app.session.unsaved());

            for entry in ["-inf", "-120", "-inf", "-96", "-inf"] {
                let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                focus_control(&ctx, &mut app, &output, "Track 1 gain value");
                frame(&ctx, &mut app, vec![
                    key_event(egui::Key::A, egui::Modifiers::COMMAND),
                    egui::Event::Text(entry.into()),
                ], egui::Modifiers::COMMAND,);
                frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
                let expected = if entry == "-inf" { 0.0 } else { mixjam_techno::application::db_to_amplitude(entry.parse::<f32>().unwrap()) };
                assert_eq!(app.session.project().tracks[0].gain, expected, "Explicit {entry} must retain its meaning.");
            }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Track 1 gain value");
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            frame(&ctx, &mut app, vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
            assert_eq!(app.session.project().tracks[0].gain, 0.0,
                "Decreasing a silent numeric control must keep silence.");
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Track 1 left output");
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(app.session.project().tracks[0].gain, 0.0,
                "Blurring after a decrease from silence must not commit a finite draft.");
            let rect = effects_control_rect(&output, "Track 1 gain value");
            let pos = rect.center();
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, pressed: true, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE,
            },], egui::Modifiers::NONE,);
            let pos = pos + Vec2::new(24.0, 0.0);
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(pos)], egui::Modifiers::NONE,);
            frame(&ctx, &mut app, vec![egui::Event::PointerButton {
                pos, pressed: false, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE,
            }], egui::Modifiers::NONE,);
            assert!(app.session.project().tracks[0].gain > 0.0,
                "Dragging the numeric control up from silence must raise gain.");
        }
    }

    #[test]
    fn mixer_retains_legacy_gain_four_until_an_explicit_db_edit() {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.tracks[0].gain = 4.0;
        project.tracks[0].polarity_inverted = true;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())), project,
        ).unwrap();
        ctx.enable_accesskit();
        app.bottom_module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, "+12.0 dB");
        assert_eq!(app.session.project().tracks[0].gain, 4.0);
        assert!(!app.session.unsaved());
        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 left output");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.session.project().tracks[0].gain, 4.0,
            "Focusing a legacy gain must preserve its exact amplitude above +12 dB.");
        assert!(app.session.project().tracks[0].polarity_inverted);
        assert!(!app.session.unsaved());

        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        frame(&ctx, &mut app, vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("+12".into()),
        ], egui::Modifiers::COMMAND,);
        frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(12.0 / 20.0)).abs() < 0.000_001,
            "An explicit +12 dB edit must use the new fader ceiling rather than legacy amplitude four.");
        assert!(app.session.project().tracks[0].polarity_inverted);
        assert!(app.session.unsaved());
        app.history_step(false);
        assert_eq!(app.session.project().tracks[0].gain, 4.0);
        assert!(app.session.project().tracks[0].polarity_inverted);
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
        app.bottom_module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let (_, gain) = accessible_node(&output, "Track 1 gain");
        assert_eq!(gain.role(), egui::accesskit::Role::Slider);
        assert_eq!(gain.numeric_value(), Some(0.0));
        assert!(gain.min_numeric_value().is_some_and(|db| db <= -60.0));
        assert_eq!(gain.max_numeric_value(), Some(12.0));
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
                egui::Event::Text("-6.0".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(-6.0 / 20.0)).abs() < 0.000_001);
        assert!(!app.session.project().tracks[0].polarity_inverted,
            "A negative dB value attenuates without reversing polarity.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 polarity invert");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Space, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert!(app.session.project().tracks[0].polarity_inverted);
        assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(-6.0 / 20.0)).abs() < 0.000_001);
        assert!(!app.session.wants_playback(), "A polarity key must not start playback.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(accessible_node(&output, "Track 1 polarity invert").1.toggled(),
            Some(egui::accesskit::Toggled::True));
        focus_control(&ctx, &mut app, &output, "Reset Track 1 gain to unity");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().tracks[0].gain, 1.0);
        assert!(app.session.project().tracks[0].polarity_inverted,
            "Unity changes magnitude without changing polarity.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 gain");
        for (key, modifiers, expected_db) in [
            (egui::Key::ArrowUp, egui::Modifiers::SHIFT, 0.1_f32),
            (egui::Key::ArrowDown, egui::Modifiers::SHIFT, 0.0),
            (egui::Key::ArrowDown, egui::Modifiers::NONE, -0.5),
            (egui::Key::Home, egui::Modifiers::NONE, f32::NEG_INFINITY),
            (egui::Key::ArrowDown, egui::Modifiers::NONE, f32::NEG_INFINITY,),
            (egui::Key::ArrowDown, egui::Modifiers::SHIFT, f32::NEG_INFINITY,),
            (egui::Key::End, egui::Modifiers::NONE, 12.0),
            (egui::Key::ArrowUp, egui::Modifiers::NONE, 12.0),
        ] {
            frame(&ctx, &mut app, vec![key_event(key, modifiers)], modifiers);
            let expected_gain = 10.0_f32.powf(expected_db / 20.0);
            assert!((app.session.project().tracks[0].gain - expected_gain).abs() < 0.000_001,
                "{key:?} with {modifiers:?} must set {expected_db} dB; lowering silence must keep exact zero.");
            assert!(app.session.project().tracks[0].polarity_inverted,
                "Gain rail keys must leave polarity unchanged.");
        }
        assert!(!app.session.wants_playback(), "Gain rail keys must not start playback.");
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
        app.bottom_module = BottomModule::Mixer;
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
    fn mixer_polarity_survives_silence_history_and_project_save() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.bottom_module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        frame(&ctx, &mut app, vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-inf".into()),
        ], egui::Modifiers::COMMAND,);
        frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.session.project().tracks[0].gain, 0.0);
        assert!(!app.session.project().tracks[0].polarity_inverted);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 polarity invert");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Space, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.session.project().tracks[0].gain, 0.0);
        assert!(app.session.project().tracks[0].polarity_inverted,
            "Polarity must remain a real independent setting at silence.");
        app.history_step(false);
        assert_eq!(app.session.project().tracks[0].gain, 0.0);
        assert!(!app.session.project().tracks[0].polarity_inverted);
        app.history_step(true);
        assert_eq!(app.session.project().tracks[0].gain, 0.0);
        assert!(app.session.project().tracks[0].polarity_inverted);

        let path = std::env::temp_dir().join(format!(
            "mixjam-ui-mixer-polarity-{}.json", std::process::id()
        ));
        app.session.project().save(&path).unwrap();
        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded.tracks[0].gain, 0.0);
        assert!(loaded.tracks[0].polarity_inverted);
        std::fs::remove_file(path).unwrap();

        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Reset Track 1 gain to unity");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Space, egui::Modifiers::NONE)], egui::Modifiers::NONE,);
        assert_eq!(app.session.project().tracks[0].gain, 1.0);
        assert!(app.session.project().tracks[0].polarity_inverted);
        assert_eq!(app.session.project().tracks[0].effective_gain(), -1.0);
        assert_eq!(app.session.project().tracks[1].gain, 1.0);
        assert!(!app.session.project().tracks[1].polarity_inverted);
    }

    #[test]
    fn mixer_compact_rails_and_inline_resets_fit_supported_densities() {
        for density in [1.0, 1.5, 2.0] {
            let (ctx, mut app) = fixture();
            ctx.set_pixels_per_point(density);
            ctx.enable_accesskit();
            let mut project = app.session.project().clone();
            project.tracks[0].gain = 10.0_f32.powf(12.0 / 20.0);
            project.tracks[1].gain = 0.0;
            app.session = Session::headless(
                Arc::new(Library::fixture(app.session.library().samples.to_vec())), project,
            ).unwrap();
            app.bottom_module = BottomModule::Mixer;
            let size = Vec2::new(960.0, 720.0);
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            for _ in 0..12 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let mut caps = Vec::new();
            for track in ["Track 1", "Track 2", "Master"] {
                let gain = effects_control_rect(&output, &format!("{track} gain"));
                let value = effects_control_rect(&output, &format!("{track} gain value"));
                let unity = effects_control_rect(&output, &format!("Reset {track} gain to unity"));
                assert!(viewport.contains_rect(gain) && viewport.contains_rect(value)
                    && viewport.contains_rect(unity), "{track} gain controls must fit at {density}x.");
                assert!(gain.height() >= 112.0, "The minimum-window rail must remain usable at {density}x.");
                assert!(value.bottom() <= gain.top(), "The dB readout must have room above its rail.");
                let cap = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) => {
                        let bounds = mesh.calc_bounds();
                        ((bounds.width() - 24.0).abs() <= 1.0 / density
                            && (bounds.height() - 32.0).abs() <= 1.0 / density
                            && (bounds.center().x - gain.center().x).abs() <= 1.0 / density
                            && gain.expand(1.0 / density).contains(bounds.center())).then_some(bounds)
                    }
                    _ => None,
                }).unwrap_or_else(|| panic!("Missing {track} fader cap at {density}x."));
                assert!(cap.top() - value.bottom() >= 4.0 - 1.0 / density,
                    "The fader cap must clear the dB readout even at +12 dB.");
                caps.push((track, cap));
                assert!(gain.y_range().contains(unity.center().y), "Unity must sit directly beside the gain rail.");
                assert!(unity.width() <= 48.0 && unity.height() <= 20.0, "Unity must be a small rail button.");
            }
            for track in ["Track 1", "Track 2"] {
                let pan = effects_control_rect(&output, &format!("{track} pan"));
                let center = effects_control_rect(&output, &format!("Center {track} pan"));
                let polarity = effects_control_rect(&output, &format!("{track} polarity invert"));
                let left = effects_control_rect(&output, &format!("{track} left output"));
                let right = effects_control_rect(&output, &format!("{track} right output"));
                for rect in [pan, center, polarity, left, right] {
                    assert!(viewport.contains_rect(rect), "{track} lower controls must fit at {density}x.");
                }
                let cap = caps.iter().find(|(name, _)| *name == track).unwrap().1;
                assert!(pan.top() - cap.bottom() >= 4.0 - 1.0 / density,
                    "The fader cap must clear pan even at silence.");
                assert!(center.top() >= pan.bottom() - 1.0 / density
                    && center.top() <= pan.bottom() + 4.0,
                    "Center must attach just below its pan rail without covering the slider.");
                assert!((pan.center().x - center.center().x).abs() <= 1.0 / density,
                    "Center must mark the center of the pan rail.");
                assert!(center.width() <= 44.0 && center.height() <= 18.0);
                assert!(!polarity.intersects(pan) && !polarity.intersects(center));
                assert!(!polarity.intersects(left) && !polarity.intersects(right));
                assert_eq!(accessible_node(&output, &format!("{track} polarity invert")).1.role(),
                    egui::accesskit::Role::CheckBox);
            }
        }
    }

    #[test]
    fn mixer_gain_taper_reaches_silence_and_favors_musically_useful_levels() {
        assert_eq!(mixer::fraction_gain(0.0), 0.0);
        assert_eq!(mixer::gain_fraction(0.0), 0.0);
        assert!((20.0 * mixer::fraction_gain(1.0).log10() - 12.0).abs() < 0.000_01);
        let useful_travel = mixer::gain_fraction(10.0_f32.powf(12.0 / 20.0))
            - mixer::gain_fraction(10.0_f32.powf(-24.0 / 20.0));
        assert!(useful_travel >= 0.65,
            "Most travel must serve -24 through +12 dB rather than near-silent levels.");
        assert!(mixer::gain_fraction(10.0_f32.powf(-60.0 / 20.0)) <= 0.12,
            "The near-silent range must use only a short part of the rail.");
        let mut previous = 0.0;
        for position in 1..=1000 {
            let fraction = position as f32 / 1000.0;
            let gain = mixer::fraction_gain(fraction);
            assert!(gain > previous, "Increasing the fader must always increase amplitude.");
            assert!((mixer::gain_fraction(gain) - fraction).abs() < 0.000_01);
            previous = gain;
        }
        for gain in [0.000_000_01, 0.001, 0.01, 0.1, 0.5, 1.0, 2.0] {
            let restored = mixer::fraction_gain(mixer::gain_fraction(gain));
            assert!((restored - gain).abs() <= gain * 0.000_01,
                "The rail must round-trip small positive amplitudes without an artificial silence floor.");
        }
    }

    #[test]
    fn bottom_module_selector_stays_fixed_and_preserves_sound_state_across_input_methods() {
        fn action(action: egui::accesskit::Action, target: egui::accesskit::NodeId) -> egui::Event {
            egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest { action, target, data: None, })
        }
        fn pointer_events(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
            vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, pressed, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE,
            },]
        }
        for size in [Vec2::new(1440.0, 1000.0), Vec2::new(1024.0, 768.0), Vec2::new(960.0, 720.0),] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.bottom_module = BottomModule::Mixer;
            app.search = "grasp".into();
            app.group = Some("Loop".into());
            app.palette_sort = PaletteSort::Name;
            app.selected_sample = Some(1);
            for _ in 0..12 { frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
            let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let selector_id = accessible_node(&output, "Bottom module").0;
            let selector = effects_control_rect(&output, "Bottom module");
            let panel = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds")).unwrap().rect;
            assert!(panel.height() >= 370.0);
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(selector));
            assert!((selector.width() - 136.0).abs() <= 1.0,
                "The selector must reserve the same fixed width for either module.");
            assert_eq!(accessible_node(&output, "Bottom module").1.role(), egui::accesskit::Role::ComboBox);
            accessible_node(&output, "Master gain");

            // Choose Sounds with the pointer, Mixer with focus and Enter,
            // then Sounds with the same accessibility actions used by agents.
            for (method, module) in [(0, BottomModule::Sounds), (1, BottomModule::Mixer), (2, BottomModule::Sounds),] {
                let known: std::collections::HashSet<_> = output.platform_output.accesskit_update.as_ref().unwrap()
                    .nodes.iter().map(|(id, _)| *id).collect();
                match method {
                    0 => {
                        frame_at_size(&ctx, &mut app, pointer_events(selector.center(), true), egui::Modifiers::NONE, size,);
                        frame_at_size(&ctx, &mut app, pointer_events(selector.center(), false), egui::Modifiers::NONE, size,);
                    }
                    1 => {
                        frame_at_size(&ctx, &mut app, vec![action(egui::accesskit::Action::Focus, selector_id)], egui::Modifiers::NONE, size,);
                        frame_at_size(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE, size,);
                    }
                    _ => { frame_at_size(&ctx, &mut app, vec![action(egui::accesskit::Action::Click, selector_id)], egui::Modifiers::NONE, size,); }
                }
                for _ in 0..2 { output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
                assert!(egui::Popup::is_any_open(&ctx), "The selector must open with method {method}.");
                let (option_id, option) = output.platform_output.accesskit_update.as_ref().unwrap().nodes.iter()
                    .find(|(id, node)| !known.contains(id) && node.label() == Some(module.label()))
                    .unwrap_or_else(|| panic!("Missing {} module choice.", module.label()));
                let option_id = *option_id;
                let bounds = option.bounds().unwrap();
                let center = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, (bounds.y0 + bounds.y1) as f32 / 2.0,);
                match method {
                    0 => {
                        frame_at_size(&ctx, &mut app, pointer_events(center, true), egui::Modifiers::NONE, size,);
                        frame_at_size(&ctx, &mut app, pointer_events(center, false), egui::Modifiers::NONE, size,);
                    }
                    1 => {
                        frame_at_size(&ctx, &mut app, vec![action(egui::accesskit::Action::Focus, option_id)], egui::Modifiers::NONE, size,);
                        frame_at_size(&ctx, &mut app, vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)], egui::Modifiers::NONE, size,);
                    }
                    _ => { frame_at_size(&ctx, &mut app, vec![action(egui::accesskit::Action::Click, option_id)], egui::Modifiers::NONE, size,); }
                }
                for _ in 0..6 { output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size); }
                assert_eq!(app.bottom_module, module, "Method {method}, viewport {size:?}.");
                assert!(!egui::Popup::is_any_open(&ctx), "Module choice must close after method {method}, viewport {size:?}.");
                assert_eq!(accessible_node(&output, "Bottom module").0, selector_id);
                assert_eq!(effects_control_rect(&output, "Bottom module"), selector,
                    "Module navigation must not move its selector at {size:?}.");
                let title = title_text(module.label());
                let painted = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == title => Some(text),
                    _ => None,
                }).expect("The module selector must paint its selected name.");
                assert!(painted.galley.job.sections.iter().all(|section|
                    section.format.font_id.family == egui::FontFamily::Name("Syncopate Bold".into())
                        && section.format.font_id.size == 16.0));
                assert_eq!(egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds")).unwrap().rect, panel,
                    "Choosing a module must preserve the existing panel size.");
                assert_eq!(app.search, "grasp");
                assert_eq!(app.group.as_deref(), Some("Loop"));
                assert_eq!(app.palette_product, None);
                assert_eq!(app.palette_sort, PaletteSort::Name);
                assert_eq!(app.selected_sample, Some(1));
                let nodes = &output.platform_output.accesskit_update.as_ref().unwrap().nodes;
                assert!(!nodes.iter().any(|(_, node)| node.label() == Some("Show Mixer")
                    || (node.label() == Some("Sounds") && node.role() == egui::accesskit::Role::Button)),
                    "The shared selector replaces the former module swap buttons.");
                if module == BottomModule::Sounds {
                    for name in ["Find a sound", "Library filter", "Category filter", "Sort order",] {
                        accessible_node(&output, name);
                    }
                    assert!(!nodes.iter().any(|(_, node)| node.label() == Some("Master gain")));
                } else {
                    accessible_node(&output, "Master gain");
                    assert!(!nodes.iter().any(|(_, node)| node.label() == Some("Find a sound")));
                }
            }
            assert!(!app.session.unsaved(), "Module navigation must not edit the project.");
        }
    }

    #[test]
    fn empty_mixer_keeps_guidance_master_and_action_errors_visible() {
        let (ctx, mut app) = fixture();
        app.new_mix();
        app.bottom_module = BottomModule::Mixer;
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
        let shared_minimum = BottomModule::ALL.into_iter()
            .map(BottomModule::minimum_height).fold(0.0_f32, f32::max);
        assert!(panel().height() <= shared_minimum,
            "The minimum viewport must clamp to the shared module height limit.");
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
            app.palette_sort = sort;
            assert_eq!(app.palette_indices(), expected, "{sort:?}");
            assert_eq!(*app.palette_view().0, expected, "{sort:?}");
        }
    }

    #[test]
    fn sound_tiles_span_their_bar_count_and_wrap_within_the_row() {
        assert_eq!(tile_width(1, 800.0), TILE_BAR);
        assert_eq!(tile_width(2, 800.0), 2.0 * TILE_BAR + TILE_GAP);
        assert_eq!(tile_width(4, 800.0), 4.0 * TILE_BAR + 3.0 * TILE_GAP);
        // A long take fills the row; a narrow panel still shows a whole one-bar tile.
        assert_eq!(tile_width(32_766, 800.0), 800.0);
        assert_eq!(tile_width(4, 40.0), TILE_BAR);
        let widths = [1, 2, 4, 1, 8, 1].map(|bars| tile_width(bars, 420.0));
        assert_eq!(tile_rows(widths, 420.0), [0, 2, 3, 4, 5]);
        assert!(tile_rows([], 420.0).is_empty());
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
        assert_eq!(app.bottom_module, BottomModule::Mixer);
        assert!(clicked.platform_output.events.iter().any(|event| {
            matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some("Mixer"))
        }), "The icon must expose its action name to accessibility output.");
        click(&ctx, &mut app, mixer, egui::Modifiers::NONE);
        assert_eq!(app.bottom_module, BottomModule::Mixer,
            "The Mixer rail shortcut must keep Mixer open on a repeated click.");
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
                Pcm { sample_rate: 100, channels: 1, samples: vec![0.25; 200], }
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
                    pressed, modifiers: egui::Modifiers::NONE, },], egui::Modifiers::NONE, size,);
        }
        for _ in 0..20 {
            frame_at_size(&context, &mut app, vec![egui::Event::PointerMoved(position),
                egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -500.0), modifiers: egui::Modifiers::NONE, },],
                egui::Modifiers::NONE, size,);
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

    fn agent_fixture() -> (egui::Context, MixJamApp, (u16, String)) {
        let (context, mut app) = fixture();
        let folder = std::env::temp_dir().join(format!("mixjam-ui-agent-key-{}", std::process::id()));
        let server = crate::agent::Server::start(0, &folder).unwrap();
        let port = (server.port(), server.key().to_string());
        app.attach_agent(server, &context);
        (context, app, port)
    }

    /// Send one request through the control channel, as the `agent` command
    /// does, and run frames until the window has replied.
    fn agent(context: &egui::Context, app: &mut MixJamApp, port: &(u16, String), words: &[&str],) -> serde_json::Value {
        let words: Vec<String> = words.iter().map(|word| (*word).to_string()).collect();
        let (port, key) = port.clone();
        let client = std::thread::spawn(move || {
            let mut output = Vec::new();
            crate::agent::send(port, &key, [crate::agent::request_line(&words)], &mut output,).unwrap();
            String::from_utf8(output).unwrap()
        });
        let started = std::time::Instant::now();
        while !client.is_finished() {
            assert!(started.elapsed() < Duration::from_secs(60), "The window did not reply.");
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                ..Default::default()
            };
            app.agent_frame(context, &mut input);
            let _ = context.run(input, |ctx| app.draw(ctx));
            std::thread::sleep(Duration::from_millis(1));
        }
        serde_json::from_str(&client.join().unwrap()).unwrap()
    }

    #[test]
    fn agent_operates_windows_sliders_toggles_text_and_combo_boxes_by_name() {
        let (context, mut app, port) = agent_fixture();
        let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);

        let reply = send(&mut app, &["ui"]);
        let nodes = reply["nodes"].as_array().unwrap();
        for (label, role) in [
            ("Play / pause (Space)", "button"),
            ("Project BPM", "combo_box"),
            ("Bottom module", "combo_box"),
            ("Zoom", "slider"),
            ("Find a sound", "text_input"),
            ("Library filter", "combo_box"),
            ("Category filter", "combo_box"),
            ("Sort order", "combo_box"),
            ("Recording track", "combo_box"),
            ("Record on track 1", "radio"),
            ("Arrangement lanes", "other"),
            ("Bar ruler", "other"),
            ("good grasp 2", "button"),
        ] {
            assert!(nodes.iter().any(|node| node["label"] == label && node["role"] == role), "Missing {role} {label}");
        }
        assert!(nodes.iter().all(|node| node["role"] != "label" && node["enabled"] == true));
        let reply = send(&mut app, &["ui", "all=true", "contains=ui fixture"]);
        assert!(reply["nodes"].as_array().unwrap().iter().any(|node| node["role"] == "label" && node["label"] == "UI fixture"), "{reply}");

        let reply = send(&mut app, &["click", "target=Effects"]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert!(app.show_effects);
        assert_eq!(reply["windows"], serde_json::json!(["Effect Studio"]));
        let reply = send(&mut app, &["set", "target=Filter", "value=true", "window=Effect Studio"],);
        assert_eq!(reply["control"]["checked"], true, "{reply}");
        assert!(app.effects.filter.is_some());
        let reply = send(&mut app, &["set", "target=Cutoff", "value=1000"]);
        assert_eq!(reply["control"]["number"], 1000.0, "{reply}");
        assert_eq!(app.effects.filter.as_ref().unwrap().cutoff_hz, 1000.0);
        let reply = send(&mut app, &["set", "target=Filter", "value=false"]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert!(app.effects.filter.is_none());

        let reply = send(&mut app, &["set", "target=Project BPM", "value=160"]);
        assert_eq!(reply["control"]["text"], "160", "{reply}");
        assert_eq!(app.session.project().bpm, 160.0);
        assert!(!egui::Popup::is_any_open(&context));
        let reply = send(&mut app, &["set", "target=Project BPM", "value=150"]);
        assert_eq!(reply["ok"], false);
        assert!(reply["error"].as_str().unwrap().contains("120, 140, 160, 180"), "{reply}");
        assert!(!egui::Popup::is_any_open(&context));
        assert_eq!(app.session.project().bpm, 160.0);

        // The module selector and the Sounds panel actions have distinct names.
        let reply = send(&mut app, &["click", "target=Step synth"]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert!(app.show_step_synth && app.group.is_none());
        send(&mut app, &["click", "target=Show Step synth sounds"]);
        assert_eq!(app.group.as_deref(), Some("Step synth"));
        send(&mut app, &["click", "target=Show All sounds"]);
        let reply = send(&mut app, &["set", "target=Bottom module", "value=Mixer"]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(app.bottom_module, BottomModule::Mixer);
        // A name that several controls contain needs a selector.
        let reply = send(&mut app, &["click", "target=gain"]);
        assert!(reply["error"].as_str().unwrap().contains("index="), "{reply}");
        // A choice can be named by its distinct word.
        send(&mut app, &["select", "sample=1"]);
        send(&mut app, &["click", "target=Stretch"]);
        let reply = send(&mut app, &["set", "target=Target BPM", "value=180"]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(app.stretch_target_bpm, 180.0);
        let reply = send(&mut app, &["ui", "window=Time Stretcher"]);
        assert!(reply["nodes"].as_array().unwrap().iter().any(|node| node["label"] == "Source sound"), "{reply}");
        send(&mut app, &["click", "target=Close Time Stretcher"]);
        let reply = send(&mut app, &["set", "target=Track 1 gain", "value=0.5"]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(0.5 / 20.0)).abs() < 0.000_001);
        assert!(!app.session.project().tracks[0].polarity_inverted);
        send(&mut app, &["click", "target=Undo"]);
        assert_eq!(app.session.project().tracks[0].gain, Project::default().tracks[0].gain);

        send(&mut app, &["set", "target=Bottom module", "value=Sounds"]);
        let reply = send(&mut app, &["set", "target=Find a sound", "value=grasp"]);
        assert_eq!(reply["control"]["text"], "grasp", "{reply}");
        assert_eq!(app.search, "grasp");
        send(&mut app, &["set", "target=Find a sound", "value="]);
        assert_eq!(app.search, "");

        let reply = send(&mut app, &["click", "target=No such control"]);
        assert_eq!(reply["ok"], false);
        let reply = send(&mut app, &["key", "key=Escape"]);
        assert_eq!(reply["ok"], true, "{reply}");
    }

    #[test]
    fn agent_arranges_clips_by_command_and_by_pointer() {
        let (context, mut app, port) = agent_fixture();
        let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);

        let reply = send(&mut app, &["samples", "search=GRASP"]);
        assert_eq!(reply["total"], 1, "{reply}");
        assert_eq!(reply["samples"][0]["id"], 1);
        let reply = send(&mut app, &["place", "sample=good grasp 2", "track=3", "bar=5"],);
        assert_eq!(reply["ok"], true, "{reply}");
        let placed = reply["clips"][0].as_u64().unwrap();
        let clip = app.session.project().clips.iter().find(|clip| clip.id == placed).unwrap();
        assert_eq!((clip.track, clip.start_tick), (2, 4 * TICKS_PER_BAR));
        assert_eq!(app.selected_clips, [placed].into_iter().collect());
        for words in [
            &["place", "sample=1", "track=0", "bar=1"][..],
            &["place", "sample=1", "track=65", "bar=1"],
            &["place", "sample=nothing", "track=1", "bar=1"],
            &["place", "sample=1", "track=3", "bar=5"],
            &["delete_clips", "clips=[999]"],
            &["select", "clips=[999]"],
        ] {
            let reply = send(&mut app, words);
            assert_eq!(reply["ok"], false, "{words:?}: {reply}");
        }
        let count = app.session.project().clips.len();
        let reply = send(&mut app, &["move_clips", &format!("clips=[{placed}]"), "tracks=1", "bars=2", "copy=true",],);
        let copy = reply["clips"][0].as_u64().unwrap();
        assert_ne!(copy, placed);
        let clip = app.session.project().clips.iter().find(|clip| clip.id == copy).unwrap();
        assert_eq!((clip.track, clip.start_tick), (3, 6 * TICKS_PER_BAR));
        let reply = send(&mut app, &["delete_clips", &format!("clips=[{copy}]")]);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(app.session.project().clips.len(), count);

        send(&mut app, &["seek", "bar=3"]);
        assert_eq!(app.session.cursor_tick(), 2 * TICKS_PER_BAR);
        send(&mut app, &["set_loop", "first_bar=2", "last_bar=3"]);
        assert_eq!(app.session.project().loop_region, Some([TICKS_PER_BAR, 3 * TICKS_PER_BAR]));
        let state = send(&mut app, &["state"]);
        assert_eq!(state["song"]["loop_bars"], serde_json::json!([2, 3]));
        send(&mut app, &["set_loop"]);
        assert_eq!(app.session.project().loop_region, None);

        // The state gives each clip's place in the window for pointer input.
        send(&mut app, &["select"]);
        assert!(app.selected_clips.is_empty() && app.selected_sample.is_none());
        let state = send(&mut app, &["state"]);
        assert_eq!(state["song"]["tracks"].as_array().unwrap().len(), 3);
        assert_eq!(state["arrangement"]["tracks"], serde_json::json!([1, 2, 3, 4]));
        let second = state["song"]["clips"].as_array().unwrap().iter().find(|clip| clip["track"] == 2).unwrap();
        let rect: Vec<f32> = second["rect"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap() as f32).collect();
        let (x, y) = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
        let reply = send(&mut app, &["pointer", "action=click", &format!("x={x}"), &format!("y={y}"),],);
        assert_eq!(reply["ok"], true, "{reply}");
        let id = second["id"].as_u64().unwrap();
        assert_eq!(app.selected_clips, [id].into_iter().collect());
        let state = send(&mut app, &["state"]);
        assert_eq!(state["selection"], serde_json::json!({"sample": 1, "clips": [id]}));
        // Dragging the clip to the Add row and two bars right moves it, as a person's drag does.
        let bar_width = state["arrangement"]["bar_width"].as_f64().unwrap() as f32;
        let row_height = state["arrangement"]["row_height"].as_f64().unwrap() as f32;
        let reply = send(&mut app, &[
            "pointer", "action=drag", &format!("x={x}"), &format!("y={y}"),
            &format!("to_x={}", x + 2.0 * bar_width), &format!("to_y={}", y + 2.0 * row_height),
        ],);
        assert_eq!(reply["ok"], true, "{reply}");
        let clip = app.session.project().clips.iter().find(|clip| clip.id == id).unwrap();
        assert_eq!((clip.track, clip.start_tick), (3, 3 * TICKS_PER_BAR));
        let reply = send(&mut app, &["pointer", "action=click", "x=-5", "y=10"]);
        assert_eq!(reply["ok"], false);
    }

    #[test]
    fn agent_types_file_paths_in_the_choose_path_modal_and_resumes_a_pending_save() {
        let (context, mut app, port) = agent_fixture();
        let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
        let dir = std::env::temp_dir().join(format!("mixjam-ui-agent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let reply = send(&mut app, &["click", "target=Export"]);
        assert_eq!(reply["modal"], "Choose path", "{reply}");
        let state = send(&mut app, &["state"]);
        assert_eq!(state["path_prompt"]["kind"], "save_file");
        assert_eq!(state["path_prompt"]["extensions"], serde_json::json!(["wav"]));
        let reply = send(&mut app, &["click", "target=Undo"]);
        assert!(reply["error"].as_str().unwrap().contains("Choose path modal"), "{reply}");
        let reply = send(&mut app, &["click", "target=Cancel"]);
        assert_eq!(reply["modal"], serde_json::Value::Null, "{reply}");
        assert!(!app.session.is_exporting());

        // An edit, then New: the unsaved-changes modal saves through a typed path.
        send(&mut app, &["place", "sample=1", "track=5", "bar=1"]);
        let reply = send(&mut app, &["click", "target=New"]);
        assert_eq!(reply["modal"], "Save your changes?", "{reply}");
        let reply = send(&mut app, &["click", "target=Save changes"]);
        assert_eq!(reply["modal"], "Choose path", "{reply}");
        send(&mut app, &["set", "target=Path", "value=song"]);
        let reply = send(&mut app, &["click", "target=Confirm path"]);
        assert_eq!(reply["modal"], "Choose path", "A relative path must be refused: {reply}");
        let reply = send(&mut app, &["ui", "all=true", "contains=full path"]);
        assert!(reply["nodes"].as_array().unwrap().len() >= 2, "{reply}");
        // The extension of the file type is added, as a native dialog adds it.
        let song = dir.join("agent song");
        let reply = send(&mut app, &["set", "target=Path", &format!("value={}", song.display())],);
        assert_eq!(reply["control"]["text"], song.display().to_string(), "{reply}");
        let reply = send(&mut app, &["click", "target=Confirm path"]);
        assert_eq!(reply["ok"], true, "{reply}");
        let song = dir.join("agent song.mixjam");
        assert!(song.is_file(), "{reply}");
        let reply = send(&mut app, &["state"]);
        assert_eq!(reply["modal"], serde_json::Value::Null, "{reply}");
        assert_eq!(reply["status"]["text"], "New mix.");
        assert!(app.session.project().clips.is_empty());

        let reply = send(&mut app, &["click", "target=Open"]);
        assert_eq!(reply["modal"], "Choose path", "{reply}");
        send(&mut app, &["set", "target=Path", &format!("value={}", song.display())],);
        send(&mut app, &["key", "key=Enter"]);
        assert_eq!(app.session.file(), Some(song.as_path()));
        assert_eq!(app.session.project().clips.len(), 3);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn agent_refuses_a_screenshot_of_a_minimized_window() {
        let (context, mut app, (port, key)) = agent_fixture();
        let client = std::thread::spawn(move || {
            let mut output = Vec::new();
            let words = ["screenshot", r"path=C:\Temp\window.png"].map(str::to_string);
            crate::agent::send(port, &key, [crate::agent::request_line(&words)], &mut output,).unwrap();
            String::from_utf8(output).unwrap()
        });
        // The platform reports a minimized window with each frame's input.
        while !client.is_finished() {
            let info = egui::ViewportInfo {
                minimized: Some(true),
                ..Default::default()
            };
            let mut input = egui::RawInput {
                viewports: [(egui::ViewportId::ROOT, info)].into_iter().collect(),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                ..Default::default()
            };
            app.agent_frame(&context, &mut input);
            let _ = context.run(input, |ctx| app.draw(ctx));
            std::thread::sleep(Duration::from_millis(1));
        }
        let reply: serde_json::Value = serde_json::from_str(&client.join().unwrap()).unwrap();
        assert_eq!(reply["ok"], false, "{reply}");
        assert!(reply["error"].as_str().unwrap().contains("minimized"), "{reply}");
    }

    #[test]
    fn agent_quit_asks_about_unsaved_work_and_replies_before_the_window_closes() {
        let (context, mut app, port) = agent_fixture();
        let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
        send(&mut app, &["place", "sample=1", "track=5", "bar=1"]);
        let reply = send(&mut app, &["quit"]);
        assert_eq!(reply["modal"], "Save your changes?", "{reply}");
        assert!(reply.get("closing").is_none() && !app.allow_close);
        // Controls behind the modal stay out of reach, as they are for a pointer.
        let reply = send(&mut app, &["click", "target=Undo"]);
        assert!(reply["error"].as_str().unwrap().contains("Save your changes? modal"), "{reply}");
        assert!(app.session.unsaved());
        let reply = send(&mut app, &["click", "target=Discard changes"]);
        assert_eq!(reply["closing"], true, "{reply}");
        assert!(app.allow_close);
    }

    #[test]
    fn hardware_utility_windows_keep_identity_keyboard_close_and_scoped_style() {
        for title in [
            "Help", "Time Stretcher help", "Effect Studio help", "Step synth help",
            "Sample libraries", "Import details",
        ] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            let original_style = ctx.style();
            match title {
                "Help" => app.show_help = true,
                "Time Stretcher help" => app.show_stretch_help = true,
                "Effect Studio help" => app.show_effects_help = true,
                "Step synth help" => app.show_step_synth_help = true,
                "Sample libraries" => app.show_libraries = true,
                "Import details" => app.show_import_details = true,
                _ => unreachable!(),
            }
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let uppercase = title_text(title);
            let identity = accessible_node(&output, &uppercase).0;
            assert!(ctx.memory(|memory| memory.area_rect(egui::Id::new(title))).is_some(),
                "{title} must keep the original mixed-case native area ID.");
            let next = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(accessible_node(&next, &uppercase).0, identity,
                "{title} must retain its native window identity between frames.");
            assert_eq!(ctx.style().text_styles, original_style.text_styles);
            assert_eq!(ctx.style().visuals.widgets.inactive.bg_fill,
                original_style.visuals.widgets.inactive.bg_fill,
                "The hardware body theme must not change the application theme.");
            focus_control(&ctx, &mut app, &next, &format!("Close {title}"));
            frame(&ctx, &mut app,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,);
            assert!(!match title {
                "Help" => app.show_help,
                "Time Stretcher help" => app.show_stretch_help,
                "Effect Studio help" => app.show_effects_help,
                "Step synth help" => app.show_step_synth_help,
                "Sample libraries" => app.show_libraries,
                "Import details" => app.show_import_details,
                _ => unreachable!(),
            }, "{title} must keep the keyboard-accessible native X action.");
        }
    }

    #[test]
    fn hardware_blocking_dialogs_keep_named_identity_and_safe_explicit_cancel() {
        for identity in ["Save your changes?", "Import sound", "Choose path"] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            let project = app.session.project().clone();
            let unsaved = app.session.unsaved();
            match identity {
                "Save your changes?" => app.pending_action = Some(PendingAction::New),
                "Import sound" => app.import_path = Some(PathBuf::from(r"C:\Sounds\unread.wav")),
                "Choose path" => { app.path_prompt = Some(PathPrompt {
                    request: FileRequest::SaveSong,
                    text: String::new(), error: None, focused: false,
                })
                }
                _ => unreachable!(),
            }
            for _ in 0..3 { frame(&ctx, &mut app, vec![], egui::Modifiers::NONE); }
            let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(accessible_node(&output, identity).1.role(), egui::accesskit::Role::Dialog,
                "Visible title formatting must preserve the exact agent modal identity.");
            if identity == "Choose path" {
                assert_eq!(output.platform_output.accesskit_update.as_ref().unwrap().focus,
                    accessible_node(&output, "Path").0,
                    "The typed-path dialog must initially focus its path field.");
            }
            focus_control(&ctx,&mut app,&output,&format!("Collapse {identity}"));
            frame(&ctx,&mut app,vec![key_event(egui::Key::Enter,egui::Modifiers::NONE)],egui::Modifiers::NONE);
            output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
            assert!(dialog::is_collapsed(&ctx, identity));
            assert!(
                ctx.memory(|memory| memory.top_modal_layer()).is_none(),
                "Collapsed pending action must release workspace."
            );
            assert!(match identity {
                "Save your changes?" => app.pending_action.is_some(),
                "Import sound" => app.import_path.is_some(),
                _ => app.path_prompt.is_some(),
            });
            assert_eq!(app.session.project(), &project);
                focus_control(&ctx,&mut app,&output,&format!("Expand {identity}"));
            frame(&ctx,&mut app,vec![key_event(egui::Key::Enter,egui::Modifiers::NONE)],egui::Modifiers::NONE);
            output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Cancel");
            frame(&ctx, &mut app,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,);
            assert!(app.pending_action.is_none() && app.import_path.is_none() && app.path_prompt.is_none(),
                "Cancel must release the {identity} modal.");
            assert_eq!(app.session.project(), &project);
            assert_eq!(app.session.unsaved(), unsaved,
                "Cancel must not mutate the song or its saved state.");
        }
    }

    #[test]
    fn hardware_recorded_take_keeps_explicit_discard_and_no_window_close() {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.enable_accesskit();
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                focused: true, ..Default::default()
            }, |ctx| {
                dialog::window("Recorded take").show(ctx, |ui| {
                    ui.label("Unsaved take: stored in memory.");
                    ui.horizontal(|ui| {
                        for label in ["Listen", "Save take", "Discard take"] { let _ = ui.button(label); }
                    });
                });
            },);
        }
        let window = accessible_node(&output, "RECORDED TAKE").0;
        let nodes = &output.platform_output.accesskit_update.as_ref().unwrap().nodes;
        let mut descendants = BTreeSet::from([window]);
        for _ in 0..64 {
            let before = descendants.len();
            for (id, node) in nodes {
                if descendants.contains(id) { descendants.extend(node.children().iter().copied()); }
            }
            if descendants.len() == before { break; }
        }
        for label in ["Listen", "Save take", "Discard take"] {
            assert!(nodes.iter().any(|(id, node)| descendants.contains(id) && node.label() == Some(label)),
                "Recorded take must keep its explicit {label} action.");
        }
        assert!(!nodes.iter().any(|(id, node)| descendants.contains(id)
            && node.label() == Some("Close Recorded take")),
            "Unsaved take still requires explicit save or discard."
        );
        let collapse = accessible_node(&output, "Collapse Recorded take").0;
        let draw = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    dialog::window("Recorded take").show(ctx, |ui| {
                        ui.label("Unsaved take: stored in memory.");
                        for name in ["Listen", "Save take", "Discard take"] {
                            let _ = ui.button(name);
                        }
                    });
                },
            )
        };
        draw(vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: collapse,
                data: None,
            },
        )]);
        draw(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
        for _ in 0..3 {
            output = draw(vec![]);
        }
        assert!(dialog::is_collapsed(&ctx, "Recorded take"));
        assert!((effects_control_rect(&output, "RECORDED TAKE").height() - 44.0).abs() < 0.1);
        assert!(
            !output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Discard take"))
        );
        let expand = accessible_node(&output, "Expand Recorded take").0;
        draw(vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: expand,
                data: None,
            },
        )]);
        draw(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
        for _ in 0..3 {
            output = draw(vec![]);
        }
        text_rect(&output, "Unsaved take: stored in memory.");
        accessible_node(&output, "Save take");
        accessible_node(&output, "Discard take");
    }

    #[test]
    fn hardware_utility_short_and_tall_dialogs_fit_and_keep_content_reachable_at_all_densities() {
        let size = Vec2::new(960.0, 720.0);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        for density in [1.0, 1.5, 2.0] {
            for tall in [false, true] {
                let ctx = egui::Context::default();
                configure_style(&ctx);
                ctx.set_pixels_per_point(density);
                ctx.enable_accesskit();
                let mut open = true;
                let mut draw = |events: Vec<egui::Event>| { ctx.run(egui::RawInput {
                    screen_rect: Some(viewport), events, focused: true, ..Default::default()
                }, |ctx| {
                    dialog::window("Utility fixture").open(&mut open).default_width(440.0)
                        .max_height(500.0).constrain_to(viewport.shrink(12.0)).show(ctx, |ui| {
                            ui.label("First instruction");
                            if tall {
                                for row in 0..60 { ui.label(format!("Diagnostic row {row}: source details remain readable.")); }
                            }
                            let _ = ui.button("Last action");
                        });
                })
                };
                let mut output = egui::FullOutput::default();
                for _ in 0..4 { output = draw(vec![]); }
                assert_folded_assets(&ctx,&output,density,"Utility");
                assert!(viewport.contains_rect(text_rect(&output, "UTILITY FIXTURE")),
                    "The title must fit at {density}x.");
                let first = text_rect(&output, "First instruction");
                assert!(viewport.contains_rect(first), "The first instruction must fit at {density}x.");
                let visible = |output: &egui::FullOutput| { output.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Last action" => {
                        let bounds = Rect::from_min_size(text.pos, text.galley.size());
                        viewport.contains_rect(bounds) && shape.clip_rect.contains_rect(bounds)
                    }
                    _ => false,
                })
                };
                for _ in 0..30 {
                    if visible(&output) { break; }
                    output = draw(vec![egui::Event::PointerMoved(first.center()),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -160.0),
                            modifiers: egui::Modifiers::NONE,
                        },]);
                    for _ in 0..12 { output = draw(vec![]); }
                }
                assert!(visible(&output), "The last action must remain reachable in a {tall:?} dialog at {density}x.");
                assert!(viewport.contains_rect(text_rect(&output, "UTILITY FIXTURE")),
                    "Scrolling body content must not move the title out of view.");
            }
        }
    }

    #[test]
    fn resized_musical_content_keeps_nested_grid_effects_and_status_inside_the_chassis() {
        for (tool,title,status) in [("effects","EFFECT STUDIO","Listen before saving. Save sound adds a new WAV to Effect."),("step","STEP SYNTH","Click a note to set it. Click it again to clear it."),("step-effects","STEP SYNTH","Adjust sequence effects. Note grid returns to the notes."),("stretch","TIME STRETCHER","Same tempo. Duration is unchanged.")] {
            let (ctx,mut app)=fixture();ctx.enable_accesskit();
            ctx.style_mut(|style|{style.animation_time=0.0;style.scroll_animation=egui::style::ScrollAnimation::none();});
            app.selected_sample=Some(1);
            match tool {"effects"=>{app.show_effects=true;app.effects.filter=Some(Default::default());},"stretch"=>app.show_stretch=true,_=>{app.show_step_synth=true;app.step_synth_effects_expanded=tool=="step-effects";}}
            for _ in 0..4 {frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);}
            let output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
            let before=effects_control_rect(&output,title);
            let grip=before.right_bottom()-Vec2::splat(3.0);
            pointer(&ctx,&mut app,grip,true,egui::Modifiers::NONE);
            frame(&ctx,&mut app,vec![egui::Event::PointerMoved(grip-Vec2::new(200.0,150.0))],egui::Modifiers::NONE);
            pointer(&ctx,&mut app,grip-Vec2::new(200.0,150.0),false,egui::Modifiers::NONE);
            for _ in 0..3 {frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);}
            let mut output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);
            let window=effects_control_rect(&output,title);
            assert!(window.width()<before.width()-100.0 && window.height()<before.height()-100.0,"Resize must shrink {tool}.");
            let body=Rect::from_min_max(window.min+Vec2::new(12.0,56.0),window.max-Vec2::splat(12.0));
            for phase in 0..2 {
                let mut found=0;
                for shape in &output.shapes {
                    if let egui::Shape::Text(text)=&shape.shape {
                        let label=text.galley.job.text.as_str();
                        if shape.clip_rect.is_positive() && ["Cutoff","Filter","Sequence effects","Pitch","Original BPM",status].contains(&label) {
                            found+=1;
                            assert!(body.expand(0.1).contains_rect(shape.clip_rect),"{tool} {label} clip must stay in body after resize and scroll: {:?}/{body:?}",shape.clip_rect);
                        }
                    }
                }
                assert!(found>0,"Expected musical body paint in phase {phase}.");
                if phase==0 {
                    frame(&ctx,&mut app,vec![egui::Event::PointerMoved(body.left_top()+Vec2::splat(4.0)),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:Vec2::new(0.0,-1200.0),modifiers:egui::Modifiers::NONE}],egui::Modifiers::NONE);
                    for _ in 0..5 {output=frame(&ctx,&mut app,vec![],egui::Modifiers::NONE);}
                }
            }
            let message=output.shapes.iter().find(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text==status)).expect("Scrolling the shell must reach the status footer.");
            assert!(body.intersects(message.clip_rect));
        }
    }

    #[test]
    fn mixer_pointer_gain_edit_keeps_transport_and_history_shortcuts() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.bottom_module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let rail = effects_control_rect(&output, "Track 1 gain");
        click(&ctx, &mut app, rail.center_bottom() - Vec2::new(0.0, 20.0), egui::Modifiers::NONE,);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(app.session.project().tracks[0].gain < 1.0, "The rail click must lower the gain.");
        // Space, undo, and save share one guard: they run only while no control has focus.
        assert!(!ctx.wants_keyboard_input(),
            "A pointer edit must not leave the rail holding keyboard focus.");
        frame(&ctx, &mut app, vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)], egui::Modifiers::COMMAND,);
        assert_eq!(app.session.project().tracks[0].gain, 1.0,
            "Undo must work directly after a pointer gain edit.");
    }
}
