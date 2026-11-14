//! Scenario fixtures and assertions shared by several themed UI test modules: skin
//! checks, boards, missing media, agent and mix view setups.

use super::*;

/// Vintage paints the prepared Folded Metal artwork at one texel per
/// physical pixel. Studio and Aero paint their chassis in code and load
/// none of it.
pub(super) fn assert_folded_assets(
    ctx: &egui::Context,
    output: &egui::FullOutput,
    density: f32,
    palette: &str,
) {
    let body = match palette {
        "Marine" => controls::Palette::Marine,
        "Granite" => controls::Palette::Granite,
        "Red" => controls::Palette::Red,
        "Olive" => controls::Palette::Olive,
        "Utility" => controls::Palette::Utility,
        "Indigo" => controls::Palette::Indigo,
        _ => panic!("Unknown chassis palette"),
    }
    .body();
    let large = |rect: Rect| rect.width() > 300.0 && rect.height() > 100.0;
    match roles().material {
        theme::Material::Metal => {}
        material => {
            assert!(
                !ctx.tex_manager()
                    .read()
                    .allocated()
                    .any(|(_, meta)| meta.name.starts_with("folded-metal-")),
                "{} paints its chassis in code.",
                theme::current().label()
            );
            assert!(
                output.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) => {
                        material == theme::Material::Flat && rect.fill == body && large(rect.rect)
                    }
                    egui::Shape::Mesh(mesh) => {
                        material == theme::Material::Plastic
                            && mesh.texture_id == egui::TextureId::default()
                            && large(mesh.calc_bounds())
                    }
                    _ => false,
                }),
                "The {palette} chassis must paint its {} body.",
                theme::current().label()
            );
            return;
        }
    }
    let textures: Vec<_> = ctx
        .tex_manager()
        .read()
        .allocated()
        .filter(|(_, meta)| meta.name.starts_with(&format!("folded-metal-{palette}-")))
        .map(|(id, meta)| (*id, meta.size, meta.name.clone()))
        .collect();
    let dimensions = [
        (112, 64),
        (112, 64),
        (112, 64),
        (112, 64),
        (192, 12),
        (192, 12),
        (12, 96),
        (12, 96),
        (36, 36),
        (48, 24),
        (256, 256),
    ];
    for (index, (width, height)) in dimensions.into_iter().enumerate() {
        let part = if index < 8 {
            format!("part-{index}")
        } else if index == 8 {
            "knob".into()
        } else if index == 9 {
            "primary".into()
        } else {
            "grain".into()
        };
        let name = format!("folded-metal-{palette}-{part}");
        let expected = [width, height];
        assert!(
            textures
                .iter()
                .any(|(_, size, key)| *key == name && *size == expected),
            "{name} must load its prepared dimensions {expected:?}."
        );
    }
    let grain = textures
        .iter()
        .find(|(_, _, name)| name == &format!("folded-metal-{palette}-grain"))
        .unwrap()
        .0;
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape,egui::Shape::Mesh(mesh) if mesh.texture_id==grain)),
        "The chassis must paint its prepared grain tile."
    );
    assert!(
        output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Rect(rect) if rect.fill == body
                && rect.rect.width() > 300.0
                && rect.rect.height() > 100.0
        )),
        "The {palette} chassis must retain its assigned body color."
    );
    fn check(
        shape: &egui::Shape,
        textures: &[(egui::TextureId, [usize; 2], String)],
        density: f32,
        parts: &mut [bool; 8],
    ) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    check(shape, textures, density, parts);
                }
            }
            egui::Shape::Mesh(mesh) => {
                let Some((_, size, name)) =
                    textures.iter().find(|(id, _, _)| *id == mesh.texture_id)
                else {
                    return;
                };
                if let Some(part) = name
                    .split("-part-")
                    .nth(1)
                    .and_then(|part| part.split('-').next())
                    .and_then(|part| part.parse::<usize>().ok())
                {
                    parts[part] = true;
                }
                for vertex in &mesh.vertices {
                    for edge in [vertex.pos.x, vertex.pos.y] {
                        assert!(
                            (edge * density - (edge * density).round()).abs() < 0.001,
                            "Folded Metal raster vertices align to physical pixels."
                        );
                    }
                }
                for triangle in mesh.indices.as_chunks::<3>().0 {
                    for edge in 0..3 {
                        let a = mesh.vertices[triangle[edge] as usize];
                        let b = mesh.vertices[triangle[(edge + 1) % 3] as usize];
                        let dest = (b.pos - a.pos).abs() * density;
                        let source =
                            (b.uv - a.uv).abs() * Vec2::new(size[0] as f32, size[1] as f32);
                        assert!(
                            (dest.x - source.x).abs() < 0.001,
                            "Do not stretch material or control raster width."
                        );
                        assert!(
                            (dest.y - source.y).abs() < 0.001,
                            "Do not stretch material or control raster height."
                        );
                    }
                }
            }
            _ => {}
        }
    }
    let mut parts = [false; 8];
    for shape in &output.shapes {
        check(&shape.shape, &textures, density, &mut parts);
    }
    assert!(
        parts.into_iter().all(|painted| painted),
        "Paint every corner and edge of the Folded Metal chassis."
    );
}

