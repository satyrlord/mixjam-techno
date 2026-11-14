//! The strips: the track strip, the return strip, the row that lays them out,
//! and the state text each announces.
use super::*;

pub(crate) fn show(
    ui: &mut egui::Ui,
    project: &Project,
    library: LibraryView<'_>,
    master: &mut f32,
    fx: &mut FxBuses,
    bypass: &mut bool,
) -> Changes {
    // Vintage reuses the ivory and graphite artwork; the other themes paint
    // their strips in code.
    let art = (roles().material == theme::Material::Metal).then(|| Artwork::load(ui.ctx()));
    let art = match &art {
        Some(Ok(art)) => Some(art.as_ref()),
        Some(Err(error)) => {
            ui.colored_label(roles().coral, error);
            None
        }
        None => None,
    };
    // Strips are measured in whole physical pixels at each supported density,
    // so every theme keeps the same layout.
    let ppp = ui.ctx().pixels_per_point();
    let density = [1.0_f32, 1.5, 2.0]
        .into_iter()
        .min_by(|a, b| (a - ppp).abs().total_cmp(&(b - ppp).abs()))
        .unwrap_or(1.0);
    let scale = art.map_or(density / ppp, |art| {
        [1.0, 1.5, 2.0][art.density] / ui.ctx().pixels_per_point()
    });
    let width = 108.0 * scale;
    let whole = ui.available_rect_before_wrap();
    if let Some(art) = art {
        art.tile(ui, 1, whole);
    } else {
        ui.painter().rect_filled(whole, 0.0, roles().mixer_floor);
    }
    // A 400-point panel gives the physical gain rail 96 points. A short
    // window can reduce the panel to 370 points and the rail to 68. Its
    // interactive area also includes the cap's 16-point overhang at each end.
    let height = (ui.available_height() - 14.0).max(304.0 * scale);
    let bank_width = (ui.available_width() - width - 12.0 * scale).max(width);
    let occupied = occupied_tracks(project);
    let mut summaries = mixer_sample_summaries(project, library);
    let any_solo = project.any_solo();
    let mut result = Changes::default();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(bank_width, height + 14.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(bank_width);
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                egui::ScrollArea::horizontal()
                    .id_salt("mixer_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .max_height(height + 14.0)
                    .show(ui, |ui| {
                        if occupied.is_empty() {
                            ui.add_space(22.0);
                            ui.label(heading("No tracks", 15.0));
                            ui.label("Place a sound in the arrangement.");
                        } else {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0 * scale;
                                for &index in &occupied {
                                    ui.scope_builder(
                                        egui::UiBuilder::new().id_salt(("mixer_track", index)),
                                        |ui| {
                                            let (summary, detail) =
                                                std::mem::take(&mut summaries[index]);
                                            let mut edited = project.tracks[index].clone();
                                            let track = &mut edited;
                                            let eq_on = processing_on(track);
                                            let mut open_eq = false;
                                            let changed = strip(
                                                ui,
                                                art,
                                                scale,
                                                height,
                                                (
                                                    &format!("Track {}", index + 1),
                                                    &summary,
                                                    &detail,
                                                ),
                                                &mut track.gain,
                                                Some(TrackControls {
                                                    index,
                                                    inverted: &mut track.polarity_inverted,
                                                    pan: &mut track.pan,
                                                    left: &mut track.left_enabled,
                                                    right: &mut track.right_enabled,
                                                    muted: &mut track.muted,
                                                    solo: &mut track.solo,
                                                    automated: std::array::from_fn(|bus| {
                                                        !track.automation.sends[bus].is_empty()
                                                    }),
                                                    pan_automated: !track.automation.pan.is_empty(),
                                                    sends: &mut track.sends,
                                                    any_solo,
                                                    eq_on,
                                                    open_eq: &mut open_eq,
                                                }),
                                            );
                                            if changed {
                                                result.tracks.push((index, edited));
                                            }
                                            if open_eq {
                                                result.eq = Some(index);
                                            }
                                        },
                                    );
                                }
                                // The returns follow the tracks that feed them.
                                for (index, bus) in fx.buses.iter_mut().enumerate() {
                                    ui.scope_builder(
                                        egui::UiBuilder::new().id_salt(("mixer_return", index)),
                                        |ui| {
                                            let automated =
                                                !project.return_automation[index].is_empty();
                                            let (edited, open) = return_strip(
                                                ui, art, scale, height, index, bus, automated,
                                            );
                                            result.fx |= edited;
                                            if open {
                                                result.open = Some(index);
                                            }
                                        },
                                    );
                                }
                            });
                        }
                    });
            },
        );
        ui.scope_builder(egui::UiBuilder::new().id_salt("mixer_master"), |ui| {
            result.master |= strip(
                ui,
                art,
                scale,
                height,
                (
                    "Master",
                    "Stereo output",
                    "Master level affects the complete mix.",
                ),
                master,
                None,
            );
            // Below the output labels, where a track strip has its switches.
            let rect = Rect::from_min_size(
                ui.min_rect().min + Vec2::new(14.0, height / scale - 56.0) * scale,
                Vec2::new(80.0, 24.0) * scale,
            );
            result.bypass = bypass_button(ui, rect, scale, bypass);
        });
    });
    result
}

