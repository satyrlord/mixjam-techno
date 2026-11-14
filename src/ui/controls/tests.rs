use super::*;

#[test]
fn hovered_destructive_buttons_keep_text_contrast() {
    for theme in theme::Theme::ALL {
        theme::paint_as(theme);
        let ctx = egui::Context::default();
        super::super::configure_style(&ctx);
        let rect = Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(180.0, 30.0));
        let face = rect.shrink(2.0).translate(-Vec2::Y);
        let mut output = egui::FullOutput::default();
        let mut hovered = false;
        for _ in 0..2 {
            output = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::PointerMoved(rect.center())],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = button_at(
                            ui,
                            rect,
                            "discard",
                            "Discard changes",
                            ButtonRole::Danger,
                            true,
                        );
                        hovered = response.hovered();
                    });
                },
            );
        }
        assert!(hovered);
        let mut colors = Vec::new();
        let mut gloss = Color32::TRANSPARENT;
        for shape in &output.shapes {
            match &shape.shape {
                egui::Shape::Rect(paint) if paint.rect == face => colors.push(paint.fill),
                egui::Shape::Mesh(mesh) if mesh.calc_bounds() == face => {
                    colors.extend(mesh.vertices.iter().map(|vertex| vertex.color));
                }
                egui::Shape::Rect(paint)
                    if face.contains_rect(paint.rect) && paint.fill.a() == theme::PLASTIC_GLOSS =>
                {
                    gloss = paint.fill;
                }
                _ => {}
            }
        }
        assert!(!colors.is_empty(), "The button must paint its face.");
        for color in colors {
            let ratio = contrast(roles().legend, color.blend(gloss));
            assert!(
                ratio >= 4.5,
                "{} hovered danger: {ratio:.2}:1",
                theme.label()
            );
        }
    }
    theme::paint_as(theme::Theme::Studio);
}

#[test]
fn loading_status_keeps_text_above_visible_rail() {
    for fraction in [None, Some(0.0), Some(0.5), Some(1.0)] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                apply_palette(ui, Palette::Utility);
                progress(ui, "Listing files…", fraction, 180.0);
            });
        });
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Listing files…" => Some(text),
                _ => None,
            })
            .unwrap();
        let rail = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == roles().ink && rect.rect.height() == 10.0 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("An empty loader still needs its full recessed track");
        assert!(text.pos.y + text.galley.size().y <= rail.top());
        assert_eq!(text.galley.job.sections[0].format.color, roles().legend);
        assert!(contrast(roles().legend, Palette::Utility.body()) >= 4.5);
        let update = output.platform_output.accesskit_update.unwrap();
        let node = update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == egui::accesskit::Role::ProgressIndicator)
            .unwrap();
        assert_eq!(
            node.1.numeric_value(),
            fraction.map(|value| f64::from(value) * 100.0)
        );
    }
}

#[test]
fn numeric_text_fits_and_centers_in_its_own_container() {
    for theme in theme::Theme::ALL {
        theme::paint_as(theme);
        for height in [18.0, 20.0, 24.0, 28.0] {
            let ctx = egui::Context::default();
            super::super::configure_style(&ctx);
            let rect = Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(100.0, height));
            let mut response_rect = Rect::NOTHING;
            let mut value = 2000.0_f64;
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    value_style(ui);
                    response_rect = ui
                        .put(rect, egui::DragValue::new(&mut value).fixed_decimals(0))
                        .rect;
                });
            });
            assert!(
                rect.contains_rect(response_rect),
                "Value widget {response_rect:?} must fit {rect:?}"
            );
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "2000" => {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .unwrap();
            let offset = (text.center() - rect.center()).abs();
            assert!(
                offset.x <= 1.0 && offset.y <= 1.0,
                "Visible numeric text offset {offset:?} in {height}-point container in {theme:?}"
            );
        }
    }
}

#[test]
fn folded_text_colors_stay_readable_on_their_actual_surfaces() {
    for theme in theme::Theme::ALL {
        theme::paint_as(theme);
        text_colors_stay_readable(theme.label());
    }
    theme::paint_as(theme::Theme::Studio);
}

