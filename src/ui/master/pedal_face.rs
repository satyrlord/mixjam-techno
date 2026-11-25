//! One compact pedal card: header (name, Edit button, active lamp),
//! gain-reduction meter on dynamics pedals, 1-3 hero dials (or 10-band mini EQ),
//! move buttons, and footswitch toggle.
use super::*;

pub(super) struct PedalCardOutcome {
    pub(super) moved_to: Option<usize>,
    pub(super) open_deep_edit: bool,
}

/// Render one compact mastering pedal card.
pub(super) fn pedal(
    ui: &mut egui::Ui,
    chain: &mut MasterChain,
    kind: PedalKind,
    rect: Rect,
    index: usize,
    metering: &Metering,
) -> PedalCardOutcome {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    let on = chain.is_on(kind);
    let ink = master_face_ink(kind);
    master_enclosure(ui, rect, kind, on);
    card_header(ui, rect, kind, ink, on);
    for y in [HEADER + 1.0, FOOTER_TOP - 1.0] {
        ui.painter().line_segment(
            [at(10.0, y), at(rect.width() - 10.0, y)],
            Stroke::new(1.0_f32, ink.gamma_multiply(0.28)),
        );
    }

    let mut open_deep_edit = card_edit_button(ui, rect, kind);
    let has_meter = card_meter(
        ui,
        at(rect.width() / 2.0, HEADER + 15.0),
        kind,
        on,
        metering,
        ink,
    );
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = master_face_dark_mode(kind);
        if kind == PedalKind::BoosterEq {
            open_deep_edit |= booster_eq_preview(ui, rect, chain, on);
        } else {
            theme::with_knob_style(master_motif(kind).knob, || {
                card_hero_dials(ui, rect, chain, kind, on, has_meter);
            });
        }
    });
    let moved_to = card_footer(ui, rect, chain, kind, index, on);
    PedalCardOutcome {
        moved_to,
        open_deep_edit,
    }
}

fn card_header(ui: &egui::Ui, rect: Rect, kind: PedalKind, ink: Color32, on: bool) {
    let label = title(kind);
    let booster = matches!(
        kind,
        PedalKind::BoosterEq | PedalKind::BoosterCompressor | PedalKind::BoosterWide
    );
    if booster {
        ui.painter().text(
            rect.min + Vec2::new(PEDAL_PADDING, 11.0),
            Align2::LEFT_CENTER,
            "Booster",
            FontId::proportional(8.5),
            ink,
        );
    }
    let room = (rect.width() - 88.0).max(32.0);
    let size = [13.0, 12.0, 11.0, 10.0]
        .into_iter()
        .find(|s| {
            ui.painter()
                .layout_no_wrap(label.into(), FontId::proportional(*s), ink)
                .size()
                .x
                <= room
        })
        .unwrap_or(9.5);
    ui.painter().text(
        rect.min + Vec2::new(PEDAL_PADDING, if booster { 24.5 } else { 18.5 }),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(size),
        ink,
    );
    lamp(ui, rect.min + Vec2::new(rect.width() - 16.0, 18.5), on);
}

fn card_edit_button(ui: &mut egui::Ui, rect: Rect, kind: PedalKind) -> bool {
    let btn = Rect::from_min_size(
        rect.min + Vec2::new(rect.width() - 66.0, 8.5),
        Vec2::new(38.0, 20.0),
    );
    let label = format!("Edit {}", name(kind));
    let resp = controls::button_at(ui, btn, &label, "", controls::ButtonRole::Secondary, true);
    ui.painter().text(
        btn.center() + Vec2::new(0.0, -0.5),
        Align2::CENTER_CENTER,
        "Edit",
        FontId::proportional(10.5),
        roles().legend,
    );
    reveal(name_control(resp, &label))
        .hover_hint(format!(
            "Open {} in Mastering deep edit to inspect curves and edit all parameters.",
            name(kind)
        ))
        .clicked()
}