pub(super) fn effects_text_mesh_bounds(output: &egui::FullOutput, text: &str) -> Rect {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(shape) if shape.galley.job.text == text => {
                Some(shape.galley.mesh_bounds.translate(shape.pos.to_vec2()))
            }
            _ => None,
        })
        .reduce(|bounds, next| bounds.union(next))
        .unwrap_or_else(|| panic!("Missing glyph mesh: {text}"))
}

pub(super) fn assert_full_recovery_caption(output: &egui::FullOutput) {
    let button = effects_control_rect(output, "Import details");
    let text = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.job.text == "Import details"
                    && button.contains(text.visual_bounding_rect().center()) =>
            {
                Some((
                    shape.clip_rect,
                    text.visual_bounding_rect(),
                    text.galley.elided,
                ))
            }
            _ => None,
        })
        .expect("Recovery button must paint the complete Import details caption.");
    assert!(!text.2, "Recovery button must not abbreviate its action.");
    assert!(
        button.shrink(6.0).contains_rect(text.1),
        "Recovery caption needs common horizontal and vertical padding."
    );
    assert!(
        text.0.contains_rect(text.1),
        "Recovery caption must remain visible."
    );
}

/// The board alone, filling the window.
pub(super) fn board(
    ctx: &egui::Context,
    chain: &MasterChain,
    metering: &master::Metering,
) -> master::Action {
    egui::CentralPanel::default()
        .show(ctx, |ui| master::show(ui, chain, metering, None))
        .inner
}

/// The app with a saved board, opened in Mastering and settled.
pub(super) fn mastering_fixture(
    change: impl Fn(&mut MasterChain),
) -> (egui::Context, MixJamApp, egui::FullOutput) {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    change(&mut project.master);
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let mut output = egui::FullOutput::default();
    for _ in 0..4 {
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    (ctx, app, output)
}

pub(super) fn stretch_layout_output(
    density: f32,
    missing: bool,
    feedback: stretch::Feedback<'_>,
) -> egui::FullOutput {
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
        output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                focused: true,
                ..Default::default()
            },
            |ctx| {
                stretch::show(
                    ctx,
                    &mut open,
                    stretch::Source {
                        name: "Sound",
                        id: Some(1),
                        samples: &[],
                        catalog_revision: 0,
                        missing,
                        bpm: 160.0,
                    },
                    stretch::Feedback {
                        processing: feedback.processing,
                        listening: feedback.listening,
                        saved: feedback.saved,
                        audition: feedback.audition,
                        message: feedback.message,
                        error: feedback.error,
                    },
                    &mut original,
                    &mut target,
                );
            },
        );
    }
    output
}

pub(super) fn missing_fixture() -> (egui::Context, MixJamApp) {
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
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    });
    project.missing_samples.push(missing);
    app.session = Session::headless(Arc::new(Library::fixture(vec![])), project).unwrap();
    app.options = None;
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    (context, app)
}

pub(super) fn sample_conflicts_fixture(tag: &str) -> (egui::Context, MixJamApp, PathBuf) {
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
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
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
    app.options = None;
    context.enable_accesskit();
    for _ in 0..3 {
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    }
    (context, app, dir)
}

/// Right-click a clip so its context menu shows its properties.
pub(super) fn open_clip_menu(context: &egui::Context, app: &mut MixJamApp, id: u64) {
    frame(context, app, vec![], egui::Modifiers::NONE);
    let clip = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| clip.id == id)
        .unwrap()
        .clone();
    let pos = app.clip_rect(&clip, app.score.rect.unwrap()).center();
    for pressed in [true, false] {
        frame(
            context,
            app,
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
}

pub(super) fn activate_control(
    context: &egui::Context,
    app: &mut MixJamApp,
    output: &egui::FullOutput,
    label: &str,
) -> egui::FullOutput {
    let target = step_synth_accessible_node(output, label).0;
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
    );
    frame(
        context,
        app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(context, app, vec![], egui::Modifiers::NONE)
}

/// The Step synth board: the lower panel's tab panel named Step synth.
pub(super) fn step_synth_board_node(
    output: &egui::FullOutput,
) -> (egui::accesskit::NodeId, &egui::accesskit::Node) {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled for this test.")
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == egui::accesskit::Role::TabPanel && node.label() == Some("Step synth")
        })
        .map(|(id, node)| (*id, node))
        .expect("The Step synth board must show.")
}

