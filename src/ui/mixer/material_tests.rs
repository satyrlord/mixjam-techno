use super::*;

/// Each channel of a tile as levels, in rows from the top.
fn channels(image: &egui::ColorImage) -> [Vec<f32>; 3] {
    std::array::from_fn(|channel| {
        image
            .pixels
            .iter()
            .map(|texel| f32::from(texel.to_array()[channel]))
            .collect()
    })
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

/// The mean absolute step between each texel and the one after it, the
/// first on its row (or column) after the last.
fn wrapped_step(values: &[f32], across: bool) -> (f32, f32) {
    let [width, height] = TILE;
    let at = |x: usize, y: usize| values[y * width + x];
    let (mut inside, mut seam) = (Vec::new(), Vec::new());
    for y in 0..height {
        for x in 0..width {
            let (next, wraps) = if across {
                (at((x + 1) % width, y), x + 1 == width)
            } else {
                (at(x, (y + 1) % height), y + 1 == height)
            };
            let step = (at(x, y) - next).abs();
            if wraps { &mut seam } else { &mut inside }.push(step);
        }
    }
    (mean(&inside), mean(&seam))
}

#[test]
fn the_material_tiles_are_the_same_on_every_run() {
    for stock in Stock::ALL {
        assert_eq!(tile_image(stock).pixels, tile_image(stock).pixels);
    }
    assert_ne!(
        tile_image(Stock::Ivory).pixels,
        tile_image(Stock::Graphite).pixels
    );
}

#[test]
fn the_material_tiles_keep_their_color_and_a_fine_grain() {
    for stock in Stock::ALL {
        let (recipe, _) = stock.recipe();
        for (channel, values) in channels(&tile_image(stock)).iter().enumerate() {
            let average = mean(values);
            let deviation = (values
                .iter()
                .map(|value| (value - average).powi(2))
                .sum::<f32>()
                / values.len() as f32)
                .sqrt();
            assert!(
                (average - recipe[channel]).abs() < 0.25,
                "{stock:?} channel {channel} mean {average:.2}"
            );
            // The prepared tiles had a deviation of about one level: grain
            // that shows on a close look and never reads as a pattern.
            assert!(
                (0.8..1.4).contains(&deviation),
                "{stock:?} channel {channel} deviation {deviation:.2}"
            );
            assert!(values.iter().all(|value| (value - average).abs() <= 7.0));
        }
    }
}

#[test]
fn the_material_tiles_repeat_without_a_seam() {
    for stock in Stock::ALL {
        for values in channels(&tile_image(stock)) {
            for across in [true, false] {
                let (inside, seam) = wrapped_step(&values, across);
                assert!(
                    (seam - inside).abs() < 0.15 * inside,
                    "{stock:?} seam step {seam:.3} against {inside:.3} inside"
                );
            }
        }
    }
}

#[test]
fn strip_text_keeps_its_contrast_on_every_ivory_texel() {
    let vintage = theme::of(theme::Theme::Vintage);
    let worst = tile_image(Stock::Ivory)
        .pixels
        .iter()
        .map(|&texel| controls::contrast(texel, vintage.strip_text))
        .fold(f32::INFINITY, f32::min);
    // The darkest texel of the prepared ivory tile gave 8.5:1.
    assert!(worst >= 8.4, "strip text gives {worst:.2}:1 on ivory");

    let worst_return = tile_image(Stock::Graphite)
        .pixels
        .iter()
        .map(|&texel| controls::contrast(texel, vintage.legend))
        .fold(f32::INFINITY, f32::min);
    assert!(
        worst_return >= 10.0,
        "return legend gives {worst_return:.2}:1 on graphite"
    );
}

#[test]
fn tiling_skips_empty_or_clipped_rects_and_aligns_visible_uvs() {
    let ctx = egui::Context::default();
    let clip = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        let painter = ctx
            .layer_painter(egui::LayerId::background())
            .with_clip_rect(clip);
        for skipped in [
            Rect::NOTHING,
            Rect::EVERYTHING,
            Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(0.0, 20.0)),
            Rect::from_min_size(Pos2::new(100.0, 0.0), Vec2::new(20.0, 20.0)),
            Rect::from_min_size(Pos2::new(150.0, 150.0), Vec2::new(40.0, 40.0)),
        ] {
            tile(&painter, skipped, Stock::Ivory);
        }
    });
    assert!(
        output.shapes.is_empty(),
        "empty, non-finite, and clipped rects must emit no tile mesh"
    );

    let visible = Rect::from_min_size(Pos2::new(10.4, 20.6), Vec2::new(50.0, 40.0));
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        let painter = ctx
            .layer_painter(egui::LayerId::background())
            .with_clip_rect(clip);
        tile(&painter, visible, Stock::Graphite);
    });
    let meshes: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh),
            _ => None,
        })
        .collect();
    assert_eq!(meshes.len(), 1);
    let mesh = meshes[0];
    assert!(
        mesh.vertices
            .iter()
            .all(|v| v.pos.is_finite() && v.uv.is_finite())
    );
    // At ppp = 1.0, origin snaps to (10.0, 21.0), so one texel covers one physical pixel.
    let uvs: Vec<Pos2> = mesh.vertices.iter().map(|v| v.uv).collect();
    let uv = Rect::from_points(&uvs);
    assert!((uv.min.x - 0.4 / TILE[0] as f32).abs() < 1e-6);
    assert!((uv.min.y - (-0.4) / TILE[1] as f32).abs() < 1e-6);
    assert!((uv.width() - visible.width() / TILE[0] as f32).abs() < 1e-6);
    assert!((uv.height() - visible.height() / TILE[1] as f32).abs() < 1e-6);
}

#[test]
fn grain_fields_stay_finite_for_empty_or_zero_recipes() {
    assert!(grain::field([0, 4], 1, |random, _| random.normal()).is_empty());
    assert!(grain::soft_field([0, 0], 1).is_empty());
    let flat = grain::field([4, 4], 1, |_, _| 0.0);
    assert_eq!(flat, vec![0.0; 16]);
}

#[test]
fn the_fader_cap_stays_inside_its_block_with_a_legible_stripe() {
    for pixel in [1.0, 0.5] {
        let min = Pos2::new(10.0, 20.0);
        let bounds =
            Rect::from_min_size(min, Vec2::new(f32::from(CAP[0]), f32::from(CAP[1])) * pixel);
        let drawn = fader_cap_mesh(min, pixel).calc_bounds();
        assert!(
            (drawn.min - bounds.min).length() < 1e-3 && (drawn.max - bounds.max).length() < 1e-3,
            "the cap covers {drawn:?}, not {bounds:?}"
        );
    }
    let stripe = cap_row(STRIPE + 1);
    for row in [STRIPE - 1, STRIPE + 3] {
        assert!(controls::contrast(stripe, cap_row(row)) >= 7.0);
    }
    // Every ridge's lit line stands out from the face beside it.
    for ridge in RIDGES {
        assert!(cap_row(ridge).r() > cap_row(ridge - 1).r() + 0x18);
    }
}
