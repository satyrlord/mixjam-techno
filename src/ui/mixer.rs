//! Compact mixer presentation. Gain magnitude and polarity remain separate project edits.
//! Each track strip also sends to the four FX buses, whose return strips follow the tracks,
//! and opens its channel EQ. The Master strip can bypass the master pedalboard.
use super::view::switch_hint;
use super::*;
use mixjam_techno::application::fx::{BUSES, FxBus, FxBuses};
use mixjam_techno::application::{MAX_MIXER_GAIN_DB, Track, amplitude_to_db, db_to_amplitude};

/// What the mixer changed: the tracks and master level, the FX buses, the
/// master pedalboard's bypass, the bus whose return strip asked to open it
/// in the Effects module, and the track whose EQ button was pressed.
#[derive(Default)]
pub(super) struct Changes {
    pub tracks: bool,
    pub fx: bool,
    pub bypass: bool,
    pub open: Option<usize>,
    pub eq: Option<usize>,
}

const INK: Color32 = Color32::from_rgb(28, 29, 26);
const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const METAL: Color32 = Color32::from_rgb(201, 194, 175);
const EDGE: Color32 = Color32::from_rgb(108, 106, 97);
const FACE: Color32 = Color32::from_rgb(29, 33, 34);
const AMBER: Color32 = Color32::from_rgb(245, 193, 86);
/// The fader slot and the resting edge of dark buttons.
const SLOT: Color32 = Color32::from_rgb(11, 14, 14);
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
    let art = Artwork::load(ui.ctx());
    let art = match &art {
        Ok(art) => Some(art.as_ref()),
        Err(error) => {
            ui.colored_label(CORAL, error);
            None
        }
    };
    let scale = art.map_or(1.0, |art| {
        [1.0, 1.5, 2.0][art.density] / ui.ctx().pixels_per_point()
    });
    let width = 108.0 * scale;
    let whole = ui.available_rect_before_wrap();
    if let Some(art) = art {
        art.tile(ui, 1, whole);
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
                                        egui::UiBuilder::new().id(("mixer_track", index)),
                                        |ui| {
                                            let (summary, detail) =
                                                mixer_sample_summary(project, library, index);
                                            let track = &mut tracks[index];
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
                                                    sends: &mut track.sends,
                                                    any_solo,
                                                    eq_on: track.eq.on,
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
                                        egui::UiBuilder::new().id(("mixer_return", index)),
                                        |ui| {
                                            let (edited, open) =
                                                return_strip(ui, art, scale, height, index, bus);
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
        ui.scope_builder(egui::UiBuilder::new().id("mixer_master"), |ui| {
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
            let response = button(ui, rect, "Bypass", "Master bypass", scale, *bypass);
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Checkbox,
                    response.enabled(),
                    *bypass,
                    "Master bypass",
                )
            });
            if response
                .on_hover_text(format!(
                    "Master bypass: {}. Bypass skips the master pedalboard, trim included, for \
                     playback and export, so you can master the exported mix elsewhere. The \
                     pedals keep their settings. Mastering's Bypass all is the same switch.",
                    if *bypass { "on" } else { "off" }
                ))
                .clicked()
            {
                *bypass = !*bypass;
                result.bypass = true;
            }
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
    /// Whether any track was soloed when the mixer was laid out.
    any_solo: bool,
    /// Whether the track's channel EQ is on, and whether its EQ button was
    /// pressed.
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
        ui.painter().rect_filled(rect, 2.0, METAL);
    }
    ui.painter().rect_stroke(
        rect.shrink(0.5 * scale),
        3.0,
        Stroke::new(1.0 * scale, EDGE),
        egui::StrokeKind::Inside,
    );
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    text(ui, at(54.0, 14.0), name, 13.0 * scale, INK);
    let summary_rect = Rect::from_min_size(at(6.0, 23.0), Vec2::new(96.0, 16.0) * scale);
    ui.put(
        summary_rect,
        egui::Label::new(egui::RichText::new(summary).size(11.0 * scale).color(INK)).truncate(),
    )
    .on_hover_text(detail);
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
            Stroke::new(1.0 * scale, INK),
        );
        // A short rail keeps the tick but drops a label that would touch -inf.
        if rail.bottom() - y >= 12.0 * scale {
            ui.painter().text(
                Pos2::new(rail.right() + 9.0 * scale, y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(10.0 * scale),
                INK,
            );
        }
    }
    ui.painter().text(
        Pos2::new(rail.right() + 9.0 * scale, rail.bottom()),
        Align2::LEFT_CENTER,
        "-inf",
        FontId::proportional(10.0 * scale),
        INK,
    );
    ui.painter().text(
        Pos2::new(rail.left() - 4.0 * scale, zero_y),
        Align2::RIGHT_CENTER,
        "0",
        FontId::proportional(10.0 * scale),
        INK,
    );
    let unity = button(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 79.0 * scale, zero_y),
            Vec2::new(44.0, 16.0) * scale,
        ),
        "Unity",
        &format!("Reset {name} gain to unity"),
        scale,
        false,
    );
    if unity
        .on_hover_text("Set gain to 0.0 dB (unity). Polarity stays unchanged.")
        .clicked()
    {
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
        ui.scope_builder(egui::UiBuilder::new().max_rect(pan_rect), |ui| {
            ui.spacing_mut().slider_width = pan_rect.width();
            ui.spacing_mut().interact_size.y = 16.0 * scale;
            let response = pan_control(ui, index, pan);
            changed |= response.changed();
            focus_ring(ui, &response, pan_rect, scale);
        });
        let center = button(
            ui,
            Rect::from_center_size(
                at(54.0, height / scale - 90.0),
                Vec2::new(40.0, 14.0) * scale,
            ),
            "Center",
            &format!("Center {name} pan"),
            scale,
            false,
        );
        if center
            .on_hover_text("Reset pan to center. Output routing stays unchanged.")
            .clicked()
        {
            changed |= *pan != 0.0;
            *pan = 0.0;
        }
        // Name why the track is silent; causes on this track come first.
        text(
            ui,
            at(54.0, height / scale - 72.0),
            &if *muted {
                "Muted".into()
            } else if !*left && !*right {
                "No output".into()
            } else if any_solo && !*solo {
                "Solo elsewhere".into()
            } else {
                pan_description(*pan)
            },
            11.0 * scale,
            INK,
        );
        // The EQ button completes the row of Mute and Solo, above polarity.
        let eq_name = format!("{name} EQ");
        let response = button(
            ui,
            Rect::from_min_size(
                at(8.0, height / scale - 56.0),
                Vec2::new(26.0, 24.0) * scale,
            ),
            "EQ",
            &eq_name,
            scale,
            eq_on,
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                response.enabled(),
                eq_on,
                &eq_name,
            )
        });
        if response
            .on_hover_text(format!(
                "{name} EQ: {}. Open the channel EQ window: low and high cuts, shelves, and a \
                 peak, before the track's automation and fader.",
                if eq_on { "on" } else { "off" }
            ))
            .clicked()
        {
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
        let invert_rect = Rect::from_min_size(
            at(8.0, height / scale - 28.0),
            Vec2::new(26.0, 24.0) * scale,
        );
        let response = button(
            ui,
            invert_rect,
            "",
            &format!("{name} polarity invert"),
            scale,
            *inverted,
        );
        let center = invert_rect.center();
        let color = if *inverted { INK } else { CREAM };
        ui.painter()
            .circle_stroke(center, 6.0 * scale, Stroke::new(1.5 * scale, color));
        ui.painter().line_segment(
            [
                center + Vec2::new(-5.0, 7.0) * scale,
                center + Vec2::new(5.0, -7.0) * scale,
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
        if response
            .on_hover_text(format!(
                "{name} polarity: {}. Invert the sign of both output channels without changing \
                 gain or pan. Negative dB values only reduce level.",
                if *inverted { "inverted" } else { "normal" }
            ))
            .clicked()
        {
            *inverted = !*inverted;
            changed = true;
        }
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
            INK,
        );
        text(
            ui,
            at(54.0, height / scale - 75.0),
            "L + R",
            11.0 * scale,
            INK,
        );
    }
    changed
}

