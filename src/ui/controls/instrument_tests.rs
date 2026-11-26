use super::super::super::{fx, theme};
use super::super::contrast;
use super::roles;
use mixjam_techno::application::options::Theme;

#[test]
fn transfer_scope_reference_matches_an_identity_curve() {
    use super::{Color32, Pos2, Rect, Vec2, egui, scope_frame};

    let ctx = egui::Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(240.0, 160.0));
            scope_frame(ui, rect, "Transfer curve", &[-1.0, 1.0], Color32::GRAY);
        });
    });
    let trace = output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Path(path) if path.points.len() == 2 => Some(path.points.as_slice()),
        _ => None,
    });
    let trace = trace.expect("The transfer scope draws a trace.");
    assert!(output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::LineSegment { points, .. } if points.as_slice() == trace
    )));
}

#[test]
fn instrument_palette_meets_contrast_targets_across_all_three_themes() {
    for theme_kind in [Theme::Studio, Theme::Aero, Theme::Vintage] {
        theme::paint_as(theme_kind);
        let r = roles();
        let panel_text = contrast(r.panel, r.text);
        assert!(
            panel_text >= 5.0,
            "{theme_kind:?} panel/text contrast {panel_text:.2}:1 < 5.0:1"
        );
        let face_legend = contrast(r.face, r.legend);
        assert!(
            face_legend >= 5.0,
            "{theme_kind:?} face/legend contrast {face_legend:.2}:1 < 5.0:1"
        );
        let meter_ink = contrast(r.meter_face, r.ink);
        assert!(
            meter_ink >= 5.0,
            "{theme_kind:?} meter_face/ink contrast {meter_ink:.2}:1 < 5.0:1"
        );
        let primary_btn = contrast(r.primary, r.on_light);
        assert!(
            primary_btn >= 4.5,
            "{theme_kind:?} primary button contrast {primary_btn:.2}:1 < 4.5:1"
        );
        let danger_btn = contrast(r.danger, r.legend);
        assert!(
            danger_btn >= 4.5,
            "{theme_kind:?} danger button contrast {danger_btn:.2}:1 < 4.5:1"
        );
        let modules = [
            mixjam_techno::application::fx::Module::Empty,
            mixjam_techno::application::fx::Module::EchoformDelay(Default::default()),
            mixjam_techno::application::fx::Module::AetherformReverb(Default::default()),
            mixjam_techno::application::fx::Module::EmberformDrive(Default::default()),
        ];
        for module in &modules {
            let (motif, _) = fx::fx_motif(module);
            for on in [true, false] {
                let ([zone_a, zone_b], _) = motif.zones(on);
                for zone in [zone_a, zone_b] {
                    let ratio = contrast(zone, r.legend);
                    assert!(
                        ratio >= 5.0,
                        "{theme_kind:?} fx motif (on={on}) contrast {ratio:.2}:1 < 5.0:1"
                    );
                }
            }
        }
    }
    theme::paint_as(Theme::Studio);
}