/// A control of the Step synth board, or of the tab row when the board
/// has none of that name (its help).
pub(super) fn step_synth_accessible_node<'a>(
    output: &'a egui::FullOutput,
    label: &str,
) -> (egui::accesskit::NodeId, &'a egui::accesskit::Node) {
    // The rail also has Effects. Use tree ownership.
    let window = step_synth_board_node(output).0;
    let nodes = &output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes;
    let mut descendants = BTreeSet::from([window]);
    for _ in 0..64 {
        let before = descendants.len();
        for (id, node) in nodes {
            if descendants.contains(id) {
                descendants.extend(node.children().iter().copied());
            }
        }
        if descendants.len() == before {
            break;
        }
    }
    nodes
        .iter()
        .find(|(id, node)| *id != window && descendants.contains(id) && node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| accessible_node(output, label))
}

pub(super) fn step_synth_control_rect(output: &egui::FullOutput, label: &str) -> Rect {
    let bounds = step_synth_accessible_node(output, label)
        .1
        .bounds()
        .unwrap();
    Rect::from_min_max(
        Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
        Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
    )
}

pub(super) fn agent_fixture() -> (egui::Context, MixJamApp, (u16, String)) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let (context, mut app) = fixture();
    let folder = std::env::temp_dir().join(format!(
        "mixjam-ui-agent-key-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let server = crate::agent::Server::start(0, &folder).unwrap();
    let port = (server.port(), server.key().to_string());
    // The tests send the key directly, so the key folder is not needed again.
    std::fs::remove_dir_all(&folder).unwrap();
    app.attach_agent(server, &context);
    (context, app, port)
}

/// Send one request through the control channel, as the `agent` command
/// does, and run frames until the window has replied.
pub(super) fn agent(
    context: &egui::Context,
    app: &mut MixJamApp,
    port: &(u16, String),
    words: &[&str],
) -> serde_json::Value {
    let words: Vec<String> = words.iter().map(|word| (*word).to_string()).collect();
    let (port, key) = port.clone();
    let client = std::thread::spawn(move || {
        let mut output = Vec::new();
        crate::agent::send(
            port,
            &key,
            [crate::agent::request_line(&words)],
            &mut output,
        )
        .unwrap();
        String::from_utf8(output).unwrap()
    });
    let started = std::time::Instant::now();
    while !client.is_finished() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "The window did not reply."
        );
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

/// Choose `label` in the arrangement's automation lane menu with
/// assistive technology clicks.
pub(super) fn choose_automation_lane(ctx: &egui::Context, app: &mut MixJamApp, label: &str) {
    let action = |action, target| {
        egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action,
            target,
            data: None,
        })
    };
    let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
    let nodes = |output: &egui::FullOutput| -> Vec<(egui::accesskit::NodeId, Option<String>)> {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.label().map(str::to_owned)))
            .collect()
    };
    let known: Vec<_> = nodes(&output).into_iter().map(|(id, _)| id).collect();
    let (selector, _) = accessible_node(&output, "Automation lane");
    frame(
        ctx,
        app,
        vec![action(egui::accesskit::Action::Click, selector)],
        egui::Modifiers::NONE,
    );
    frame(ctx, app, vec![], egui::Modifiers::NONE);
    let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
    // The option is a node that the open menu added.
    let option = nodes(&output)
        .into_iter()
        .find(|(id, name)| !known.contains(id) && name.as_deref() == Some(label))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("Missing lane choice {label}."));
    frame(
        ctx,
        app,
        vec![action(egui::accesskit::Action::Click, option)],
        egui::Modifiers::NONE,
    );
    for _ in 0..3 {
        frame(ctx, app, vec![], egui::Modifiers::NONE);
    }
}

/// The fixture in the Mix view at `size`, with its layout settled.
pub(super) fn mix_view_fixture(size: Vec2) -> (egui::Context, MixJamApp, egui::FullOutput) {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.set_mix_view(true);
    let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    (ctx, app, output)
}