fn text_colors_stay_readable(theme: &str) {
    for (role, text, background) in [
        ("utility labels", roles().legend, Palette::Utility.body()),
        ("marine labels", roles().legend, Palette::Marine.body()),
        ("granite labels", roles().legend, Palette::Granite.body()),
        ("red labels", roles().legend, Palette::Red.body()),
        ("olive labels", roles().legend, Palette::Olive.body()),
        ("indigo labels", roles().legend, Palette::Indigo.body()),
        ("primary labels", roles().ink, roles().primary),
        ("utility selection", roles().ink, Palette::Utility.accent()),
        ("marine selection", roles().ink, Palette::Marine.accent()),
        ("granite selection", roles().ink, Palette::Granite.accent()),
        ("red selection", roles().ink, Palette::Red.accent()),
        ("olive selection", roles().ink, Palette::Olive.accent()),
        ("indigo selection", roles().ink, Palette::Indigo.accent()),
        ("value and button labels", roles().legend, roles().face),
        ("help text", roles().help, roles().face),
        (
            "disabled labels",
            roles().disabled_text,
            roles().disabled_face,
        ),
    ] {
        let ratio = contrast(text, background);
        assert!(
            ratio >= 4.5,
            "{theme} {role} contrast {ratio:.2}:1 must be at least 4.5:1"
        );
    }
}

#[test]
fn focus_rings_stay_visible_against_every_chassis() {
    for theme in theme::Theme::ALL {
        theme::paint_as(theme);
        for palette in [
            Palette::Utility,
            Palette::Marine,
            Palette::Granite,
            Palette::Red,
            Palette::Olive,
            Palette::Indigo,
        ] {
            let ratio = contrast(palette.focus(), palette.body());
            assert!(
                ratio >= 3.0,
                "{} {palette:?} focus ring contrast {ratio:.2}:1 must be at least 3:1",
                theme.label()
            );
        }
    }
    // Vintage's red enamel is too close to its accent, so its ring is cream.
    theme::paint_as(theme::Theme::Vintage);
    assert_eq!(Palette::Red.focus(), roles().legend);
    assert_eq!(Palette::Marine.focus(), Palette::Marine.accent());
    theme::paint_as(theme::Theme::Studio);
}

fn spec() -> RotarySpec<'static> {
    RotarySpec {
        name: "Test dial",
        low: 0.0,
        high: 2.0,
        reset: 1.0,
        step: 0.01,
        decimals: 3,
        logarithmic: false,
        choices: &[],
    }
}

#[test]
fn absolute_buttons_do_not_advance_parent_layout() {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (bounds, _) = ui.allocate_exact_size(Vec2::new(400.0, 44.0), Sense::hover());
            let cursor = ui.cursor();
            let occupied = ui.min_rect();
            for index in 0..3 {
                let rect = Rect::from_min_size(
                    bounds.min + Vec2::new(index as f32 * 34.0, 4.0),
                    Vec2::splat(28.0),
                );
                let response = button_at(
                    ui,
                    rect,
                    &format!("header-{index}"),
                    "?",
                    ButtonRole::Secondary,
                    index != 1,
                );
                assert_eq!(response.enabled(), index != 1);
            }
            assert_eq!(ui.cursor(), cursor);
            assert_eq!(ui.min_rect(), occupied);
        });
    });
}

#[test]
fn rotary_steps_boundaries_fine_and_discrete() {
    let linear = spec();
    assert_eq!(linear.stepped(2.0, 1.0, false), 2.0);
    assert_eq!(linear.stepped(0.0, -1.0, false), 0.0);
    assert_eq!(linear.stepped(1.0, 0.0, false), 1.0);
    assert_eq!(linear.stepped(1.0, 1.0, true), 1.001);
    let discrete = RotarySpec {
        low: 120.0,
        high: 180.0,
        choices: &[120.0, 140.0, 160.0, 180.0],
        ..linear
    };
    assert_eq!(discrete.bounded(150.0), 140.0);
    assert_eq!(discrete.stepped(140.0, 1.0, true), 160.0);
    assert_eq!(discrete.stepped(180.0, 1.0, false), 180.0);
    let logarithmic = RotarySpec {
        low: 20.0,
        high: 20_000.0,
        decimals: 0,
        step: 1.0,
        logarithmic: true,
        ..linear
    };
    assert_eq!(logarithmic.stepped(2000.0, 1.0, false), 2143.0);
}

#[test]
fn supported_materials_decode_and_fade_to_solid_chassis() {
    for (index, density) in [1.0_f32, 1.5, 2.0].into_iter().enumerate() {
        let corner = image::load_from_memory(material!("utility", "corner-tl")[index])
            .unwrap()
            .into_rgba8();
        assert_eq!(
            corner.dimensions(),
            ((112.0 * density) as u32, (64.0 * density) as u32)
        );
        assert!(corner.pixels().any(|pixel| pixel[3] > 0));
        for x in 0..corner.width() {
            assert_eq!(corner.get_pixel(x, corner.height() - 1)[3], 0);
        }
        for y in 0..corner.height() {
            assert_eq!(corner.get_pixel(corner.width() - 1, y)[3], 0);
        }
        let knob = image::load_from_memory(variants!("knob")[index]).unwrap();
        assert_eq!(
            (knob.width(), knob.height()),
            ((36.0 * density) as u32, (36.0 * density) as u32)
        );
        let rgba = knob.to_rgba8();
        assert!(
            rgba.pixels()
                .filter(|pixel| pixel[3] > 0 && pixel[3] < 255)
                .count()
                > knob.width() as usize,
            "The native-size dial silhouette must retain antialiased edge coverage at {density}x."
        );
    }
}

