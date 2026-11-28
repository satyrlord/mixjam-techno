//! The strips: the track strip, the return strip, the row that lays them out,
//! and the state text each announces.
use super::*;

pub(crate) fn show(
    ui: &mut egui::Ui,
    project: &Project,
    summaries: &[(String, String)],
    master: &mut f32,
    fx: &mut FxBuses,
    state: MasterState<'_>,
) -> Changes {
    // Strips are measured in whole physical pixels, as Vintage's grain and
    // fader cap are, so every theme keeps the same layout.
    let scale = 1.0 / ui.ctx().pixels_per_point();
    let width = 108.0 * scale;
    let whole = ui.available_rect_before_wrap();
    floor(ui.painter(), whole);
    // A 400-point panel gives the physical gain rail 96 points. A short
    // window can reduce the panel to 370 points and the rail to 68. Its
    // interactive area also includes the cap's 16-point overhang at each end.
    let height = (ui.available_height() - 14.0).max(304.0 * scale);
    let bank_width = (ui.available_width() - width - 12.0 * scale).max(width);
    let occupied = project.occupied_tracks();
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
                                track_strips(
                                    ui,
                                    project,
                                    &occupied,
                                    summaries,
                                    [scale, height],
                                    &mut result,
                                );
                                // The returns follow the tracks that feed them.
                                return_strips(ui, project, fx, [scale, height], &mut result);
                            });
                        }
                    });
                super::super::view::name_scroll_area(ui, "mixer_scroll", "Mixer");
            },
        );
        ui.scope_builder(egui::UiBuilder::new().id_salt("mixer_master"), |ui| {
            let summary = master_summary(*state.bypass, state.pedals);
            let mut bypassed = false;
            result.master |= strip(
                ui,
                scale,
                height,
                (
                    "Master",
                    summary.as_str(),
                    "The master pedalboard, after the mix. Mastering edits its pedals; \
                     Bypass skips them.",
                ),
                master,
                Controls::Master {
                    state,
                    bypassed: &mut bypassed,
                },
            );
            result.bypass = bypassed;
        });
    });
    result
}

/// The strips of the occupied tracks, in track order, at the strip scale
/// and height.
fn track_strips(
    ui: &mut egui::Ui,
    project: &Project,
    occupied: &[usize],
    summaries: &[(String, String)],
    [scale, height]: [f32; 2],
    result: &mut Changes,
) {
    let any_solo = project.any_solo();
    for &index in occupied {
        ui.scope_builder(
            egui::UiBuilder::new().id_salt(("mixer_track", index)),
            |ui| {
                let (summary, detail) = &summaries[index];
                let track = &project.tracks[index];
                let mut mix = track.mix();
                let mut gain = mix.gain;
                let mut open_eq = false;
                let changed = strip(
                    ui,
                    scale,
                    height,
                    (
                        &format!("Track {}", index + 1),
                        summary.as_str(),
                        detail.as_str(),
                    ),
                    &mut gain,
                    Controls::Track(TrackControls {
                        index,
                        mix: &mut mix,
                        automated: std::array::from_fn(|bus| {
                            !track.automation.sends[bus].is_empty()
                        }),
                        pan_automated: !track.automation.pan.is_empty(),
                        any_solo,
                        eq_on: track.processing_on(),
                        open_eq: &mut open_eq,
                    }),
                );
                mix.gain = gain;
                if changed {
                    result.tracks.push((index, mix));
                }
                if open_eq {
                    result.eq = Some(index);
                }
            },
        );
    }
}

/// The return strips of the four FX buses, at the strip scale and height.
fn return_strips(
    ui: &mut egui::Ui,
    project: &Project,
    fx: &mut FxBuses,
    [scale, height]: [f32; 2],
    result: &mut Changes,
) {
    for (index, bus) in fx.buses.iter_mut().enumerate() {
        ui.scope_builder(
            egui::UiBuilder::new().id_salt(("mixer_return", index)),
            |ui| {
                let automated = !project.return_automation[index].is_empty();
                let (edited, open) = return_strip(ui, scale, height, index, bus, automated);
                result.fx |= edited;
                if open {
                    result.open = Some(index);
                }
            },
        );
    }
}

