//! Compact mixer presentation. Gain magnitude and polarity remain separate project edits.
//! Each track strip also sends to the four FX buses, whose return strips follow the tracks,
//! and opens its channel EQ. The Master strip can bypass the master pedalboard.
use super::view::switch_hint;
use super::*;
use mixjam_techno::application::fx::{BUSES, FxBus, FxBuses};
use mixjam_techno::application::{
    MAX_MIXER_GAIN_DB, TRACK_WIDTH, Track, amplitude_to_db, db_to_amplitude,
};

/// What the mixer changed: the tracks and master level, the FX buses, the
/// master pedalboard's bypass, the bus whose return strip asked to open it
/// on the FX returns board, and the track whose EQ button was pressed.
#[derive(Default)]
pub(super) struct Changes {
    pub tracks: bool,
    pub fx: bool,
    pub bypass: bool,
    pub open: Option<usize>,
    pub eq: Option<usize>,
}

const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
const SILENCE_DB: f32 = -96.0;
// The useful -24..+12 dB interval occupies 72% of the travel. The short
// bottom section is linear amplitude, so the rail reaches exact zero.
const TAPER: [(f32, f32); 6] = [
    (0.08, -60.0),
    (0.16, -36.0),
    (0.28, -24.0),
    (0.46, -12.0),
    (0.70, 0.0),
    (1.0, 12.0),
];

pub(super) fn gain_fraction(gain: f32) -> f32 {
    let low = db_to_amplitude(TAPER[0].1);
    if gain <= low {
        return TAPER[0].0 * gain / low;
    }
    let db = amplitude_to_db(gain);
    for pair in TAPER.windows(2) {
        let [(a, lo), (b, hi)] = [pair[0], pair[1]];
        if db <= hi {
            return egui::lerp(a..=b, (db - lo) / (hi - lo));
        }
    }
    1.0
}

pub(super) fn fraction_gain(fraction: f32) -> f32 {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= TAPER[0].0 {
        return db_to_amplitude(TAPER[0].1) * fraction / TAPER[0].0;
    }
    for pair in TAPER.windows(2) {
        let [(a, lo), (b, hi)] = [pair[0], pair[1]];
        if fraction <= b {
            return db_to_amplitude(egui::lerp(lo..=hi, (fraction - a) / (b - a)));
        }
    }
    db_to_amplitude(MAX_MIXER_GAIN_DB)
}

fn db_text(db: f32) -> String {
    if db == f32::NEG_INFINITY {
        "-inf dB".into()
    } else if db.abs() < 0.05 {
        "0.0 dB".into()
    } else {
        format!("{db:+.1} dB")
    }
}

/// The gain field's text while it is being edited: one decimal, like `db_text`, without the unit.
fn db_edit_text(db: f32) -> String {
    if db == f32::NEG_INFINITY {
        "-inf".into()
    } else if db.abs() < 0.05 {
        "0.0".into()
    } else {
        format!("{db:.1}")
    }
}

fn parse_db(text: &str) -> Option<f64> {
    let value = text.trim().to_lowercase();
    let value = value.trim_end_matches("db").trim();
    match value {
        "-inf" | "-infinity" | "-∞" | "silence" => Some(f64::NEG_INFINITY),
        _ => value.parse::<f64>().ok().filter(|value| value.is_finite()),
    }
}

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../assets/mixer/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/mixer/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/mixer/200/", $name, ".png")).as_slice(),
        ]
    };
}

struct Artwork {
    density: usize,
    textures: Vec<egui::TextureHandle>,
}