struct TrackControls<'a> {
    index: usize,
    inverted: &'a mut bool,
    pan: &'a mut f32,
    left: &'a mut bool,
    right: &'a mut bool,
    muted: &'a mut bool,
    solo: &'a mut bool,
    sends: &'a mut [f32; BUSES],
    /// Which sends an automation lane moves instead of the knob.
    automated: [bool; BUSES],
    /// Whether the pan lane moves the pan instead of the knob.
    pan_automated: bool,
    /// Whether any track was soloed when the mixer was laid out.
    any_solo: bool,
    /// Whether the track's channel EQ, compressor, or ducker is on or its
    /// width is not 100%, and whether its EQ button was pressed.
    eq_on: bool,
    open_eq: &'a mut bool,
}

fn strip(
    ui: &mut egui::Ui,
    art: Option<&Artwork>,
    scale: f32,
    height: f32,
    identity: (&str, &str, &str),
    gain: &mut f32,
    controls: Option<TrackControls<'_>>,
) -> bool {
    let (name, summary, detail) = identity;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(108.0 * scale, height), Sense::hover());
    if let Some(art) = art {
        art.tile(ui, 0, rect);
    } else {
        theme::surface(ui.painter(), rect, roles().corner(3.0), roles().strip);
    }
    ui.painter().rect_stroke(
        rect.shrink(0.5 * scale),
        3.0,
        Stroke::new(1.0 * scale, roles().strip_edge),
        egui::StrokeKind::Inside,
    );
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    text(ui, at(54.0, 14.0), name, 13.0 * scale, roles().strip_text);
    let summary_rect = Rect::from_min_size(at(6.0, 23.0), Vec2::new(96.0, 16.0) * scale);
    ui.put(
        summary_rect,
        egui::Label::new(
            egui::RichText::new(summary)
                .size(11.0 * scale)
                .color(roles().strip_text),
        )
        .truncate(),
    )
    .hover_hint(detail);
    let value_rect = Rect::from_min_size(at(8.0, 42.0), Vec2::new(92.0, 22.0) * scale);
    let mut changed = gain_value(ui, value_rect, scale, name, gain);
    // Leave room for the cap at silence above the send knobs.
    let bottom = height / scale - 152.0;
    let rail = Rect::from_min_max(at(18.0, 84.0), at(50.0, bottom));
    changed |= gain_rail(ui, art, scale, rail, name, gain);
    let zero_y = egui::lerp(rail.bottom()..=rail.top(), gain_fraction(1.0));
    for (db, label) in [
        (12.0, "+12"),
        (-12.0, "−12"),
        (-24.0, "−24"),
        (-48.0, "−48"),
    ] {
        let y = egui::lerp(
            rail.bottom()..=rail.top(),
            gain_fraction(db_to_amplitude(db)),
        );
        ui.painter().line_segment(
            [
                Pos2::new(rail.right() + 2.0 * scale, y),
                Pos2::new(rail.right() + 6.0 * scale, y),
            ],
            Stroke::new(1.0 * scale, roles().strip_text),
        );
        // A short rail keeps the tick but drops a label that would touch -inf.
        if rail.bottom() - y >= 12.0 * scale {
            ui.painter().text(
                Pos2::new(rail.right() + 9.0 * scale, y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(10.0 * scale),
                roles().strip_text,
            );
        }
    }
    ui.painter().text(
        Pos2::new(rail.right() + 9.0 * scale, rail.bottom()),
        Align2::LEFT_CENTER,
        "-inf",
        FontId::proportional(10.0 * scale),
        roles().strip_text,
    );
    ui.painter().text(
        Pos2::new(rail.left() - 4.0 * scale, zero_y),
        Align2::RIGHT_CENTER,
        "0",
        FontId::proportional(10.0 * scale),
        roles().strip_text,
    );
    if unity_button(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 79.0 * scale, zero_y),
            Vec2::new(44.0, 16.0) * scale,
        ),
        scale,
        name,
    ) {
        changed |= *gain != 1.0;
        *gain = 1.0;
    }
    if let Some(TrackControls {
        index,
        inverted,
        pan,
        left,
        right,
        muted,
        solo,
        sends,
        automated,
        pan_automated,
        any_solo,
        eq_on,
        open_eq,
    }) = controls
    {
        // The sends to FX 1 to 4, from left to right, in the gap between the
        // rail and the pan.
        for (bus, send) in sends.iter_mut().enumerate() {
            let x = 15.0 + bus as f32 * 26.0;
            changed |= send_knob(
                ui,
                Rect::from_center_size(at(x, height / scale - 123.0), Vec2::splat(13.0 * scale)),
                scale,
                &format!("{name} send FX {}", bus + 1),
                send,
                automated[bus],
            );
        }
        let pan_rect = Rect::from_min_size(
            at(8.0, height / scale - 116.0),
            Vec2::new(92.0, 16.0) * scale,
        );
        changed |= pan_slider(
            ui,
            pan_rect,
            scale,
            &format!("Track {}", index + 1),
            pan,
            pan_automated,
        );
        if center_button(
            ui,
            Rect::from_center_size(
                at(54.0, height / scale - 90.0),
                Vec2::new(40.0, 14.0) * scale,
            ),
            scale,
            name,
        ) {
            changed |= *pan != 0.0;
            *pan = 0.0;
        }
        text(
            ui,
            at(54.0, height / scale - 72.0),
            &track_state(
                *muted,
                *left || *right,
                any_solo && !*solo,
                pan_automated,
                *pan,
            ),
            11.0 * scale,
            roles().strip_text,
        );
        // The EQ button completes the row of Mute and Solo, above polarity.
        if eq_button(
            ui,
            Rect::from_min_size(
                at(8.0, height / scale - 56.0),
                Vec2::new(26.0, 24.0) * scale,
            ),
            scale,
            name,
            eq_on,
        ) {
            *open_eq = true;
        }
        for (switch, x, on) in [
            (TrackSwitch::Mute, 40.0, muted),
            (TrackSwitch::Solo, 72.0, solo),
        ] {
            changed |= switch_button(
                ui,
                Rect::from_min_size(at(x, height / scale - 56.0), Vec2::new(28.0, 24.0) * scale),
                scale,
                name,
                switch,
                on,
            );
        }
        changed |= polarity_button(
            ui,
            Rect::from_min_size(
                at(8.0, height / scale - 28.0),
                Vec2::new(26.0, 24.0) * scale,
            ),
            scale,
            name,
            inverted,
        );
        for (switch, x, on) in [
            (TrackSwitch::Left, 40.0, left),
            (TrackSwitch::Right, 72.0, right),
        ] {
            changed |= switch_button(
                ui,
                Rect::from_min_size(at(x, height / scale - 28.0), Vec2::new(28.0, 24.0) * scale),
                scale,
                name,
                switch,
                on,
            );
        }
    } else {
        text(
            ui,
            at(54.0, height / scale - 93.0),
            "Main output",
            11.0 * scale,
            roles().strip_text,
        );
        text(
            ui,
            at(54.0, height / scale - 75.0),
            "L + R",
            11.0 * scale,
            roles().strip_text,
        );
    }
    changed
}

