use super::*;

#[test]
fn ink_stays_legible_on_every_enamel_lit_or_off() {
    for theme in theme::Theme::ALL {
        theme::paint_as(theme);
        for kind in PedalKind::ALL {
            for on in [true, false] {
                // A metal pedal that is on is checked where its finish is
                // darkest, a plastic one where its shading is.
                let darkest = match roles().material {
                    theme::Material::Metal if on => lit(enamel(kind, on), -(GRAIN_DIP + SHEEN_DIP)),
                    theme::Material::Plastic => {
                        enamel(kind, on).lerp_to_gamma(Color32::BLACK, theme::PLASTIC_SHADE)
                    }
                    _ => enamel(kind, on),
                };
                let ratio = controls::contrast(darkest, roles().ink);
                assert!(
                    ratio >= 5.0,
                    "{} {} ({on}) gives {ratio:.2}:1",
                    theme.label(),
                    name(kind)
                );
            }
        }
    }
    theme::paint_as(theme::Theme::Studio);
}

#[test]
fn the_metal_finish_is_brushed_along_the_rows_and_keeps_within_its_dip() {
    let [width, height] = GRAIN;
    let amounts: Vec<f32> = grain_image()
        .pixels
        .iter()
        .map(|texel| (f32::from(texel.r()) - f32::from(texel.a())) / 255.0)
        .collect();
    assert!(
        amounts
            .iter()
            .all(|amount| amount.abs() <= GRAIN_DIP + 0.5 / 255.0)
    );
    let variance = |means: &[f32]| {
        let mean = means.iter().sum::<f32>() / means.len() as f32;
        means
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f32>()
            / means.len() as f32
    };
    let rows: Vec<f32> = amounts
        .chunks(width)
        .map(|row| row.iter().sum::<f32>() / width as f32)
        .collect();
    let columns: Vec<f32> = (0..width)
        .map(|x| (0..height).map(|y| amounts[y * width + x]).sum::<f32>() / height as f32)
        .collect();
    assert!(variance(&rows) > 20.0 * variance(&columns));
    assert!((0..=100).all(|step| sheen(step as f32 / 100.0) >= -SHEEN_DIP));
}

#[test]
fn values_read_in_their_units_and_typed_units_are_optional() {
    assert_eq!(Format::db(true).show(1.0), "+1.0 dB");
    assert_eq!(Format::db(false).show(-0.0), "0.0 dB");
    assert_eq!(Format::whole("%").show(105.0), "105%");
    assert_eq!(Format::tenths(":1").show(2.0), "2.0:1");
    assert_eq!(Format::tenths("").show(2.5), "2.5");
    assert_eq!(Format::Speed.show(30.0), "30 ips");
    assert_eq!(Format::Band(4).show(50.0), "+0.0");
    assert_eq!(Format::db(false).read("-3 dB", 0.0), Some(-3.0));
    assert_eq!(Format::db(false).read("-1.5db", 0.0), Some(-1.5));
    assert_eq!(Format::whole("Hz").read(" 120 ", 300.0), Some(120.0));
    assert_eq!(Format::Band(4).read("+18", 100.0), Some(100.0));
    assert_eq!(Format::whole("ms").read("fast", 1200.0), None);
    assert_eq!(Format::Hertz.show(80.0), "80 Hz");
    assert_eq!(Format::Hertz.show(8000.0), "8.00 kHz");
    assert_eq!(Format::Hertz.show(20_000.0), "20.0 kHz");
    assert_eq!(Format::Hertz.read("12k", 20_000.0), Some(12_000.0));
    assert_eq!(Format::Hertz.read("2.5 kHz", 20_000.0), Some(2500.0));
    assert_eq!(Format::Hertz.read("180 hz", 20_000.0), Some(180.0));
    assert_eq!(Format::Hertz.read("400", 20_000.0), Some(400.0));
}

#[test]
fn knobs_move_whole_steps_of_their_own_size() {
    let trim = Knob::Trim.range();
    // A tenth of a decibel stays a tenth, however far from the low end.
    let mut value = 0.0;
    for expected in [-0.1, -0.2, -0.3] {
        let next = value - 0.100_000_001_49;
        value = on_grid(next, Some(value), &trim);
        assert_eq!(value, expected);
    }
    // A fine step still moves one step; a typed value goes to the nearest.
    assert_eq!(on_grid(0.01, Some(0.0), &trim), 0.1);
    assert_eq!(on_grid(1.23, None, &trim), 1.2);
    let release = Knob::CompressorRelease.range();
    assert_eq!(on_grid(301.0, Some(300.0), &release), 310.0);
    assert_eq!(on_grid(1500.0, None, &release), 1200.0);
    assert_eq!(decimal(0.1), 0.1);
}

#[test]
fn the_vu_scale_spreads_by_voltage() {
    assert_eq!(vu_position(f32::NEG_INFINITY), 0.0);
    assert_eq!(vu_position(-20.0), 0.0);
    assert!((vu_position(0.0) - 0.685).abs() < 0.01);
    assert!((vu_position(3.0) - 1.0).abs() < 1e-6);
    assert!(vu_position(12.0) <= 1.03);
}