impl Artwork {
    fn load(ctx: &egui::Context) -> Result<Arc<Self>, String> {
        let density = [1.0_f32, 1.5, 2.0]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - ctx.pixels_per_point())
                    .abs()
                    .total_cmp(&(**b - ctx.pixels_per_point()).abs())
            })
            .map_or(0, |(index, _)| index);
        let id = egui::Id::new(("mixer-artwork", density));
        if let Some(art) = ctx.data(|data| data.get_temp::<Arc<Self>>(id)) {
            return Ok(art);
        }
        let textures = [pngs!("ivory"), pngs!("graphite"), pngs!("fader")]
            .iter()
            .enumerate()
            .map(|(index, variants)| {
                let image = image::load_from_memory(variants[density])
                    .map_err(|error| format!("Cannot load mixer artwork: {error}"))?
                    .into_rgba8();
                Ok(ctx.load_texture(
                    format!("mixer-{density}-{index}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ),
                    egui::TextureOptions::NEAREST,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let art = Arc::new(Self { density, textures });
        ctx.data_mut(|data| data.insert_temp(id, art.clone()));
        Ok(art)
    }

    fn tile(&self, ui: &egui::Ui, index: usize, rect: Rect) {
        let ppp = ui.ctx().pixels_per_point();
        let size = self.textures[index].size_vec2() / ppp;
        let min = (rect.min.to_vec2() * ppp).round() / ppp;
        let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
        let mut y = min.y;
        while y < rect.bottom() {
            let mut x = min.x;
            while x < rect.right() {
                painter.image(
                    self.textures[index].id(),
                    Rect::from_min_size(Pos2::new(x, y), size),
                    UV,
                    Color32::WHITE,
                );
                x += size.x;
            }
            y += size.y;
        }
    }

    fn fader(&self, ui: &egui::Ui, center: Pos2) {
        let ppp = ui.ctx().pixels_per_point();
        let size = self.textures[2].size_vec2() / ppp;
        let min = ((center - size / 2.0).to_vec2() * ppp).round() / ppp;
        ui.painter().image(
            self.textures[2].id(),
            Rect::from_min_size(min.to_pos2(), size),
            UV,
            Color32::WHITE,
        );
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    project: &Project,
    library: LibraryView<'_>,
    tracks: &mut [Track],
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
    let any_solo = project.any_solo();
    let mut changed = false;
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
                                                mixer_sample_summary(project, library, index);
                                            let track = &mut tracks[index];
                                            let eq_on = processing_on(track);
                                            let mut open_eq = false;
                                            changed |= strip(
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
            changed |= strip(
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
    result.tracks = changed;
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

/// A small knob for a send amount from 0 to 1: drag, the wheel or arrow keys
/// (Shift for fine steps), Home and End, or a double-click back to 0. An `automated`
/// knob is ringed: an automation lane sets its amount while the song plays.
pub(super) fn send_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    value: &mut f32,
    automated: bool,
) -> bool {
    let response = ui.interact(rect, ui.id().with(name), Sense::click_and_drag());
    let before = *value;
    let fine = ui.input(|input| input.modifiers.shift);
    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta());
        *value += (delta.x - delta.y) / 120.0 * if fine { 0.1 } else { 1.0 };
    }
    if response.double_clicked() {
        *value = 0.0;
    }
    *value += controls::wheel(ui, &response) * if fine { 0.01 } else { 0.05 };
    if response.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let step = if fine { 0.01 } else { 0.05 };
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            if input.consume_key(modifiers, egui::Key::ArrowUp)
                || input.consume_key(modifiers, egui::Key::ArrowRight)
            {
                *value += step;
            }
            if input.consume_key(modifiers, egui::Key::ArrowDown)
                || input.consume_key(modifiers, egui::Key::ArrowLeft)
            {
                *value -= step;
            }
            if input.consume_key(modifiers, egui::Key::Home) {
                *value = 0.0;
            }
            if input.consume_key(modifiers, egui::Key::End) {
                *value = 1.0;
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(draft)) = request.data
                && draft.is_finite()
            {
                *value = draft as f32;
            }
        }
        let turns = input.num_accesskit_action_requests(response.id, Action::Increment) as f32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as f32;
        *value += turns * 0.05;
    });
    // Keep saved precision and small fine-drag movements. Rounding belongs
    // only to the displayed percentage.
    *value = (*value).clamp(0.0, 1.0);
    let center = rect.center();
    let radius = rect.width() / 2.0;
    ui.painter()
        .circle_filled(center, radius, roles().strip_face);
    ui.painter().circle_stroke(
        center,
        radius,
        Stroke::new(
            1.0 * scale,
            if response.has_focus() {
                roles().strip_lit
            } else {
                roles().strip_slot
            },
        ),
    );
    if *value > 0.0 {
        let arc: Vec<Pos2> = (0..=24)
            .map(|step| {
                let angle = (-225.0 + 270.0 * *value * step as f32 / 24.0).to_radians();
                center + Vec2::angled(angle) * (radius - 2.0 * scale)
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            arc,
            Stroke::new(2.0 * scale, roles().strip_lit),
        ));
    }
    let pointer = Vec2::angled((-225.0 + 270.0 * *value).to_radians());
    ui.painter().line_segment(
        [center, center + pointer * (radius - 3.0 * scale)],
        Stroke::new(1.5 * scale, roles().legend),
    );
    if automated {
        ui.painter().circle_stroke(
            center,
            radius + 2.0 * scale,
            Stroke::new(1.0 * scale, roles().legend),
        );
    }
    focus_ring(ui, &response, rect, scale);
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), f64::from(*value), name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        builder.set_min_numeric_value(0.0);
        builder.set_max_numeric_value(1.0);
        builder.set_numeric_value_step(0.01);
        builder.add_action(Action::SetValue);
        builder.add_action(Action::Increment);
        builder.add_action(Action::Decrement);
    });
    response.hover_hint(format!(
        "{name}: {:.0}%. Drag, or use the wheel or arrow keys; Shift for fine steps; \
         double-click for 0.{}",
        *value * 100.0,
        if automated {
            "\nAutomated: its automation lane sets it while the song plays."
        } else {
            ""
        }
    ));
    before != *value
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
pub(super) fn return_state(bus: &FxBus, automated: bool) -> String {
    if automated {
        "Return automated".to_string()
    } else {
        format!("Return {:.0}%", bus.return_level * 100.0)
    }
}