/// The return of one FX bus: its effect, return level, input and limiter
/// switches, and a button that opens it on the FX returns board. An
/// `automated` return has a return lane, which sets the level while the
/// song plays; its knob is ringed. Returns whether the bus changed and
/// whether Edit was pressed.
fn return_strip(
    ui: &mut egui::Ui,
    art: Option<&Artwork>,
    scale: f32,
    height: f32,
    index: usize,
    bus: &mut FxBus,
    automated: bool,
) -> (bool, bool) {
    let name = format!("FX {}", index + 1);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(108.0 * scale, height), Sense::hover());
    if let Some(art) = art {
        art.tile(ui, 1, rect);
    } else {
        theme::surface(
            ui.painter(),
            rect,
            roles().corner(3.0),
            roles().return_strip,
        );
    }
    ui.painter().rect_stroke(
        rect.shrink(0.5 * scale),
        3.0,
        Stroke::new(1.0 * scale, roles().strip_edge),
        egui::StrokeKind::Inside,
    );
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    text(ui, at(54.0, 14.0), &name, 13.0 * scale, roles().legend);
    text(
        ui,
        at(54.0, 31.0),
        bus.module.label(),
        11.0 * scale,
        roles().legend,
    );
    let before = *bus;
    let mut level = bus.return_level;
    send_knob(
        ui,
        Rect::from_center_size(at(54.0, 82.0), Vec2::splat(44.0 * scale)),
        scale,
        &format!("{name} return level"),
        &mut level,
        automated,
    );
    bus.return_level = level;
    text(
        ui,
        at(54.0, 116.0),
        &return_state(bus, automated),
        11.0 * scale,
        roles().legend,
    );
    bus_switches(
        ui,
        [
            Rect::from_min_size(at(14.0, 136.0), Vec2::new(80.0, 22.0) * scale),
            Rect::from_min_size(at(14.0, 164.0), Vec2::new(80.0, 22.0) * scale),
        ],
        scale,
        &name,
        bus,
    );
    let edit = edit_button(
        ui,
        Rect::from_min_size(
            at(14.0, height / scale - 40.0),
            Vec2::new(80.0, 24.0) * scale,
        ),
        scale,
        &name,
    );
    (*bus != before, edit)
}

/// A return's state: "Return automated" while its lane sets the level, else
/// the level.
pub(crate) fn return_state(bus: &FxBus, automated: bool) -> String {
    if automated {
        "Return automated".to_string()
    } else {
        format!("Return {:.0}%", bus.return_level * 100.0)
    }
}

/// Whether a track's channel EQ, drive, compressor, or ducker is on, or its
/// width is not 100%: what lights its EQ button.
pub(crate) fn processing_on(track: &Track) -> bool {
    track.eq.on
        || track.drive.on
        || track.compressor.on
        || track.width != TRACK_WIDTH.default
        || track.ducker.on
}

/// Why a track is silent, with causes on the track first, else its pan:
/// Muted, No output, Solo elsewhere, Pan automated, or the pan position.
pub(crate) fn track_state(
    muted: bool,
    any_output: bool,
    solo_elsewhere: bool,
    pan_automated: bool,
    pan: f32,
) -> String {
    if muted {
        "Muted".into()
    } else if !any_output {
        "No output".into()
    } else if solo_elsewhere {
        "Solo elsewhere".into()
    } else if pan_automated {
        "Pan automated".into()
    } else {
        pan_description(pan)
    }
}