/// Lays out a strip of `height` and paints its body and edge. A return strip
/// has the return material.
fn strip_frame(ui: &mut egui::Ui, scale: f32, height: f32, is_return: bool) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(108.0 * scale, height), Sense::hover());
    strip_body(ui.painter(), rect, is_return);
    ui.painter().rect_stroke(
        rect.shrink(0.5 * scale),
        roles().corner(3.0),
        Stroke::new(1.0 * scale, roles().strip_edge),
        egui::StrokeKind::Inside,
    );
    rect
}

/// What the Mixer's Master strip shows besides its level: the master
/// pedalboard's bypass, how many of its pedals are on, and the output meters.
pub(crate) struct MasterState<'a> {
    pub bypass: &'a mut bool,
    pub pedals: usize,
    pub metering: &'a super::super::master::Metering,
}

/// The Master strip's summary line: the state of the master pedalboard.
fn master_summary(bypass: bool, pedals: usize) -> String {
    match (bypass, pedals) {
        (true, _) => "Bypassed".into(),
        (false, 0) => "No pedals on".into(),
        (false, 1) => "1 pedal on".into(),
        (false, count) => format!("{count} pedals on"),
    }
}

/// What a strip holds under its fader: a track's sends, pan, and switches,
/// or the Master strip's meter and Bypass.
enum Controls<'a> {
    Track(TrackControls<'a>),
    Master {
        state: MasterState<'a>,
        /// Whether Bypass was pressed.
        bypassed: &'a mut bool,
    },
}

struct TrackControls<'a> {
    index: usize,
    /// The track's mix; the strip edits its gain through its own argument.
    mix: &'a mut TrackMix,
    /// Which sends an automation lane moves instead of the knob.
    automated: [bool; BUSES],
    /// Whether the pan lane moves the pan instead of the knob.
    pan_automated: bool,
    /// Whether any track was soloed when the mixer was laid out.
    any_solo: bool,
    /// Whether the track's channel EQ, drive, compressor, or ducker is on or
    /// its width is not 100%, and whether its EQ button was pressed.
    eq_on: bool,
    open_eq: &'a mut bool,
}