/// A bus's Input and Limiter switches, named "{name} power" and
/// "{name} limiter".
pub(super) fn bus_switches(
    ui: &mut egui::Ui,
    rects: [Rect; 2],
    scale: f32,
    name: &str,
    bus: &mut FxBus,
) {
    for (rect, label, key, on, hint) in [
        (
            rects[0],
            "Input",
            format!("{name} power"),
            &mut bus.powered,
            "Off stops new sound into the effect; its tail rings out.",
        ),
        (
            rects[1],
            "Limiter",
            format!("{name} limiter"),
            &mut bus.limiter,
            "Holds the return under -1 dBFS.",
        ),
    ] {
        let response = button(ui, rect, label, &key, scale, *on);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *on, &key)
        });
        if response
            .hover_hint(format!("{key}: {}. {hint}", if *on { "on" } else { "off" }))
            .clicked()
        {
            *on = !*on;
        }
    }
}

/// "Edit {name}": shows the bus on the FX returns board. Returns whether it
/// was pressed.
pub(super) fn edit_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str) -> bool {
    button(ui, rect, "Edit", &format!("Edit {name}"), scale, false)
        .hover_hint(format!(
            "Show {name} on the FX returns board, with all its controls."
        ))
        .clicked()
}

/// Whether a track's channel EQ, drive, compressor, or ducker is on, or its
/// width is not 100%: what lights its EQ button.
pub(super) fn processing_on(track: &Track) -> bool {
    track.eq.on
        || track.drive.on
        || track.compressor.on
        || track.width != TRACK_WIDTH.default
        || track.ducker.on
}

/// Why a track is silent, with causes on the track first, else its pan:
/// Muted, No output, Solo elsewhere, Pan automated, or the pan position.
pub(super) fn track_state(
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

/// "Master bypass": skips the master pedalboard. Returns whether it changed.
pub(super) fn bypass_button(ui: &mut egui::Ui, rect: Rect, scale: f32, bypass: &mut bool) -> bool {
    let response = button(ui, rect, "Bypass", "Master bypass", scale, *bypass);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *bypass,
            "Master bypass",
        )
    });
    let clicked = response
        .hover_hint(format!(
            "Master bypass: {}. Bypass skips the master pedalboard, trim included, for \
             playback and export, so you can master the exported mix elsewhere. The \
             pedals keep their settings. Mastering's Bypass all is the same switch.",
            if *bypass { "on" } else { "off" }
        ))
        .clicked();
    if clicked {
        *bypass = !*bypass;
    }
    clicked
}