/// A small knob for a send amount from 0 to 1: drag, arrow keys (Shift for
/// fine steps), Home and End, or a double-click back to 0. An `automated`
/// knob is ringed: an automation lane sets its amount while the song plays.
fn send_knob(
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
    ui.painter().circle_filled(center, radius, FACE);
    ui.painter().circle_stroke(
        center,
        radius,
        Stroke::new(1.0 * scale, if response.has_focus() { AMBER } else { SLOT }),
    );
    if *value > 0.0 {
        let arc: Vec<Pos2> = (0..=24)
            .map(|step| {
                let angle = (-225.0 + 270.0 * *value * step as f32 / 24.0).to_radians();
                center + Vec2::angled(angle) * (radius - 2.0 * scale)
            })
            .collect();
        ui.painter()
            .add(egui::Shape::line(arc, Stroke::new(2.0 * scale, AMBER)));
    }
    let pointer = Vec2::angled((-225.0 + 270.0 * *value).to_radians());
    ui.painter().line_segment(
        [center, center + pointer * (radius - 3.0 * scale)],
        Stroke::new(1.5 * scale, CREAM),
    );
    if automated {
        ui.painter().circle_stroke(
            center,
            radius + 2.0 * scale,
            Stroke::new(1.0 * scale, CREAM),
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
    response.on_hover_text(format!(
        "{name}: {:.0}%. Drag or use arrow keys; Shift for fine steps; double-click for 0.{}",
        *value * 100.0,
        if automated {
            "\nAutomated: its automation lane sets the send while the song plays."
        } else {
            ""
        }
    ));
    before != *value
}

/// The return of one FX bus: its effect, return level, input and limiter
/// switches, and a button that opens it in the Effects module. Returns
/// whether the bus changed and whether Edit was pressed.
fn return_strip(
    ui: &mut egui::Ui,
    art: Option<&Artwork>,
    scale: f32,
    height: f32,
    index: usize,
    bus: &mut FxBus,
) -> (bool, bool) {
    let name = format!("FX {}", index + 1);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(108.0 * scale, height), Sense::hover());
    if let Some(art) = art {
        art.tile(ui, 1, rect);
    } else {
        ui.painter().rect_filled(rect, 2.0, FACE);
    }
    ui.painter().rect_stroke(
        rect.shrink(0.5 * scale),
        3.0,
        Stroke::new(1.0 * scale, EDGE),
        egui::StrokeKind::Inside,
    );
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    text(ui, at(54.0, 14.0), &name, 13.0 * scale, CREAM);
    text(ui, at(54.0, 31.0), bus.module.label(), 11.0 * scale, CREAM);
    let before = *bus;
    let mut level = bus.return_level;
    send_knob(
        ui,
        Rect::from_center_size(at(54.0, 82.0), Vec2::splat(44.0 * scale)),
        scale,
        &format!("{name} return level"),
        &mut level,
        false,
    );
    bus.return_level = level;
    text(
        ui,
        at(54.0, 116.0),
        &format!("Return {:.0}%", bus.return_level * 100.0),
        11.0 * scale,
        CREAM,
    );
    for (y, label, key, on, hint) in [
        (
            136.0,
            "Input",
            format!("{name} power"),
            &mut bus.powered,
            "Off stops new sound into the effect; its tail rings out.",
        ),
        (
            164.0,
            "Limiter",
            format!("{name} limiter"),
            &mut bus.limiter,
            "Holds the return under -1 dBFS.",
        ),
    ] {
        let response = button(
            ui,
            Rect::from_min_size(at(14.0, y), Vec2::new(80.0, 22.0) * scale),
            label,
            &key,
            scale,
            *on,
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *on, &key)
        });
        if response
            .on_hover_text(format!("{key}: {}. {hint}", if *on { "on" } else { "off" }))
            .clicked()
        {
            *on = !*on;
        }
    }
    let edit = button(
        ui,
        Rect::from_min_size(
            at(14.0, height / scale - 40.0),
            Vec2::new(80.0, 24.0) * scale,
        ),
        "Edit",
        &format!("Edit {name}"),
        scale,
        false,
    )
    .on_hover_text(format!(
        "Show {name} in the Effects module, with all its controls."
    ))
    .clicked();
    (*bus != before, edit)
}