/// Paint a dynamics pedal's gain-reduction meter under the header, and
/// tell whether the card has one.
fn card_meter(
    ui: &egui::Ui,
    center: Pos2,
    kind: PedalKind,
    on: bool,
    metering: &Metering,
    ink: Color32,
) -> bool {
    if let Some(thresholds) = reduction_lamps(kind) {
        let reduction = card_reduction(kind, on, metering);
        reduction_meter(ui, center, name(kind), reduction, &thresholds, ink);
        true
    } else if kind == PedalKind::Multiband {
        let bands = if on && metering.playing {
            metering.reading.multiband_reduction
        } else {
            [0.0; 3]
        };
        multiband_reduction_strip(ui, center, bands);
        true
    } else {
        false
    }
}

fn card_reduction(kind: PedalKind, on: bool, metering: &Metering) -> f32 {
    if !on || !metering.playing {
        return 0.0;
    }
    match kind {
        PedalKind::BusCompressor => metering.reading.compressor_reduction,
        _ => metering.reading.limiter_reduction,
    }
}

/// Paint the 15/30 ips tape-speed dial on both lower cards and Mastering Deep Edit.
/// A `compact_well` width gives it the compact face of the Deep Edit grid.
pub(super) fn tape_speed_dial(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    chain: &mut MasterChain,
    access_name: &str,
    on: bool,
    compact_well: Option<f32>,
) {
    let mut speed = f64::from(chain.tape.speed);
    let spec = Dial {
        name: access_name,
        label: "Speed",
        hint: "15 or 30 inches per second: moves the low-end bump and the treble roll-off.",
        range: KnobRange {
            low: 15.0,
            high: 30.0,
            default: 30.0,
            step: 15.0,
            logarithmic: false,
        },
        format: Format::Speed,
        choices: &[15.0, 30.0],
    };
    match compact_well {
        Some(well) => compact_dial(ui, positions, &mut speed, spec, on, well),
        None => dial(ui, positions, &mut speed, spec, on),
    }
    chain.tape.speed = if speed < 22.5 { 15 } else { 30 };
}

fn card_hero_dials(
    ui: &mut egui::Ui,
    rect: Rect,
    chain: &mut MasterChain,
    kind: PedalKind,
    on: bool,
    has_lamps: bool,
) {
    let knobs = hero_knobs(kind);
    let count = card_dial_count(kind).max(1);
    let left = (rect.width() - count as f32 * CELL) / 2.0;
    let rotary_y = if has_lamps { 102.0 } else { 94.0 };
    let well_y = if has_lamps { 144.0 } else { 136.0 };
    let label_y = if has_lamps { 168.0 } else { 160.0 };
    let cell = |slot: usize| {
        let x = left + (slot as f32 + 0.5) * CELL;
        [
            rect.min + Vec2::new(x, rotary_y),
            rect.min + Vec2::new(x, well_y),
            rect.min + Vec2::new(x, label_y),
        ]
    };
    for (slot, knob) in knobs.iter().enumerate() {
        let (label, name, hint, format) = face(*knob);
        let before = decimal(knob.get(chain));
        let mut value = before;
        dial(
            ui,
            cell(slot),
            &mut value,
            Dial {
                name: &name,
                label: &label,
                hint,
                range: knob.range(),
                format,
                choices: &[],
            },
            on,
        );
        if value != before {
            knob.set(chain, value as f32);
        }
    }
    if kind == PedalKind::Tape {
        tape_speed_dial(ui, cell(1), chain, "Tape speed", on, None);
    }
}