/// "Reset {name} gain to unity". Returns whether it was pressed.
pub(super) fn unity_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str) -> bool {
    button(
        ui,
        rect,
        "Unity",
        &format!("Reset {name} gain to unity"),
        scale,
        false,
    )
    .hover_hint("Set gain to 0.0 dB (unity). Polarity stays unchanged.")
    .clicked()
}

/// "Center {name} pan". Returns whether it was pressed.
pub(super) fn center_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str) -> bool {
    button(
        ui,
        rect,
        "Center",
        &format!("Center {name} pan"),
        scale,
        false,
    )
    .hover_hint("Reset pan to center. Output routing stays unchanged.")
    .clicked()
}

/// A track's pan in `rect`, styled for the strips: "{name} pan". Returns
/// whether it changed.
pub(super) fn pan_slider(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    pan: &mut f32,
    automated: bool,
) -> bool {
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().slider_width = rect.width();
        ui.spacing_mut().interact_size.y = 16.0 * scale;
        // The rail is a dark slot on every theme's strip, and the idle
        // handle keeps a cap-colored edge on it.
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = roles().strip_slot;
        widgets.inactive.fg_stroke = Stroke::new(1.5 * scale, roles().cap);
        for state in [&mut widgets.hovered, &mut widgets.active] {
            state.bg_fill = roles().cap;
            state.fg_stroke = Stroke::new(1.5 * scale, roles().strip_slot);
        }
        let response = pan_control(ui, name, pan, automated);
        changed = response.changed();
        focus_ring(ui, &response, rect, scale);
    });
    changed
}

/// "{name} EQ", lit while `on`: opens the channel EQ window. Returns whether
/// it was pressed.
pub(super) fn eq_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str, on: bool) -> bool {
    let eq_name = format!("{name} EQ");
    let response = button(ui, rect, "EQ", &eq_name, scale, on);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, response.enabled(), on, &eq_name)
    });
    response
        .hover_hint(format!(
            "{name} channel processing: {}. Open the channel EQ window: low and high cuts, \
             shelves, and two peaks; the drive, which adds harmonics; the compressor and the \
             stereo width, before the track's automation and fader; and the ducker, which \
             lowers the track while another track sounds.",
            if on { "on" } else { "off" }
        ))
        .clicked()
}

/// "{name} polarity invert": a slashed circle, amber while inverted. Returns
/// whether it changed.
pub(super) fn polarity_button(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    inverted: &mut bool,
) -> bool {
    let response = button(
        ui,
        rect,
        "",
        &format!("{name} polarity invert"),
        scale,
        *inverted,
    );
    let center = rect.center();
    let color = if *inverted {
        roles().ink
    } else {
        roles().legend
    };
    // The strip's 6-point circle, smaller in a short row.
    let radius = (rect.height() / 2.0 - 2.0).min(6.0 * scale);
    let unit = radius / 6.0;
    ui.painter()
        .circle_stroke(center, radius, Stroke::new(1.5 * scale, color));
    ui.painter().line_segment(
        [
            center + Vec2::new(-5.0, 7.0) * unit,
            center + Vec2::new(5.0, -7.0) * unit,
        ],
        Stroke::new(1.5 * scale, color),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *inverted,
            format!("{name} polarity invert"),
        )
    });
    let clicked = response
        .hover_hint(format!(
            "{name} polarity: {}. Invert the sign of both output channels without changing \
             gain or pan. Negative dB values only reduce level.",
            if *inverted { "inverted" } else { "normal" }
        ))
        .clicked();
    if clicked {
        *inverted = !*inverted;
    }
    clicked
}