#[test]
fn material_slices_have_no_outer_fringe_or_strip_endpoint_seams() {
    for palette in [
        parts!("utility"),
        parts!("marine"),
        parts!("granite"),
        parts!("red"),
        parts!("olive"),
        parts!("indigo"),
    ] {
        for (part, variants) in palette.into_iter().enumerate() {
            for (density, bytes) in variants.into_iter().enumerate() {
                let image = image::load_from_memory(bytes).unwrap().into_rgba8();
                let (width, height) = image.dimensions();
                assert!(image.pixels().any(|pixel| pixel[3] > 0));
                for x in 0..width {
                    assert_eq!(image.get_pixel(x, 0)[3], 0);
                    assert_eq!(image.get_pixel(x, height - 1)[3], 0);
                }
                for y in 0..height {
                    assert_eq!(image.get_pixel(0, y)[3], 0);
                    assert_eq!(image.get_pixel(width - 1, y)[3], 0);
                }
                let margin = [4, 6, 8][density];
                for y in 0..height {
                    for x in 0..width {
                        let side = match part {
                            0 | 2 | 6 => x < margin,
                            1 | 3 | 7 => x >= width - margin,
                            _ => false,
                        };
                        let top_bottom = match part {
                            0 | 1 | 4 => y < margin,
                            2 | 3 | 5 => y >= height - margin,
                            _ => false,
                        };
                        if side || top_bottom {
                            assert_eq!(image.get_pixel(x, y)[3], 0);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn paint_grain_preserves_palette_mean_and_monochrome_gaussian_statistics() {
    // The grain artwork belongs to Vintage's painted metal.
    theme::paint_as(theme::Theme::Vintage);
    for (palette, variants) in [
        (Palette::Utility, material!("utility", "paint-grain")),
        (Palette::Marine, material!("marine", "paint-grain")),
        (Palette::Granite, material!("granite", "paint-grain")),
        (Palette::Red, material!("red", "paint-grain")),
        (Palette::Olive, material!("olive", "paint-grain")),
        (Palette::Indigo, material!("indigo", "paint-grain")),
    ] {
        let base = palette.body();
        for (index, bytes) in variants.into_iter().enumerate() {
            let image = image::load_from_memory(bytes).unwrap().into_rgba8();
            let size = [256, 384, 512][index];
            assert_eq!(image.dimensions(), (size, size));
            let mut sum = 0.0_f64;
            let mut squares = 0.0_f64;
            let mut fourth = 0.0_f64;
            for pixel in image.pixels() {
                let delta = i16::from(pixel[0]) - i16::from(base.r());
                assert_eq!(delta, i16::from(pixel[1]) - i16::from(base.g()));
                assert_eq!(delta, i16::from(pixel[2]) - i16::from(base.b()));
                assert_eq!(pixel[3], 255);
                assert!(delta.abs() <= 8, "Rare grain tails must remain subtle.");
                let delta = f64::from(delta);
                sum += delta;
                squares += delta * delta;
                fourth += delta.powi(4);
            }
            let count = f64::from(size * size);
            assert_eq!(sum, 0.0, "Grain must not shift a palette's mean color.");
            let variance = squares / count;
            assert!(variance.is_finite() && (1.2..=1.8).contains(&variance.sqrt()));
            let kurtosis = (fourth / count) / (variance * variance);
            assert!(
                (2.7..=3.3).contains(&kurtosis),
                "Grain must retain a Gaussian distribution, rather than two-level dithering."
            );
        }
    }
}

#[test]
fn disabled_rotary_rejects_accessible_edits() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut value = 1.0;
    let mut response_id = egui::Id::NULL;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(false, |ui| {
                let response = rotary(
                    ui,
                    Rect::from_min_size(Pos2::new(30.0, 30.0), Vec2::splat(48.0)),
                    &mut value,
                    spec(),
                );
                assert!(!response.enabled());
                response_id = response.id;
            });
        });
    });
    let input = egui::RawInput {
        events: vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target: response_id.value().into(),
                data: Some(egui::accesskit::ActionData::NumericValue(2.0)),
            },
        )],
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(false, |ui| {
                rotary(
                    ui,
                    Rect::from_min_size(Pos2::new(30.0, 30.0), Vec2::splat(48.0)),
                    &mut value,
                    spec(),
                );
            });
        });
    });
    assert_eq!(value, 1.0);
}