fn booster_eq_preview(ui: &mut egui::Ui, rect: Rect, chain: &MasterChain, on: bool) -> bool {
    let box_rect = Rect::from_min_max(
        rect.min + Vec2::new(14.0, HEADER + 10.0),
        rect.min + Vec2::new(rect.width() - 14.0, FOOTER_TOP - 28.0),
    );
    controls::well(ui, box_rect);
    let accent = if on {
        family_accent(PedalKind::BoosterEq)
    } else {
        roles().legend.gamma_multiply(0.45)
    };
    let mid_y = box_rect.center().y;
    ui.painter().line_segment(
        [
            Pos2::new(box_rect.left() + 6.0, mid_y),
            Pos2::new(box_rect.right() - 6.0, mid_y),
        ],
        Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.22)),
    );
    let points: Vec<Pos2> = chain
        .booster_eq
        .bands
        .iter()
        .enumerate()
        .map(|(i, &raw)| {
            let t = i as f32 / 9.0;
            let norm = (f32::from(raw) / f32::from(EQ_BAND_MAX) - 0.5) * 2.0;
            Pos2::new(
                box_rect.left() + 10.0 + t * (box_rect.width() - 20.0),
                mid_y - norm * (box_rect.height() * 0.38),
            )
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.0_f32, accent)));
    ui.painter().text(
        rect.min + Vec2::new(rect.width() / 2.0, FOOTER_TOP - 14.0),
        Align2::CENTER_CENTER,
        "10-band graphic EQ",
        FontId::proportional(10.5),
        master_face_ink(PedalKind::BoosterEq).gamma_multiply(0.85),
    );
    let hit = ui.interact(box_rect, ui.id().with("booster-eq-preview"), Sense::click());
    reveal(name_control(hit, "Booster EQ curve"))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .hover_hint("Click to edit all 10 Booster EQ bands in Mastering deep edit.")
        .clicked()
}

fn card_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    chain: &mut MasterChain,
    kind: PedalKind,
    index: usize,
    on: bool,
) -> Option<usize> {
    let moved = if kind.movable() {
        let follow =
            ui.data(|data| data.get_temp::<(PedalKind, bool, bool)>(ui.id().with("move-follow")));
        let left = move_button(ui, rect, chain, kind, index, true, follow);
        let right = move_button(ui, rect, chain, kind, index, false, follow);
        left.or(right)
    } else {
        None
    };
    pedal_power(ui, rect, chain, kind, on);
    moved
}

fn move_button(
    ui: &mut egui::Ui,
    rect: Rect,
    chain: &MasterChain,
    kind: PedalKind,
    index: usize,
    left: bool,
    follow: Option<(PedalKind, bool, bool)>,
) -> Option<usize> {
    let target = if left {
        index.checked_sub(1)
    } else {
        chain
            .order
            .get(index + 1)
            .filter(|k| k.movable())
            .map(|_| index + 1)
    };
    let x = if left {
        rect.left() + 12.0
    } else {
        rect.right() - 40.0
    };
    let button = Rect::from_min_size(
        Pos2::new(x, rect.top() + FOOTER_TOP + 4.0),
        Vec2::new(28.0, 24.0),
    );
    let step = if left { "left" } else { "right" };
    let label = format!("Move {} {step}", name(kind));
    let enabled = target.is_some();
    let response = controls::button_at(
        ui,
        button,
        &label,
        "",
        controls::ButtonRole::Secondary,
        enabled,
    );
    arrow(ui, button.center(), if left { -1.0 } else { 1.0 }, enabled);
    let response = reveal(name_control(response, &label)).hover_hint(format!(
        "Move {} one place {step} in the signal path.",
        name(kind)
    ));
    let follow_id = ui.id().with("move-follow");
    follow_move(&response, follow_id, follow, kind, left);
    let to = target.filter(|_| response.clicked())?;
    queue_move_follow(ui, chain, kind, left, to, response.has_focus());
    Some(to)
}

fn queue_move_follow(
    ui: &egui::Ui,
    chain: &MasterChain,
    kind: PedalKind,
    left: bool,
    to: usize,
    focused: bool,
) {
    let next_left = if left {
        to != 0
    } else {
        !chain.order.get(to + 1).is_some_and(|next| next.movable())
    };
    let id = ui.id().with("move-follow");
    ui.data_mut(|data| data.insert_temp(id, (kind, next_left, focused)));
}

fn follow_move(
    response: &Response,
    id: egui::Id,
    follow: Option<(PedalKind, bool, bool)>,
    kind: PedalKind,
    left: bool,
) {
    if let Some((target, target_left, focus)) = follow
        && (target, target_left) == (kind, left)
    {
        response
            .ctx
            .data_mut(|data| data.remove::<(PedalKind, bool, bool)>(id));
        if focus {
            response.request_focus();
        }
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
    }
}