/// Paints a Mix view row in the strips' material: the ivory, or for a return
/// the graphite, tiles in Vintage, and the theme's strip colors elsewhere.
pub(super) fn row_surface(ui: &egui::Ui, rect: Rect, is_return: bool) {
    let art = (roles().material == theme::Material::Metal)
        .then(|| Artwork::load(ui.ctx()).ok())
        .flatten();
    if let Some(art) = art {
        art.tile(ui, usize::from(is_return), rect);
    } else {
        theme::surface(
            ui.painter(),
            rect,
            roles().corner(3.0),
            if is_return {
                roles().return_strip
            } else {
                roles().strip
            },
        );
    }
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, roles().strip_edge),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn text(ui: &egui::Ui, pos: Pos2, value: &str, size: f32, color: Color32) {
    ui.painter().text(
        pos,
        Align2::CENTER_CENTER,
        value,
        FontId::proportional(size),
        color,
    );
}

pub(super) fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    name: &str,
    scale: f32,
    selected: bool,
) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    let fill = if selected {
        roles().strip_lit
    } else if response.hovered() {
        roles().raised
    } else {
        roles().strip_face
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(
            1.0 * scale,
            if response.has_focus() {
                roles().strip_lit
            } else {
                roles().strip_slot
            },
        ),
        egui::StrokeKind::Inside,
    );
    focus_ring(ui, &response, rect, scale);
    text(
        ui,
        rect.center(),
        label,
        10.0 * scale,
        if selected {
            roles().ink
        } else {
            roles().legend
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    response
}

/// Amber is illegible on Vintage's ivory, so each theme names the ring color
/// that reads on its strips; amber remains the inner edge on dark faces.
fn focus_ring(ui: &egui::Ui, response: &Response, rect: Rect, scale: f32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(1.0 * scale),
            3.0,
            Stroke::new(2.0 * scale, roles().strip_focus),
            egui::StrokeKind::Outside,
        );
    }
}

pub(super) fn switch_button(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    switch: TrackSwitch,
    enabled: &mut bool,
) -> bool {
    let label = format!("{name} {}", switch.name());
    let response = button(ui, rect, "", &label, scale, false);
    ui.painter().rect_filled(
        Rect::from_center_size(
            rect.center_top() + Vec2::new(0.0, 5.0) * scale,
            Vec2::new(12.0, 2.0) * scale,
        ),
        1.0,
        if *enabled {
            roles().accent
        } else {
            roles().muted
        },
    );
    text(
        ui,
        rect.center() + Vec2::new(0.0, 3.0) * scale,
        switch.letter(),
        11.0 * scale,
        if *enabled {
            roles().accent
        } else {
            roles().muted
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *enabled,
            &label,
        )
    });
    let clicked = response
        .hover_hint(switch_hint(&label, switch, *enabled))
        .clicked();
    if clicked {
        *enabled = !*enabled;
    }
    clicked
}