fn strip(
    ui: &mut egui::Ui,
    scale: f32,
    height: f32,
    identity: (&str, &str, &str),
    gain: &mut f32,
    controls: Controls<'_>,
) -> bool {
    let (name, summary, detail) = identity;
    let rect = strip_frame(ui, scale, height, false);
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    // Master ends the signal chain: its name takes the heading face at the
    // section size, which holds its rank even in Aero's light, wide face.
    if matches!(controls, Controls::Master { .. }) {
        ui.painter().text(
            at(54.0, 14.0),
            Align2::CENTER_CENTER,
            name,
            FontId::new(theme::type_size::SECTION * scale, theme::heading_family()),
            roles().strip_text,
        );
    } else {
        text(ui, at(54.0, 14.0), name, 13.0 * scale, roles().strip_text);
    }
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
    changed |= gain_rail(ui, scale, rail, name, gain);
    let zero_y = egui::lerp(rail.bottom()..=rail.top(), gain_fraction(1.0));
    // Each label is centred on its level. The labels carry no ticks: a tick
    // before a minus sign read as a second minus.
    for (db, label) in [(12.0, "+12"), (-12.0, "−12"), (-24.0, "−24")] {
        let y = egui::lerp(
            rail.bottom()..=rail.top(),
            gain_fraction(db_to_amplitude(db)),
        );
        ui.painter().text(
            Pos2::new(rail.right() + 4.0 * scale, y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(10.0 * scale),
            roles().strip_text,
        );
    }
    ui.painter().text(
        Pos2::new(rail.right() + 4.0 * scale, rail.bottom()),
        Align2::LEFT_CENTER,
        "−inf",
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
        Look::Strip(scale),
        name,
    ) {
        changed |= *gain != 1.0;
        *gain = 1.0;
    }
    let (state, bypassed) = match controls {
        Controls::Master { state, bypassed } => (state, bypassed),
        Controls::Track(track) => {
            return track_controls(ui, scale, height, rect, name, track) || changed;
        }
    };
    // Where a track has its sends, pan, and state, Master meters the output:
    // the true-peak bar, the highest peak, and the OVER lamp.
    let foot = height / scale;
    super::super::master::strip_meter(
        ui,
        [
            Rect::from_min_size(at(8.0, foot - 124.0), Vec2::new(92.0, 10.0) * scale),
            Rect::from_min_size(at(8.0, foot - 106.0), Vec2::new(92.0, 16.0) * scale),
            Rect::from_center_size(at(54.0, foot - 74.0), Vec2::new(54.0, 22.0) * scale),
        ],
        state.metering,
        roles().strip_text,
    );
    // In line with the Edit buttons of the return strips beside it.
    *bypassed = bypass_button(
        ui,
        Rect::from_min_size(at(14.0, foot - 40.0), Vec2::new(80.0, 24.0) * scale),
        scale,
        state.bypass,
    );
    changed
}

/// A track strip's sends, pan, state, and switches under its fader. Returns
/// whether the track's mix changed.
fn track_controls(
    ui: &mut egui::Ui,
    scale: f32,
    height: f32,
    rect: Rect,
    name: &str,
    controls: TrackControls<'_>,
) -> bool {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    let mut changed = false;
    let TrackControls {
        index,
        mix,
        automated,
        pan_automated,
        any_solo,
        eq_on,
        open_eq,
    } = controls;
    // The sends to FX 1 to 4, from left to right, in the gap between the
    // rail and the pan.
    for (bus, send) in mix.sends.iter_mut().enumerate() {
        let x = 15.0 + bus as f32 * 26.0;
        changed |= send_knob(
            ui,
            Rect::from_center_size(at(x, height / scale - 123.0), Vec2::splat(13.0 * scale)),
            Look::Strip(scale),
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
        Look::Strip(scale),
        &format!("Track {}", index + 1),
        &mut mix.pan,
        pan_automated,
    );
    if center_button(
        ui,
        Rect::from_center_size(
            at(54.0, height / scale - 90.0),
            Vec2::new(40.0, 14.0) * scale,
        ),
        Look::Strip(scale),
        name,
    ) {
        changed |= mix.pan != 0.0;
        mix.pan = 0.0;
    }
    text(
        ui,
        at(54.0, height / scale - 72.0),
        &track_state(mix.silence(any_solo), pan_automated, mix.pan),
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
        Look::Strip(scale),
        name,
        eq_on,
    ) {
        *open_eq = true;
    }
    for (switch, x, on) in [
        (TrackSwitch::Mute, 40.0, &mut mix.muted),
        (TrackSwitch::Solo, 72.0, &mut mix.solo),
    ] {
        changed |= switch_button(
            ui,
            Rect::from_min_size(at(x, height / scale - 56.0), Vec2::new(28.0, 24.0) * scale),
            Look::Strip(scale),
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
        Look::Strip(scale),
        name,
        &mut mix.polarity_inverted,
    );
    for (switch, x, on) in [
        (TrackSwitch::Left, 40.0, &mut mix.left_enabled),
        (TrackSwitch::Right, 72.0, &mut mix.right_enabled),
    ] {
        changed |= switch_button(
            ui,
            Rect::from_min_size(at(x, height / scale - 28.0), Vec2::new(28.0, 24.0) * scale),
            Look::Strip(scale),
            name,
            switch,
            on,
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
    scale: f32,
    height: f32,
    index: usize,
    bus: &mut FxBus,
    automated: bool,
) -> (bool, bool) {
    let name = format!("FX {}", index + 1);
    let rect = strip_frame(ui, scale, height, true);
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
        Look::Strip(scale),
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
        Look::Strip(scale),
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
fn return_state(bus: &FxBus, automated: bool) -> String {
    if automated {
        "Return automated".to_string()
    } else {
        format!("Return {:.0}%", bus.return_level * 100.0)
    }
}

/// A track's state: why it is silent, else its pan: Muted, No output,
/// Solo elsewhere, Pan automated, or the pan position.
fn track_state(silence: Option<Silence>, pan_automated: bool, pan: f32) -> String {
    match silence {
        Some(Silence::Muted) => "Muted".into(),
        Some(Silence::NoOutput) => "No output".into(),
        Some(Silence::SoloElsewhere) => "Solo elsewhere".into(),
        None if pan_automated => "Pan automated".into(),
        None => pan_description(pan),
    }
}