fn text(ui: &egui::Ui, pos: Pos2, value: &str, size: f32, color: Color32) {
    ui.painter().text(
        pos,
        Align2::CENTER_CENTER,
        value,
        FontId::proportional(size),
        color,
    );
}

fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    name: &str,
    scale: f32,
    selected: bool,
) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    let fill = if selected {
        AMBER
    } else if response.hovered() {
        RAISED
    } else {
        FACE
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0 * scale, if response.has_focus() { AMBER } else { SLOT }),
        egui::StrokeKind::Inside,
    );
    focus_ring(ui, &response, rect, scale);
    text(
        ui,
        rect.center(),
        label,
        10.0 * scale,
        if selected { INK } else { CREAM },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    response
}

/// Amber is illegible on ivory. An ink ring around the control carries
/// keyboard focus there; amber remains the inner edge on dark faces.
fn focus_ring(ui: &egui::Ui, response: &Response, rect: Rect, scale: f32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(1.0 * scale),
            3.0,
            Stroke::new(2.0 * scale, INK),
            egui::StrokeKind::Outside,
        );
    }
}

fn switch_button(
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
        if *enabled { ACCENT } else { MUTED },
    );
    text(
        ui,
        rect.center() + Vec2::new(0.0, 3.0) * scale,
        switch.letter(),
        11.0 * scale,
        if *enabled { ACCENT } else { MUTED },
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
        .on_hover_text(switch_hint(&label, switch, *enabled))
        .clicked();
    if clicked {
        *enabled = !*enabled;
    }
    clicked
}