pub(super) fn gain_value(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    gain: &mut f32,
) -> bool {
    ui.painter().rect_filled(rect, 3.0, roles().strip_face);
    let before = *gain;
    let db = amplitude_to_db(before);
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(("gain-value", name))
            .max_rect(rect),
        |ui| {
            ui.spacing_mut().interact_size = rect.size();
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(12.0 * scale));
            let widgets = &mut ui.style_mut().visuals.widgets;
            for widget in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                widget.bg_fill = Color32::TRANSPARENT;
                widget.weak_bg_fill = Color32::TRANSPARENT;
                widget.bg_stroke = Stroke::NONE;
                widget.fg_stroke.color = roles().legend;
            }
            // A focused value shows an edge before it is typed in.
            widgets.active.bg_stroke = Stroke::new(1.0 * scale, roles().legend);
            let id = ui.next_auto_id();
            let context = ui.ctx().clone();
            // DragValue needs a finite origin for relative edits. Keep silence's
            // text and parser independent from that origin, so passive focus and
            // explicit finite entry retain their exact meaning.
            let has_focus = context.memory(|memory| memory.has_focus(id));
            let relative_edit = ui.ctx().is_being_dragged(id)
                || ui.input(|input| {
                    use egui::accesskit::Action;
                    (input.key_pressed(egui::Key::ArrowUp)
                        || input.key_pressed(egui::Key::ArrowDown))
                        && has_focus
                        || input.num_accesskit_action_requests(id, Action::Increment) > 0
                        || input.num_accesskit_action_requests(id, Action::Decrement) > 0
                });
            // The edit text shows one decimal. Leaving the field without typing
            // commits that text, which must not round the saved amplitude.
            let typed_id = id.with("typed");
            let typed = context.data(|data| data.get_temp::<bool>(typed_id)) == Some(true)
                || has_focus
                    && ui.input(|input| {
                        input.events.iter().any(|event| {
                            matches!(
                                event,
                                egui::Event::Text(_)
                                    | egui::Event::Paste(_)
                                    | egui::Event::Key {
                                        key: egui::Key::Backspace | egui::Key::Delete,
                                        pressed: true,
                                        ..
                                    }
                            )
                        })
                    });
            let shown = parse_db(&db_edit_text(db)).map(|shown| shown as f32);
            let response = ui.add(
                egui::DragValue::from_get_set(|value| {
                    if let Some(value) = value {
                        let value = value as f32;
                        let lowering_silence =
                            before == 0.0 && relative_edit && value <= SILENCE_DB;
                        let untouched_text = !typed && !relative_edit && Some(value) == shown;
                        if !lowering_silence
                            && !untouched_text
                            && value != db
                            && (value.is_finite() || value == f32::NEG_INFINITY)
                        {
                            *gain = db_to_amplitude(value.min(MAX_MIXER_GAIN_DB));
                        }
                    }
                    f64::from(if *gain == 0.0 {
                        SILENCE_DB
                    } else {
                        amplitude_to_db(*gain)
                    })
                })
                .range(f64::NEG_INFINITY..=f64::from(amplitude_to_db(4.0)))
                .clamp_existing_to_range(false)
                .speed(0.1)
                .update_while_editing(false)
                .custom_formatter(move |value, _| {
                    let value = if before == 0.0
                        && (value as f32 == SILENCE_DB
                            || relative_edit && value as f32 <= SILENCE_DB)
                    {
                        f32::NEG_INFINITY
                    } else {
                        value as f32
                    };
                    if context.memory(|memory| memory.has_focus(id)) {
                        db_edit_text(value)
                    } else {
                        db_text(value)
                    }
                })
                .custom_parser(parse_db),
            );
            let focused = response.has_focus();
            ui.ctx().data_mut(|data| {
                if focused {
                    data.insert_temp(typed_id, typed);
                } else {
                    data.remove::<bool>(typed_id);
                }
            });
            response.widget_info(|| {
                let mut info =
                    egui::WidgetInfo::drag_value(ui.is_enabled(), f64::from(db.max(SILENCE_DB)));
                info.label = Some(format!("{name} gain value"));
                info.current_text_value = Some(db_text(db));
                info
            });
            response.hover_hint(format!(
                "{name} gain: {}. Click to enter dB, then press Enter. Negative dB attenuates. \
                 Enter -inf for silence. Polarity stays unchanged.",
                db_text(db)
            ));
        },
    );
    before != *gain
}

fn gain_rail(
    ui: &mut egui::Ui,
    art: Option<&Artwork>,
    scale: f32,
    rect: Rect,
    name: &str,
    gain: &mut f32,
) -> bool {
    let before = *gain;
    let response = ui.interact(
        // The whole cap stays draggable at either end. Position mapping
        // below still uses the physical rail and clamps beyond its ends.
        rect.expand2(Vec2::new(0.0, 16.0 * scale)),
        ui.id().with(("gain-rail", name)),
        Sense::click_and_drag(),
    );
    let slot = Rect::from_center_size(
        rect.center(),
        Vec2::new(8.0 * scale, rect.height() + 4.0 * scale),
    );
    ui.painter().rect_filled(slot, 4.0, roles().strip_slot);
    ui.painter().rect_stroke(
        slot.expand(1.0 * scale),
        4.0,
        Stroke::new(1.0 * scale, roles().strip_edge),
        egui::StrokeKind::Outside,
    );
    // A pointer edit leaves keyboard focus unchanged. Tab focuses the rail
    // for key adjustments.
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let fine = ui.input(|input| input.modifiers.shift);
        let fraction = if fine && response.dragged() {
            gain_fraction(*gain) - ui.input(|input| input.pointer.delta().y) / rect.height() * 0.1
        } else {
            (rect.bottom() - pos.y) / rect.height()
        };
        *gain = fraction_gain(fraction);
    }
    gain_keys(ui, &response, gain);
    let cap = Pos2::new(
        rect.center().x,
        egui::lerp(rect.bottom()..=rect.top(), gain_fraction(*gain)),
    );
    if let Some(art) = art {
        art.fader(ui, cap);
    } else {
        // A flat software cap on Studio, a molded one on Aero; both stay
        // inside the bitmap cap's 24 by 32 points.
        let size = if roles().material == theme::Material::Flat {
            Vec2::new(24.0, 16.0)
        } else {
            Vec2::new(24.0, 24.0)
        };
        theme::fader_cap(ui.painter(), cap, size * scale);
    }
    focus_ring(ui, &response, rect, scale);
    gain_semantics(ui, response, name, before, *gain);
    before != *gain
}