fn pedal_power(ui: &mut egui::Ui, rect: Rect, chain: &mut MasterChain, kind: PedalKind, on: bool) {
    let mut switch = on;
    let response = controls::instrument::bat_toggle(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + FOOTER_TOP + 16.0),
            Vec2::new(30.0, 22.0),
        ),
        name(kind),
        "",
        &mut switch,
        "A pedal that is off keeps its settings and passes the signal on.",
        false,
    );
    reveal(response);
    chain.set_on(kind, switch);
}

/// Bring a control into view at once when it gains keyboard focus, so Tab
/// can walk the whole row. The row takes the scroll for itself, so the
/// board returns to the top on the next frame, which shows the whole row.
pub(super) fn reveal(response: Response) -> Response {
    if response.gained_focus() {
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
        response
            .ctx
            .data_mut(|data| data.insert_temp(raise_id(), true));
    }
    response
}

/// The request to show the top of a scrolled board on its next frame.
pub(super) fn raise_id() -> egui::Id {
    egui::Id::new("mastering-raise")
}

/// A row of lamps for the gain reduction of a dynamics pedal; each lights
/// once the reduction reaches its step.
fn reduction_meter(
    ui: &egui::Ui,
    center: Pos2,
    pedal: &str,
    reduction: f32,
    steps: &[f32; 6],
    ink: Color32,
) {
    let first = center.x - 2.5 * 14.0 + 10.0;
    ui.painter().text(
        Pos2::new(first - 22.0, center.y),
        Align2::CENTER_CENTER,
        "GR",
        FontId::proportional(10.0),
        ink,
    );
    for (index, step) in steps.iter().enumerate() {
        led(
            ui,
            Pos2::new(first + index as f32 * 14.0, center.y),
            4.0,
            reduction >= *step,
            roles().lamp_warn,
        );
    }
    let response = ui.interact(
        Rect::from_center_size(center + Vec2::new(-6.0, 0.0), Vec2::new(112.0, 14.0)),
        ui.id().with(("reduction", pedal)),
        Sense::hover(),
    );
    named(
        &response,
        egui::WidgetType::Label,
        &format!("{pedal} gain reduction {reduction:.1} dB"),
    );
    response.hover_hint(format!(
        "Gain reduction. The lamps light at {} dB.",
        steps.map(|step| step.to_string()).join(", ")
    ));
}

/// Compact 3-band `L / M / H` gain reduction bars on the Multiband card.
fn multiband_reduction_strip(ui: &egui::Ui, center: Pos2, bands: [f32; 3]) {
    let strip = Rect::from_center_size(center, Vec2::new(164.0, 14.0));
    controls::well(ui, strip);
    let slot_w = (strip.width() - 8.0) / 3.0;
    let warn = roles().lamp_warn;
    let track_bg = roles().well_edge.gamma_multiply(0.35);
    for (idx, (letter, db)) in ["L", "M", "H"].into_iter().zip(bands).enumerate() {
        let x0 = strip.left() + 4.0 + idx as f32 * slot_w;
        ui.painter().text(
            Pos2::new(x0 + 5.0, center.y),
            Align2::CENTER_CENTER,
            letter,
            FontId::proportional(8.5),
            roles().legend,
        );
        let track = Rect::from_center_size(
            Pos2::new(x0 + 11.0 + (slot_w - 13.0) * 0.5, center.y),
            Vec2::new(slot_w - 14.0, 6.0),
        );
        ui.painter().rect_filled(track, 1.0, track_bg);
        let frac = (db / 6.0).clamp(0.0, 1.0);
        if frac > 0.01 {
            let fill = Rect::from_min_max(
                track.min,
                Pos2::new(track.left() + track.width() * frac, track.max.y),
            );
            ui.painter().rect_filled(fill, 1.0, warn);
        }
    }
    let response = ui.interact(strip, ui.id().with("multiband-gr-strip"), Sense::hover());
    named(
        &response,
        egui::WidgetType::Label,
        &format!(
            "Multiband gain reduction L {:.1} dB, M {:.1} dB, H {:.1} dB",
            bands[0], bands[1], bands[2]
        ),
    );
    response.hover_hint("Live 3-band gain reduction (Low <200 Hz, Mid 200-3k, High >3 kHz).");
}