fn gain_value(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str, gain: &mut f32) -> bool {
    ui.painter().rect_filled(rect, 3.0, FACE);
    let before = *gain;
    let db = amplitude_to_db(before);
    ui.scope_builder(
        egui::UiBuilder::new()
            .id(("gain-value", name))
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
                widget.fg_stroke.color = CREAM;
            }
            // A focused value shows an edge before it is typed in.
            widgets.active.bg_stroke = Stroke::new(1.0 * scale, CREAM);
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
            let response = ui.add(
                egui::DragValue::from_get_set(|value| {
                    if let Some(value) = value {
                        let value = value as f32;
                        let lowering_silence =
                            before == 0.0 && relative_edit && value <= SILENCE_DB;
                        if !lowering_silence
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
                        format!("{value}")
                    } else {
                        db_text(value)
                    }
                })
                .custom_parser(parse_db),
            );
            response.widget_info(|| {
                let mut info =
                    egui::WidgetInfo::drag_value(ui.is_enabled(), f64::from(db.max(SILENCE_DB)));
                info.label = Some(format!("{name} gain value"));
                info.current_text_value = Some(db_text(db));
                info
            });
            response.on_hover_text(format!(
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
    let mut response = ui.interact(
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
    ui.painter().rect_filled(slot, 4.0, SLOT);
    ui.painter().rect_stroke(
        slot.expand(1.0 * scale),
        4.0,
        Stroke::new(1.0 * scale, EDGE),
        egui::StrokeKind::Outside,
    );
    // A pointer edit leaves focus alone: a focused rail would take Space,
    // undo, and save from the workspace. Tab still focuses the rail for keys.
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
    let mut step = 0.0_f32;
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
    let cap = Pos2::new(
        rect.center().x,
        egui::lerp(rect.bottom()..=rect.top(), gain_fraction(*gain)),
    );
    if let Some(art) = art {
        art.fader(ui, cap);
    } else {
        ui.painter().rect_filled(
            Rect::from_center_size(cap, Vec2::new(24.0, 32.0) * scale),
            2.0,
            FACE,
        );
    }
    focus_ring(ui, &response, rect, scale);
    let db = amplitude_to_db(*gain);
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
    if before != *gain {
        response.mark_changed();
    }
    response.on_hover_text(format!(
        "{name} gain: {}. Drag to adjust; Shift gives fine movement. Arrow keys change 0.5 dB \
         (Shift: 0.1 dB). Home silences, End sets +12 dB. Double-click for unity. Negative dB \
         only reduces level.",
        db_text(db)
    ));
    before != *gain
}