/// The edits every gain fader takes besides the pointer: the wheel, the
/// arrow keys, Home and End, the accessibility actions, and a double-click.
pub(super) fn gain_keys(ui: &egui::Ui, response: &Response, gain: &mut f32) {
    let mut step = controls::wheel(ui, response);
    if response.has_focus() {
        controls::keep_transport_key(ui.ctx(), response.id);
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            if input.consume_key(modifiers, egui::Key::ArrowUp)
                || input.consume_key(modifiers, egui::Key::ArrowRight)
            {
                step = 1.0;
            }
            if input.consume_key(modifiers, egui::Key::ArrowDown)
                || input.consume_key(modifiers, egui::Key::ArrowLeft)
            {
                step = -1.0;
            }
            if input.consume_key(modifiers, egui::Key::Home) {
                *gain = 0.0;
            }
            if input.consume_key(modifiers, egui::Key::End) {
                *gain = db_to_amplitude(MAX_MIXER_GAIN_DB);
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(value)) = request.data
                && value.is_finite()
            {
                *gain = if value <= f64::from(SILENCE_DB) {
                    0.0
                } else {
                    db_to_amplitude((value as f32).min(MAX_MIXER_GAIN_DB))
                };
            }
        }
        step += input.num_accesskit_action_requests(response.id, Action::Increment) as f32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as f32;
    });
    if step != 0.0 {
        if response.has_focus() {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        let previous_db = amplitude_to_db(*gain);
        let db = if *gain == 0.0 {
            if step > 0.0 { -60.0 } else { f32::NEG_INFINITY }
        } else {
            previous_db
                + step
                    * if ui.input(|input| input.modifiers.shift) {
                        0.1
                    } else {
                        0.5
                    }
        };
        *gain = if step < 0.0 && previous_db >= SILENCE_DB && db <= SILENCE_DB {
            0.0
        } else {
            db_to_amplitude(db.min(MAX_MIXER_GAIN_DB))
        };
    }
    if response.double_clicked() {
        *gain = 1.0;
    }
}

/// A gain fader's accessible name, value, range, and hint: "{name} gain".
pub(super) fn gain_semantics(
    ui: &egui::Ui,
    mut response: Response,
    name: &str,
    before: f32,
    gain: f32,
) -> Response {
    let db = amplitude_to_db(gain);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::slider(
            response.enabled(),
            f64::from(db.max(SILENCE_DB)),
            format!("{name} gain"),
        );
        info.current_text_value = Some(db_text(db));
        info
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::Action;
        node.set_min_numeric_value(f64::from(SILENCE_DB));
        node.set_max_numeric_value(f64::from(MAX_MIXER_GAIN_DB));
        node.set_numeric_value_step(0.5);
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    });
    if before != gain {
        response.mark_changed();
    }
    response.hover_hint(format!(
        "{name} gain: {}. Drag to adjust; Shift gives fine movement. \
         The wheel and arrow keys change 0.5 dB \
         (Shift: 0.1 dB). Home silences, End sets +12 dB. Double-click for unity. Negative dB \
         only reduces level.",
        db_text(db)
    ))
}