#[test]
fn instrument_controls_render_telemetry_scope_and_all_materials() {
    use super::{
        ScopeState, bay, delay_scope_frame_state, eq_response_scope, gr_history_scope, meter_bar,
        multiband_gr_scope_state, reverb_scope_frame_state, scope_frame, stereo_vectorscope,
    };
    use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
    use mixjam_techno::application::FxBusMeterReading;
    use mixjam_techno::application::Session;
    use mixjam_techno::application::fx::Aetherform;

    let profile = Session::fx_reverb_profile(&Aetherform::default());
    let points: Vec<f32> = (0..65).map(|i| (i as f32 / 32.0) - 1.0).collect();
    let eq_curve: Vec<f32> = (0..65).map(|i| -3.0 + (i as f32 / 64.0) * 5.0).collect();
    let stereo_pts = [(0.4, 0.2), (-0.3, 0.1), (0.6, 0.5), (-0.4, -0.4)];
    let readings = [
        (
            "Hot meter",
            FxBusMeterReading {
                peak_l: 0.25,
                rms_l: 0.1,
                peak_r: 0.98,
                rms_r: 0.5,
                ducker_gr_db: 6.0,
                limiter_gr_db: 3.5,
                tail_active: true,
            },
            &[
                "dBFS",
                "Ducker reduction 6.0 dB",
                "Limiter reduction 3.5 dB",
                "Tail active",
            ][..],
        ),
        (
            "Active tail meter",
            FxBusMeterReading {
                peak_l: 0.1,
                rms_l: 0.05,
                peak_r: 0.1,
                rms_r: 0.05,
                ducker_gr_db: 0.0,
                limiter_gr_db: 0.0,
                tail_active: true,
            },
            &["Tail active"][..],
        ),
        (
            "Idle meter",
            FxBusMeterReading::default(),
            &["Left peak silent, RMS silent", "Tail idle"][..],
        ),
    ];

    for theme_kind in [Theme::Studio, Theme::Aero, Theme::Vintage] {
        theme::paint_as(theme_kind);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let meter_rect = Rect::from_min_size(Pos2::new(20.0, 260.0), Vec2::new(180.0, 24.0));
        let delay_rect = Rect::from_min_size(Pos2::new(320.0, 330.0), Vec2::new(240.0, 140.0));
        let reverb_rect = Rect::from_min_size(Pos2::new(20.0, 350.0), Vec2::new(246.0, 120.0));
        for pass in 0..2 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 620.0))),
                events: if pass == 1 {
                    vec![egui::Event::PointerMoved(meter_rect.center())]
                } else {
                    vec![]
                },
                ..Default::default()
            };
            let output = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let accent = Color32::from_rgb(234, 164, 120);
                    bay(
                        ui,
                        Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(280.0, 110.0)),
                        "Drive & Curve",
                        accent,
                        true,
                    );
                    bay(
                        ui,
                        Rect::from_min_size(Pos2::new(20.0, 140.0), Vec2::new(280.0, 110.0)),
                        "Tone",
                        accent,
                        false,
                    );
                    for (idx, (name, reading, _)) in readings.iter().enumerate() {
                        let y = 260.0 + idx as f32 * 30.0;
                        meter_bar(
                            ui,
                            Rect::from_min_size(Pos2::new(20.0, y), Vec2::new(180.0, 24.0)),
                            name,
                            *reading,
                            idx == 0,
                        );
                    }

                    scope_frame(
                        ui,
                        Rect::from_min_size(Pos2::new(320.0, 50.0), Vec2::new(240.0, 160.0)),
                        "Transfer Curve",
                        &points,
                        accent,
                    );
                    scope_frame(
                        ui,
                        Rect::from_min_size(Pos2::new(320.0, 220.0), Vec2::new(240.0, 100.0)),
                        "Empty Scope",
                        &[],
                        accent,
                    );
                    delay_scope_frame_state(
                        ui,
                        delay_rect,
                        "Tap & Decay",
                        &[(0.15, 1.0, -0.25)],
                        true,
                        ScopeState::active(accent),
                    );
                    reverb_scope_frame_state(
                        ui,
                        reverb_rect,
                        "Decay & Space",
                        "HALL · BLOOM +12",
                        &profile,
                        ScopeState::active(accent),
                    );
                    gr_history_scope(
                        ui,
                        Rect::from_min_size(Pos2::new(20.0, 480.0), Vec2::new(240.0, 90.0)),
                        "Bus compressor GR history",
                        &[0.0, 1.5, 2.4, 1.2],
                        12.0,
                        1.7,
                        accent,
                    );
                    multiband_gr_scope_state(
                        ui,
                        Rect::from_min_size(Pos2::new(280.0, 480.0), Vec2::new(240.0, 90.0)),
                        "Multiband 3-band GR",
                        [2.1, 1.4, 0.9],
                        12.0,
                        ScopeState::active(accent),
                    );
                    eq_response_scope(
                        ui,
                        Rect::from_min_size(Pos2::new(540.0, 40.0), Vec2::new(160.0, 110.0)),
                        "EQ Response",
                        &eq_curve,
                        6.0,
                        true,
                        accent,
                    );
                    stereo_vectorscope(
                        ui,
                        Rect::from_min_size(Pos2::new(540.0, 170.0), Vec2::new(160.0, 120.0)),
                        "Stereo Scope",
                        &stereo_pts,
                        0.72,
                        120.0,
                        accent,
                    );
                });
            });
            if pass == 1 {
                let update = output
                    .platform_output
                    .accesskit_update
                    .expect("accesskit update");
                for (name, _, expected_parts) in readings {
                    let value = update
                        .nodes
                        .iter()
                        .find_map(|(_, node)| {
                            (node.label() == Some(name))
                                .then(|| node.value().map(str::to_owned))
                                .flatten()
                        })
                        .unwrap_or_else(|| panic!("{theme_kind:?}: missing {name}"));
                    for part in expected_parts {
                        assert!(
                            value.contains(part),
                            "{theme_kind:?} {name}: {value:?} missing {part:?}"
                        );
                    }
                }
                for (scope_name, expected) in [
                    ("EQ Response", "-3.0 to +2.0 dB"),
                    ("Stereo Scope", "Correlation +0.72, Mono below 120 Hz"),
                ] {
                    let value = update
                        .nodes
                        .iter()
                        .find_map(|(_, node)| {
                            (node.label() == Some(scope_name))
                                .then(|| node.value().map(str::to_owned))
                                .flatten()
                        })
                        .unwrap_or_else(|| panic!("{theme_kind:?}: missing {scope_name}"));
                    assert_eq!(value, expected);
                }
                assert!(
                    output
                        .shapes
                        .iter()
                        .any(|s| matches!(s.shape, egui::Shape::Path(_))),
                    "{theme_kind:?}: scope_frame must emit polyline trace for 65 samples"
                );
                assert!(
                    output.shapes.iter().any(|shape| matches!(
                        &shape.shape,
                        egui::Shape::Circle(tip)
                            if delay_rect.contains(tip.center)
                                && tip.center.y > delay_rect.center().y
                    )),
                    "{theme_kind:?}: the inverted right tap must appear below the center line"
                );
            }
        }
    }
    theme::paint_as(Theme::Studio);
}
